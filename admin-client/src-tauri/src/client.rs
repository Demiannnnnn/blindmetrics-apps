use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::sync::RwLock;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_VERSION_HEADER: &str = "X-App-Version";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SingleResponse<T> {
    pub data: T,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CollectionResponse<T> {
    pub data: Vec<T>,
    #[serde(default)]
    pub pagination: Option<PaginationMeta>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PaginationMeta {
    pub total: usize,
    pub page: usize,
    pub per_page: usize,
    pub total_pages: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UserDto {
    pub id: String,
    pub email: String,
    pub name: String,
    pub role: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub user: UserDto,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AdminMeResponse {
    pub id: String,
    pub email: String,
    pub name: String,
    pub organization: String,
    #[serde(default)]
    pub public_key: Option<String>,
    #[serde(default)]
    pub public_key_fingerprint: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AdminProfile {
    pub id: String,
    pub email: String,
    pub name: String,
    pub organization: String,
    #[serde(default)]
    pub public_key: Option<String>,
    #[serde(default)]
    pub public_key_fingerprint: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RegisterPublicKeyResponse {
    pub public_key: String,
    pub public_key_fingerprint: String,
    #[serde(default)]
    pub public_key_registered_at: Option<String>,
}

pub struct AdminApiClient {
    client: Client,
    base_url: RwLock<String>,
    access_token: RwLock<Option<String>>,
    current_user: RwLock<Option<AdminProfile>>,
    failed_unlock_attempts: std::sync::atomic::AtomicUsize,
}

static API_CLIENT: std::sync::OnceLock<AdminApiClient> = std::sync::OnceLock::new();

pub fn get_api_client() -> &'static AdminApiClient {
    API_CLIENT.get_or_init(|| {
        let default_url = std::env::var("ADMIN_API_URL")
            .or_else(|_| std::env::var("VITE_API_URL"))
            .unwrap_or_else(|_| "http://localhost:8080/api/v1".to_string());
        AdminApiClient::new(default_url)
    })
}

impl AdminApiClient {
    pub fn new(base_url: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            base_url: RwLock::new(base_url),
            access_token: RwLock::new(None),
            current_user: RwLock::new(None),
            failed_unlock_attempts: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    pub fn set_base_url(&self, url: String) {
        *self.base_url.write().unwrap_or_else(|p| p.into_inner()) = url;
    }

    pub fn get_base_url(&self) -> String {
        self.base_url
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub fn get_current_user(&self) -> Option<AdminProfile> {
        self.current_user
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub fn set_current_user_public_key(&self, pubkey: String, fp: String) {
        let mut lock = self.current_user.write().unwrap_or_else(|p| p.into_inner());
        if let Some(ref mut profile) = *lock {
            profile.public_key = Some(pubkey);
            profile.public_key_fingerprint = Some(fp);
        }
    }

    #[cfg(test)]
    pub fn set_access_token_for_test(&self, token: Option<String>) {
        *self.access_token.write().unwrap_or_else(|p| p.into_inner()) = token;
    }

    #[cfg(test)]
    pub fn get_access_token_for_test(&self) -> Option<String> {
        self.access_token
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<AdminProfile, String> {
        let base = self.get_base_url();
        let url = format!("{}/auth/login", base.trim_end_matches('/'));

        let resp = self
            .client
            .post(&url)
            .header(APP_VERSION_HEADER, APP_VERSION)
            .json(&serde_json::json!({
                "email": email,
                "password": password,
                "role": "ADMIN"
            }))
            .send()
            .await
            .map_err(|e| format!("Network error: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_body = resp.text().await.unwrap_or_default();
            return Err(format!("Login failed ({}): {}", status, err_body));
        }

        let body: SingleResponse<AuthResponse> = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse auth response: {}", e))?;

        let auth_data = body.data;

        // Persist refresh token securely in OS Keystore
        let _ = crate::keystore::save_admin_refresh_token(&auth_data.refresh_token);

        *self.access_token.write().unwrap_or_else(|p| p.into_inner()) =
            Some(auth_data.access_token.clone());

        // Fetch /admins/me to retrieve organization and registered public key
        let me_url = format!("{}/admins/me", base.trim_end_matches('/'));
        let (org, pub_key, pub_fp) = match self
            .client
            .get(&me_url)
            .header(APP_VERSION_HEADER, APP_VERSION)
            .bearer_auth(&auth_data.access_token)
            .send()
            .await
        {
            Ok(me_resp) if me_resp.status().is_success() => {
                if let Ok(me_data) = me_resp.json::<SingleResponse<AdminMeResponse>>().await {
                    (
                        me_data.data.organization,
                        me_data.data.public_key,
                        me_data.data.public_key_fingerprint,
                    )
                } else {
                    ("Acme SpA".to_string(), None, None)
                }
            }
            _ => ("Acme SpA".to_string(), None, None),
        };

        let profile = AdminProfile {
            id: auth_data.user.id,
            email: auth_data.user.email,
            name: auth_data.user.name,
            organization: org,
            public_key: pub_key,
            public_key_fingerprint: pub_fp,
        };

        *self.current_user.write().unwrap_or_else(|p| p.into_inner()) = Some(profile.clone());
        Ok(profile)
    }

    pub async fn register(
        &self,
        name: &str,
        email: &str,
        password: &str,
        organization: &str,
    ) -> Result<AdminProfile, String> {
        let base = self.get_base_url();
        let url = format!("{}/auth/register", base.trim_end_matches('/'));

        let resp = self
            .client
            .post(&url)
            .header(APP_VERSION_HEADER, APP_VERSION)
            .json(&serde_json::json!({
                "name": name,
                "email": email,
                "password": password,
                "organization": organization
            }))
            .send()
            .await
            .map_err(|e| format!("Network error: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_body = resp.text().await.unwrap_or_default();
            return Err(format!("Registration failed ({}): {}", status, err_body));
        }

        let body: SingleResponse<AuthResponse> = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse register response: {}", e))?;

        let auth_data = body.data;
        let _ = crate::keystore::save_admin_refresh_token(&auth_data.refresh_token);

        *self.access_token.write().unwrap_or_else(|p| p.into_inner()) =
            Some(auth_data.access_token);

        let profile = AdminProfile {
            id: auth_data.user.id,
            email: auth_data.user.email,
            name: auth_data.user.name,
            organization: organization.to_string(),
            public_key: None,
            public_key_fingerprint: None,
        };

        *self.current_user.write().unwrap_or_else(|p| p.into_inner()) = Some(profile.clone());
        Ok(profile)
    }

    pub async fn refresh_session(&self) -> Result<String, String> {
        let refresh_token = crate::keystore::load_admin_refresh_token()
            .map_err(|e| format!("Keystore load error: {}", e))?;

        let base = self.get_base_url();
        let url = format!("{}/auth/refresh", base.trim_end_matches('/'));

        let resp = self
            .client
            .post(&url)
            .header(APP_VERSION_HEADER, APP_VERSION)
            .json(&serde_json::json!({ "refresh_token": refresh_token }))
            .send()
            .await
            .map_err(|e| format!("Network error on refresh: {}", e))?;

        if !resp.status().is_success() {
            self.logout();
            return Err("Session refresh failed. Session revoked.".into());
        }

        let body: SingleResponse<AuthResponse> = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse refresh response: {}", e))?;

        let auth_data = body.data;
        let _ = crate::keystore::save_admin_refresh_token(&auth_data.refresh_token);
        *self.access_token.write().unwrap_or_else(|p| p.into_inner()) =
            Some(auth_data.access_token.clone());

        Ok(auth_data.access_token)
    }

    pub fn lock_and_flush(&self) {
        if let Ok(mut token_lock) = self.access_token.write() {
            if let Some(ref mut token) = *token_lock {
                use zeroize::Zeroize;
                token.zeroize();
            }
            *token_lock = None;
        }
        tracing::info!("Admin lock_and_flush: access_token zeroized in RAM (SEC-11)");
    }

    pub fn is_locked(&self) -> bool {
        self.access_token
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .is_none()
            && self
                .current_user
                .read()
                .unwrap_or_else(|p| p.into_inner())
                .is_some()
    }

    pub async fn unlock_with_password(&self, password: &str) -> Result<AdminProfile, String> {
        let email = {
            let user_lock = self.current_user.read().unwrap_or_else(|p| p.into_inner());
            user_lock
                .as_ref()
                .map(|u| u.email.clone())
                .ok_or_else(|| "No active session to unlock".to_string())?
        };

        match self.login(&email, password).await {
            Ok(profile) => {
                self.failed_unlock_attempts
                    .store(0, std::sync::atomic::Ordering::SeqCst);
                tracing::info!("Workspace unlocked successfully via password verification");
                Ok(profile)
            }
            Err(e) => {
                let attempts = self
                    .failed_unlock_attempts
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                    + 1;
                tracing::warn!("Failed unlock attempt {}/5 for {}", attempts, email);
                if attempts >= 5 {
                    self.logout();
                    tracing::error!(
                        "Maximum unlock attempts exceeded (5). Session purged and revoked (SEC-11)"
                    );
                    return Err(
                        "Límite de intentos fallidos alcanzado (5). Sesión cerrada por seguridad."
                            .to_string(),
                    );
                }
                Err(format!(
                    "Contraseña incorrecta (Intento {} de 5): {}",
                    attempts, e
                ))
            }
        }
    }

    pub async fn unlock_with_refresh(&self) -> Result<AdminProfile, String> {
        let _ = self.refresh_session().await?;
        self.failed_unlock_attempts
            .store(0, std::sync::atomic::Ordering::SeqCst);
        let profile = self
            .current_user
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
            .ok_or_else(|| "No user profile available after refresh".to_string())?;
        tracing::info!("Workspace unlocked successfully via refresh token / biometrics");
        Ok(profile)
    }

    pub fn logout(&self) {
        if let Ok(mut token_lock) = self.access_token.write() {
            if let Some(ref mut token) = *token_lock {
                use zeroize::Zeroize;
                token.zeroize();
            }
            *token_lock = None;
        }
        *self.current_user.write().unwrap_or_else(|p| p.into_inner()) = None;
        self.failed_unlock_attempts
            .store(0, std::sync::atomic::Ordering::SeqCst);
        let _ = crate::keystore::delete_admin_refresh_token();
    }

    async fn send_with_auth(
        &self,
        method: reqwest::Method,
        endpoint: &str,
        body: Option<serde_json::Value>,
    ) -> Result<reqwest::Response, String> {
        let base = self.get_base_url();
        let url = format!("{}{}", base.trim_end_matches('/'), endpoint);
        let token = self
            .access_token
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
            .unwrap_or_default();

        let mut req = self
            .client
            .request(method.clone(), &url)
            .header(APP_VERSION_HEADER, APP_VERSION)
            .bearer_auth(&token);

        if let Some(ref b) = body {
            req = req.json(b);
        }

        let res = req.send().await.map_err(|e| e.to_string())?;

        if res.status() == StatusCode::UNAUTHORIZED {
            tracing::info!("Received 401 on admin endpoint, attempting refresh...");
            if let Ok(new_token) = self.refresh_session().await {
                let mut retry_req = self
                    .client
                    .request(method, &url)
                    .header(APP_VERSION_HEADER, APP_VERSION)
                    .bearer_auth(&new_token);
                if let Some(ref b) = body {
                    retry_req = retry_req.json(b);
                }
                let retry = retry_req.send().await.map_err(|e| e.to_string())?;
                if retry.status() == StatusCode::UNAUTHORIZED {
                    self.logout();
                    return Err("Session expired or unauthorized".to_string());
                }
                return Ok(retry);
            }
            return Err("Session expired or unauthorized".to_string());
        }

        Ok(res)
    }

    pub async fn get_with_auth(&self, endpoint: &str) -> Result<reqwest::Response, String> {
        self.send_with_auth(reqwest::Method::GET, endpoint, None)
            .await
    }

    pub async fn post_with_auth<B: Serialize>(
        &self,
        endpoint: &str,
        body: &B,
    ) -> Result<reqwest::Response, String> {
        let val = serde_json::to_value(body).map_err(|e| e.to_string())?;
        self.send_with_auth(reqwest::Method::POST, endpoint, Some(val))
            .await
    }

    pub async fn put_with_auth<B: Serialize>(
        &self,
        endpoint: &str,
        body: &B,
    ) -> Result<reqwest::Response, String> {
        let val = serde_json::to_value(body).map_err(|e| e.to_string())?;
        self.send_with_auth(reqwest::Method::PUT, endpoint, Some(val))
            .await
    }

    pub async fn patch_with_auth<B: Serialize>(
        &self,
        endpoint: &str,
        body: &B,
    ) -> Result<reqwest::Response, String> {
        let val = serde_json::to_value(body).map_err(|e| e.to_string())?;
        self.send_with_auth(reqwest::Method::PATCH, endpoint, Some(val))
            .await
    }

    pub async fn delete_with_auth(&self, endpoint: &str) -> Result<reqwest::Response, String> {
        self.send_with_auth(reqwest::Method::DELETE, endpoint, None)
            .await
    }

    pub async fn get_admin_me(&self) -> Result<AdminMeResponse, String> {
        let resp = self.get_with_auth("/admins/me").await?;
        if !resp.status().is_success() {
            return Err(format!("Failed to fetch /admins/me: {}", resp.status()));
        }
        let wrapped: SingleResponse<AdminMeResponse> =
            resp.json().await.map_err(|e| e.to_string())?;
        Ok(wrapped.data)
    }

    pub async fn register_public_key(
        &self,
        public_key: &str,
    ) -> Result<RegisterPublicKeyResponse, String> {
        let body = serde_json::json!({ "public_key": public_key });
        let resp = self.put_with_auth("/admins/me/public-key", &body).await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!(
                "Failed to register public key ({}): {}",
                status, text
            ));
        }
        let wrapped: SingleResponse<RegisterPublicKeyResponse> =
            resp.json().await.map_err(|e| e.to_string())?;
        Ok(wrapped.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_admin_login_sends_role_admin_and_reads_data_envelope() {
        // login() persists the refresh token: isolate from the real OS keystore
        // and from other tests sharing the in-memory vault.
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;

        // Mock login endpoint checking role: ADMIN
        Mock::given(method("POST"))
            .and(path("/auth/login"))
            .and(header(APP_VERSION_HEADER, APP_VERSION))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "access_token": "mock_access_token_xyz",
                    "refresh_token": "mock_refresh_token_abc",
                    "token_type": "Bearer",
                    "expires_in": 900,
                    "user": {
                        "id": "admin-uuid-1",
                        "email": "ana@acme.cl",
                        "name": "Ana Admin",
                        "role": "ADMIN"
                    }
                }
            })))
            .mount(&server)
            .await;

        // Mock /admins/me
        Mock::given(method("GET"))
            .and(path("/admins/me"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "id": "admin-uuid-1",
                    "email": "ana@acme.cl",
                    "name": "Ana Admin",
                    "organization": "Acme Global",
                    "is_active": true,
                    "worker_count": 5,
                    "public_key": null,
                    "public_key_fingerprint": null,
                    "created_at": "2026-09-28T00:00:00Z",
                    "updated_at": "2026-09-28T00:00:00Z"
                }
            })))
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        let profile = client
            .login("ana@acme.cl", "Secret123!")
            .await
            .expect("Login failed");

        assert_eq!(profile.id, "admin-uuid-1");
        assert_eq!(profile.email, "ana@acme.cl");
        assert_eq!(profile.name, "Ana Admin");
        assert_eq!(profile.organization, "Acme Global");
        assert!(profile.public_key.is_none());
    }

    #[tokio::test]
    async fn test_transparent_refresh_on_401() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();

        let server = MockServer::start().await;

        crate::keystore::save_admin_refresh_token("valid_initial_refresh_token").unwrap();

        Mock::given(method("GET"))
            .and(path("/workers"))
            .and(header("Authorization", "Bearer expired_old_token"))
            .respond_with(ResponseTemplate::new(401))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/auth/refresh"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "access_token": "brand_new_access_token",
                    "refresh_token": "brand_new_refresh_token",
                    "token_type": "Bearer",
                    "expires_in": 900,
                    "user": {
                        "id": "admin-1",
                        "email": "admin@acme.cl",
                        "name": "Admin",
                        "role": "ADMIN"
                    }
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/workers"))
            .and(header("Authorization", "Bearer brand_new_access_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": []
            })))
            .expect(1)
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("expired_old_token".to_string()));

        let resp = client
            .get_with_auth("/workers")
            .await
            .expect("Request should succeed after auto-refresh");
        assert_eq!(resp.status(), 200);

        assert_eq!(
            client.get_access_token_for_test(),
            Some("brand_new_access_token".to_string())
        );
        assert_eq!(
            crate::keystore::load_admin_refresh_token().unwrap(),
            "brand_new_refresh_token"
        );
    }

    #[tokio::test]
    async fn test_refresh_failure_revokes_session() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();

        let server = MockServer::start().await;
        crate::keystore::save_admin_refresh_token("revoked_refresh_token").unwrap();

        Mock::given(method("GET"))
            .and(path("/workers"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/auth/refresh"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": {
                    "code": "INVALID_TOKEN",
                    "message": "Token family revoked"
                }
            })))
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("expired_token".to_string()));

        let result = client.get_with_auth("/workers").await;
        assert!(result.is_err());
        assert!(client.get_access_token_for_test().is_none());
        assert!(crate::keystore::load_admin_refresh_token().is_err());
    }
}
