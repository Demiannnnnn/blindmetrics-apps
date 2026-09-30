//! Authenticated calls for the worker session.
//!
//! Tokens live only in the [`CredentialStore`] (SEC-05). Refresh tokens are
//! single use: the rotated refresh token is persisted *before* the new access
//! token is used (api-contract §2), and a 401 on refresh means the worker was
//! revoked.
//!
//! Refresh is **single-flight** (SEC-14, TASK-INT-02): the engine loop and tasks spawned by
//! the UI (consent, data requests) may hit a 401 at the same time. Two refreshes with the same
//! token look like a replay to the API, which revokes the whole token family — and the agent
//! would then wipe its enrollment. One refresh runs at a time; waiters reuse its token.

use crate::api::client::ApiClient;
use crate::credentials::CredentialStore;
use crate::errors::AgentError;
use std::future::Future;

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("This device is not enrolled")]
    NotEnrolled,
    #[error("The worker was revoked or the session family is no longer valid")]
    Revoked,
    #[error("Could not persist rotated credentials: {0}")]
    Store(String),
    /// Rejected locally before any request (e.g. an empty consent selection).
    #[error("{0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Api(#[from] AgentError),
}

/// Serializes refreshes of this process (SEC-14).
static REFRESH_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Rotates the session with the stored refresh token and returns the new access token.
///
/// `failed_token` is the access token that got the 401: if another task already rotated it,
/// the stored token is returned without calling the API.
pub async fn refresh_session(
    api: &ApiClient,
    creds: &dyn CredentialStore,
    failed_token: Option<&str>,
) -> Result<String, SessionError> {
    let _guard = REFRESH_LOCK.lock().await;
    if let (Some(failed), Some(current)) = (failed_token, creds.get_access_token()) {
        if current != failed {
            tracing::info!("refresh skipped: token already rotated by another task");
            return Ok(current);
        }
    }
    let refresh_token = creds.get_refresh_token().ok_or(SessionError::Revoked)?;
    let auth = match api.refresh(&refresh_token).await {
        Ok(auth) => auth,
        Err(AgentError::Unauthorized) => return Err(SessionError::Revoked),
        Err(e) => return Err(SessionError::Api(e)),
    };
    // Single-use refresh token: persist the new one first.
    creds
        .set_refresh_token(Some(&auth.refresh_token))
        .map_err(SessionError::Store)?;
    creds
        .set_access_token(Some(&auth.access_token))
        .map_err(SessionError::Store)?;
    Ok(auth.access_token)
}

/// Runs `op` with the stored access token, refreshing once on 401.
pub async fn with_access_token<T, F, Fut>(
    api: &ApiClient,
    creds: &dyn CredentialStore,
    op: F,
) -> Result<T, SessionError>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<T, AgentError>>,
{
    let token = creds.get_access_token().ok_or(SessionError::NotEnrolled)?;
    match op(token.clone()).await {
        Err(AgentError::Unauthorized) => {
            let fresh = refresh_session(api, creds, Some(&token)).await?;
            match op(fresh).await {
                Err(AgentError::Unauthorized) => Err(SessionError::Revoked),
                other => other.map_err(SessionError::Api),
            }
        }
        other => other.map_err(SessionError::Api),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::InMemoryCredentialStore;
    use wiremock::matchers::{bearer_token, body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn auth_body(access: &str, refresh: &str) -> serde_json::Value {
        serde_json::json!({ "data": {
            "access_token": access, "refresh_token": refresh,
            "token_type": "Bearer", "expires_in": 900,
            "user": { "id": "w-1", "email": "w@acme.cl", "name": "W", "role": "WORKER" }
        }})
    }

    fn store_with(access: &str, refresh: &str) -> InMemoryCredentialStore {
        let creds = InMemoryCredentialStore::new();
        creds.set_access_token(Some(access)).unwrap();
        creds.set_refresh_token(Some(refresh)).unwrap();
        creds
    }

    #[tokio::test]
    async fn test_session_refreshes_once_and_persists_rotated_tokens() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/auth/refresh"))
            .and(body_json(serde_json::json!({ "refresh_token": "r-old" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(auth_body("a-new", "r-new")))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let api = ApiClient::new(server.uri()).unwrap();
        let creds = store_with("a-old", "r-old");
        let seen = std::sync::Mutex::new(Vec::new());
        let out = with_access_token(&api, &creds, |t| {
            seen.lock().unwrap().push(t.clone());
            async move {
                if t == "a-old" {
                    Err(AgentError::Unauthorized)
                } else {
                    Ok(t)
                }
            }
        })
        .await
        .unwrap();

        assert_eq!(out, "a-new");
        assert_eq!(*seen.lock().unwrap(), vec!["a-old", "a-new"]);
        assert_eq!(creds.get_refresh_token().as_deref(), Some("r-new"));
        assert_eq!(creds.get_access_token().as_deref(), Some("a-new"));
    }

    #[tokio::test]
    async fn test_session_refresh_401_means_revoked() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/auth/refresh"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let api = ApiClient::new(server.uri()).unwrap();
        let creds = store_with("a-old", "r-old");
        let err = with_access_token(&api, &creds, |_t| async {
            Err::<(), _>(AgentError::Unauthorized)
        })
        .await
        .unwrap_err();
        assert!(matches!(err, SessionError::Revoked));
    }

    #[tokio::test]
    async fn test_session_concurrent_refresh_single_flight() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/auth/refresh"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(auth_body("a-new", "r-new"))
                    .set_delay(std::time::Duration::from_millis(50)),
            )
            .expect(1)
            .mount(&server)
            .await;

        let api = ApiClient::new(server.uri()).unwrap();
        let creds = std::sync::Arc::new(store_with("a-old", "r-old"));
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let api = api.clone();
            let creds = creds.clone();
            tasks.push(tokio::spawn(async move {
                with_access_token(&api, creds.as_ref(), |t| async move {
                    if t == "a-old" {
                        Err(AgentError::Unauthorized)
                    } else {
                        Ok(t)
                    }
                })
                .await
            }));
        }
        for task in tasks {
            assert_eq!(task.await.unwrap().unwrap(), "a-new");
        }
        // One refresh, no family revocation, enrollment intact.
        assert_eq!(creds.get_refresh_token().as_deref(), Some("r-new"));
    }

    #[tokio::test]
    async fn test_session_without_token_is_not_enrolled() {
        let api = ApiClient::new("http://127.0.0.1:9".into()).unwrap();
        let creds = InMemoryCredentialStore::new();
        let err = with_access_token(&api, &creds, |_t| async { Ok::<_, AgentError>(()) })
            .await
            .unwrap_err();
        assert!(matches!(err, SessionError::NotEnrolled));
    }

    #[tokio::test]
    async fn test_session_passes_through_non_auth_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/workers/me"))
            .and(bearer_token("a-ok"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let api = ApiClient::new(server.uri()).unwrap();
        let creds = store_with("a-ok", "r");
        let err = with_access_token(&api, &creds, |t| {
            let api = api.clone();
            async move { api.get_me(&t).await }
        })
        .await
        .unwrap_err();
        assert!(matches!(
            err,
            SessionError::Api(AgentError::ServerError(503))
        ));
        // No refresh attempted: tokens unchanged.
        assert_eq!(creds.get_refresh_token().as_deref(), Some("r"));
    }
}
