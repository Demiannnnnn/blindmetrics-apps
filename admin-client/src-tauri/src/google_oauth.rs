//! Google sign-in for the desktop admin app (TASK-ADMIN-10, SEC-16).
//!
//! RFC 8252 installed-app flow: the system browser shows Google's page, the redirect comes back
//! to `http://127.0.0.1:<random port>` and PKCE (S256) binds the code to this process. `state`
//! and `nonce` are random per attempt and checked. The ID token is only forwarded to the API,
//! which verifies its signature (Google JWKS), `aud`, `iss`, `exp` and `email_verified`.
//!
//! Everything but the socket and the browser launch is a pure function, tested without network.

use crate::commands::CommandError;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::time::Duration;
use subtle::ConstantTimeEq;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use zeroize::Zeroizing;

pub const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
/// How long the app waits for the browser to come back.
pub const REDIRECT_TIMEOUT: Duration = Duration::from_secs(180);
/// Maximum size of the browser's request head on the loopback socket.
const MAX_REQUEST_HEAD: usize = 8 * 1024;
const CONNECTION_READ_TIMEOUT: Duration = Duration::from_secs(10);
/// Same limit as the API for `id_token`.
const MAX_ID_TOKEN_LEN: usize = 4096;
const SCOPES: &str = "openid email profile";

/// Unreserved characters of RFC 3986 stay as-is in query values.
const QUERY_VALUE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// OAuth client of type "Desktop app". Its secret is not confidential by Google's design but
/// the token endpoint requires it; it is still never logged.
#[derive(Clone)]
pub struct GoogleConfig {
    pub client_id: String,
    pub client_secret: Option<Zeroizing<String>>,
}

impl std::fmt::Debug for GoogleConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleConfig")
            .field("client_id", &self.client_id)
            .finish_non_exhaustive()
    }
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

impl GoogleConfig {
    /// `ADMIN_GOOGLE_CLIENT_ID` / `ADMIN_GOOGLE_CLIENT_SECRET` from the environment, falling
    /// back to values baked in at compile time (RF-ADMIN-10-1). `None` disables Google.
    pub fn from_env() -> Option<Self> {
        let client_id = std::env::var("ADMIN_GOOGLE_CLIENT_ID")
            .ok()
            .and_then(non_empty)
            .or_else(|| {
                option_env!("ADMIN_GOOGLE_CLIENT_ID")
                    .map(str::to_string)
                    .and_then(non_empty)
            })?;
        let client_secret = std::env::var("ADMIN_GOOGLE_CLIENT_SECRET")
            .ok()
            .and_then(non_empty)
            .or_else(|| {
                option_env!("ADMIN_GOOGLE_CLIENT_SECRET")
                    .map(str::to_string)
                    .and_then(non_empty)
            })
            .map(Zeroizing::new);
        Some(Self {
            client_id,
            client_secret,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GoogleError {
    #[error("Google sign-in is not configured in this build")]
    NotConfigured,
    #[error("The user cancelled the Google sign-in")]
    Cancelled,
    #[error("The browser did not come back in time")]
    Timeout,
    #[error("The redirect state does not match")]
    StateMismatch,
    #[error("Invalid redirect from Google: {0}")]
    InvalidRedirect(String),
    #[error("Google token exchange failed")]
    TokenExchange,
    #[error("The ID token nonce does not match")]
    NonceMismatch,
    #[error("The system browser could not be opened")]
    Browser,
    #[error("Loopback socket error: {0}")]
    Io(String),
}

impl GoogleError {
    pub fn code(&self) -> &'static str {
        match self {
            GoogleError::NotConfigured => "GOOGLE_NOT_CONFIGURED",
            GoogleError::Cancelled => "GOOGLE_SIGN_IN_CANCELLED",
            GoogleError::Timeout => "GOOGLE_SIGN_IN_TIMEOUT",
            GoogleError::StateMismatch => "GOOGLE_STATE_MISMATCH",
            GoogleError::InvalidRedirect(_) => "GOOGLE_INVALID_REDIRECT",
            GoogleError::TokenExchange => "GOOGLE_TOKEN_EXCHANGE_FAILED",
            GoogleError::NonceMismatch => "GOOGLE_NONCE_MISMATCH",
            GoogleError::Browser => "BROWSER_UNAVAILABLE",
            GoogleError::Io(_) => "GOOGLE_LOOPBACK_ERROR",
        }
    }

    fn user_message(&self) -> &'static str {
        match self {
            GoogleError::NotConfigured => {
                "El inicio con Google no está configurado en esta versión."
            }
            GoogleError::Cancelled => "Cancelaste el inicio de sesión con Google.",
            GoogleError::Timeout => "El navegador no volvió a tiempo. Inténtalo de nuevo.",
            GoogleError::StateMismatch
            | GoogleError::InvalidRedirect(_)
            | GoogleError::NonceMismatch => {
                "La respuesta de Google no es válida. Inténtalo de nuevo."
            }
            GoogleError::TokenExchange => "Google no validó la sesión. Inténtalo de nuevo.",
            GoogleError::Browser => "No se pudo abrir el navegador del sistema.",
            GoogleError::Io(_) => "No se pudo preparar el inicio de sesión con Google.",
        }
    }
}

impl From<GoogleError> for CommandError {
    fn from(e: GoogleError) -> Self {
        tracing::warn!(error = %e, "google sign-in failed");
        CommandError::new(e.code(), e.user_message())
    }
}

/// PKCE verifier (43 chars of base64url) and its S256 challenge.
pub struct PkcePair {
    pub verifier: Zeroizing<String>,
    pub challenge: String,
}

/// `base64url(SHA-256(verifier))` without padding (RFC 7636 §4.2).
pub fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// 32 random bytes as base64url without padding.
pub fn random_token() -> Zeroizing<String> {
    let mut bytes = Zeroizing::new([0u8; 32]);
    OsRng.fill_bytes(bytes.as_mut());
    Zeroizing::new(URL_SAFE_NO_PAD.encode(bytes.as_ref()))
}

pub fn pkce_pair() -> PkcePair {
    let verifier = random_token();
    let challenge = pkce_challenge(&verifier);
    PkcePair {
        verifier,
        challenge,
    }
}

fn encode(value: &str) -> String {
    utf8_percent_encode(value, QUERY_VALUE).to_string()
}

/// Authorization request URL (RF-ADMIN-10-2 step 3).
pub fn build_authorization_url(
    auth_url: &str,
    cfg: &GoogleConfig,
    redirect_uri: &str,
    state: &str,
    nonce: &str,
    code_challenge: &str,
) -> String {
    let params = [
        ("client_id", cfg.client_id.as_str()),
        ("redirect_uri", redirect_uri),
        ("response_type", "code"),
        ("scope", SCOPES),
        ("code_challenge", code_challenge),
        ("code_challenge_method", "S256"),
        ("state", state),
        ("nonce", nonce),
        ("prompt", "select_account"),
    ];
    let query = params
        .iter()
        .map(|(k, v)| format!("{k}={}", encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{auth_url}?{query}")
}

/// What the browser sent to the loopback socket.
#[derive(Debug, PartialEq, Eq)]
pub enum RedirectOutcome {
    Code {
        code: String,
        state: String,
    },
    Error(String),
    /// Another path (`/favicon.ico`) or a request without the expected parameters.
    NotCallback,
}

fn decode_component(raw: &str) -> String {
    percent_decode_str(&raw.replace('+', " "))
        .decode_utf8_lossy()
        .to_string()
}

/// Parses the request line of the browser's request (`GET /?code=…&state=… HTTP/1.1`).
pub fn parse_redirect_request(head: &str) -> RedirectOutcome {
    let Some(request_line) = head.lines().next() else {
        return RedirectOutcome::NotCallback;
    };
    let mut parts = request_line.split_whitespace();
    let (Some(method), Some(target)) = (parts.next(), parts.next()) else {
        return RedirectOutcome::NotCallback;
    };
    if method != "GET" {
        return RedirectOutcome::NotCallback;
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    if path != "/" {
        return RedirectOutcome::NotCallback;
    }
    let mut code = None;
    let mut state = None;
    let mut error = None;
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        match key {
            "code" => code = Some(decode_component(value)),
            "state" => state = Some(decode_component(value)),
            "error" => error = Some(decode_component(value)),
            _ => {}
        }
    }
    if let Some(error) = error {
        return RedirectOutcome::Error(error);
    }
    match (code, state) {
        (Some(code), Some(state)) if !code.is_empty() => RedirectOutcome::Code { code, state },
        _ => RedirectOutcome::NotCallback,
    }
}

const PAGE_DONE: &str = "<!doctype html><html lang=\"es\"><head><meta charset=\"utf-8\"><title>Telemetry Admin</title></head><body style=\"font-family:sans-serif;background:#0f172a;color:#e2e8f0;text-align:center;padding-top:15vh\"><h1>Listo</h1><p>Ya puedes volver a Telemetry Admin.</p></body></html>";
const PAGE_FAILED: &str = "<!doctype html><html lang=\"es\"><head><meta charset=\"utf-8\"><title>Telemetry Admin</title></head><body style=\"font-family:sans-serif;background:#0f172a;color:#e2e8f0;text-align:center;padding-top:15vh\"><h1>No se completó el inicio de sesión</h1><p>Vuelve a Telemetry Admin e inténtalo de nuevo.</p></body></html>";

async fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

async fn read_head(stream: &mut TcpStream) -> Option<String> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    let read = async {
        loop {
            let n = stream.read(&mut chunk).await.ok()?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
            if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
            if buf.len() > MAX_REQUEST_HEAD {
                return None;
            }
        }
        Some(())
    };
    tokio::time::timeout(CONNECTION_READ_TIMEOUT, read)
        .await
        .ok()??;
    String::from_utf8(buf).ok()
}

fn states_match(received: &str, expected: &str) -> bool {
    received.len() == expected.len() && bool::from(received.as_bytes().ct_eq(expected.as_bytes()))
}

/// Waits for the single valid redirect (RF-ADMIN-10-2 step 4) and returns the code.
pub async fn wait_for_redirect(
    listener: TcpListener,
    expected_state: &str,
    timeout: Duration,
) -> Result<Zeroizing<String>, GoogleError> {
    let wait = async {
        loop {
            let (mut stream, _) = listener
                .accept()
                .await
                .map_err(|e| GoogleError::Io(e.to_string()))?;
            let Some(head) = read_head(&mut stream).await else {
                continue;
            };
            match parse_redirect_request(&head) {
                RedirectOutcome::NotCallback => {
                    respond(&mut stream, "404 Not Found", "").await;
                }
                RedirectOutcome::Error(error) => {
                    respond(&mut stream, "200 OK", PAGE_FAILED).await;
                    return Err(if error == "access_denied" {
                        GoogleError::Cancelled
                    } else {
                        GoogleError::InvalidRedirect(error)
                    });
                }
                RedirectOutcome::Code { code, state } => {
                    if !states_match(&state, expected_state) {
                        respond(&mut stream, "400 Bad Request", PAGE_FAILED).await;
                        return Err(GoogleError::StateMismatch);
                    }
                    respond(&mut stream, "200 OK", PAGE_DONE).await;
                    return Ok(Zeroizing::new(code));
                }
            }
        }
    };
    tokio::time::timeout(timeout, wait)
        .await
        .map_err(|_| GoogleError::Timeout)?
}

#[derive(serde::Deserialize)]
struct TokenResponse {
    id_token: Option<String>,
}

/// Exchanges the authorization code for an ID token (RF-ADMIN-10-2 step 5).
pub async fn exchange_code(
    http: &reqwest::Client,
    token_url: &str,
    cfg: &GoogleConfig,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<Zeroizing<String>, GoogleError> {
    let mut form = vec![
        ("code", code),
        ("client_id", cfg.client_id.as_str()),
        ("redirect_uri", redirect_uri),
        ("grant_type", "authorization_code"),
        ("code_verifier", code_verifier),
    ];
    if let Some(secret) = &cfg.client_secret {
        form.push(("client_secret", secret.as_str()));
    }
    let resp = http
        .post(token_url)
        .form(&form)
        .send()
        .await
        .map_err(|_| GoogleError::TokenExchange)?;
    if !resp.status().is_success() {
        tracing::warn!(status = %resp.status(), "google token endpoint rejected the code");
        return Err(GoogleError::TokenExchange);
    }
    let body: TokenResponse = resp.json().await.map_err(|_| GoogleError::TokenExchange)?;
    let id_token = Zeroizing::new(body.id_token.ok_or(GoogleError::TokenExchange)?);
    if id_token.len() > MAX_ID_TOKEN_LEN || id_token.split('.').count() != 3 {
        return Err(GoogleError::TokenExchange);
    }
    Ok(id_token)
}

/// Checks the `nonce` claim **without** verifying the signature (the API verifies the token).
pub fn check_nonce(id_token: &str, expected_nonce: &str) -> Result<(), GoogleError> {
    let payload = id_token
        .split('.')
        .nth(1)
        .ok_or(GoogleError::NonceMismatch)?;
    let decoded = URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .map_err(|_| GoogleError::NonceMismatch)?;
    let claims: serde_json::Value =
        serde_json::from_slice(&decoded).map_err(|_| GoogleError::NonceMismatch)?;
    let nonce = claims
        .get("nonce")
        .and_then(|n| n.as_str())
        .ok_or(GoogleError::NonceMismatch)?;
    if states_match(nonce, expected_nonce) {
        Ok(())
    } else {
        Err(GoogleError::NonceMismatch)
    }
}

/// Endpoints and timing of a sign-in (constants in production, local mocks in tests).
pub struct SignInEndpoints<'a> {
    pub auth_url: &'a str,
    pub token_url: &'a str,
    pub timeout: Duration,
}

impl Default for SignInEndpoints<'static> {
    fn default() -> Self {
        Self {
            auth_url: GOOGLE_AUTH_URL,
            token_url: GOOGLE_TOKEN_URL,
            timeout: REDIRECT_TIMEOUT,
        }
    }
}

/// Complete browser round-trip; returns a Google ID token whose nonce was checked.
pub async fn sign_in(
    http: &reqwest::Client,
    cfg: &GoogleConfig,
    endpoints: &SignInEndpoints<'_>,
    open_browser: impl FnOnce(&str) -> Result<(), GoogleError>,
) -> Result<Zeroizing<String>, GoogleError> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|e| GoogleError::Io(e.to_string()))?;
    let port = listener
        .local_addr()
        .map_err(|e| GoogleError::Io(e.to_string()))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}");
    let pkce = pkce_pair();
    let state = random_token();
    let nonce = random_token();
    let url = build_authorization_url(
        endpoints.auth_url,
        cfg,
        &redirect_uri,
        &state,
        &nonce,
        &pkce.challenge,
    );
    tracing::info!(port, "google sign-in started");
    open_browser(&url)?;
    let code = wait_for_redirect(listener, &state, endpoints.timeout).await?;
    let id_token = exchange_code(
        http,
        endpoints.token_url,
        cfg,
        &code,
        &redirect_uri,
        &pkce.verifier,
    )
    .await?;
    check_nonce(&id_token, &nonce)?;
    Ok(id_token)
}

/// Opens `url` in the system browser.
pub fn open_system_browser(url: &str) -> Result<(), GoogleError> {
    open::that_detached(url).map_err(|e| {
        tracing::warn!("system browser unavailable: {}", e);
        GoogleError::Browser
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    fn cfg() -> GoogleConfig {
        GoogleConfig {
            client_id: "desktop-client.apps.googleusercontent.com".into(),
            client_secret: Some(Zeroizing::new("not-really-secret".into())),
        }
    }

    fn id_token_with_nonce(nonce: &str) -> String {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","kid":"k1"}"#);
        let claims = URL_SAFE_NO_PAD.encode(
            serde_json::json!({ "sub": "1", "email": "ana@acme.cl", "nonce": nonce }).to_string(),
        );
        format!("{header}.{claims}.c2lnbmF0dXJl")
    }

    fn query_param(url: &str, key: &str) -> Option<String> {
        let query = url.split_once('?')?.1;
        query.split('&').find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            (k == key).then(|| decode_component(v))
        })
    }

    #[test]
    fn test_google_pkce_matches_rfc7636_vector() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let pair = pkce_pair();
        assert_eq!(pair.verifier.len(), 43);
        assert_eq!(pair.challenge, pkce_challenge(&pair.verifier));
        assert_ne!(random_token().as_str(), random_token().as_str());
    }

    #[test]
    fn test_google_authorization_url_contains_pkce_state_nonce() {
        let url = build_authorization_url(
            GOOGLE_AUTH_URL,
            &cfg(),
            "http://127.0.0.1:53682",
            "st-1",
            "no-1",
            "ch-1",
        );
        assert!(url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?"));
        assert_eq!(
            query_param(&url, "redirect_uri").unwrap(),
            "http://127.0.0.1:53682"
        );
        assert_eq!(query_param(&url, "code_challenge_method").unwrap(), "S256");
        assert_eq!(query_param(&url, "code_challenge").unwrap(), "ch-1");
        assert_eq!(query_param(&url, "state").unwrap(), "st-1");
        assert_eq!(query_param(&url, "nonce").unwrap(), "no-1");
        assert_eq!(query_param(&url, "scope").unwrap(), "openid email profile");
        assert_eq!(query_param(&url, "response_type").unwrap(), "code");
        assert!(
            !url.contains("not-really-secret"),
            "the secret never goes to the browser"
        );
    }

    #[test]
    fn test_google_parse_redirect_code_error_and_other_paths() {
        assert_eq!(
            parse_redirect_request(
                "GET /?state=abc&code=4%2F0Ad-x&scope=openid HTTP/1.1\r\nHost: x\r\n\r\n"
            ),
            RedirectOutcome::Code {
                code: "4/0Ad-x".into(),
                state: "abc".into()
            }
        );
        assert_eq!(
            parse_redirect_request("GET /?error=access_denied&state=abc HTTP/1.1\r\n\r\n"),
            RedirectOutcome::Error("access_denied".into())
        );
        assert_eq!(
            parse_redirect_request("GET /favicon.ico HTTP/1.1\r\n\r\n"),
            RedirectOutcome::NotCallback
        );
        assert_eq!(
            parse_redirect_request("POST /?code=a&state=b HTTP/1.1\r\n\r\n"),
            RedirectOutcome::NotCallback
        );
        assert_eq!(
            parse_redirect_request("GET /?code=&state=b HTTP/1.1\r\n\r\n"),
            RedirectOutcome::NotCallback
        );
        assert_eq!(parse_redirect_request(""), RedirectOutcome::NotCallback);
    }

    async fn browser_get(port: u16, target: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        stream
            .write_all(format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();
        response
    }

    #[tokio::test]
    async fn test_google_wait_for_redirect_accepts_matching_state() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let browser = tokio::spawn(async move {
            let favicon = browser_get(port, "/favicon.ico").await;
            assert!(favicon.starts_with("HTTP/1.1 404"));
            browser_get(port, "/?code=the-code&state=expected").await
        });
        let code = wait_for_redirect(listener, "expected", Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(code.as_str(), "the-code");
        let page = browser.await.unwrap();
        assert!(page.starts_with("HTTP/1.1 200"));
        assert!(page.contains("Ya puedes volver a Telemetry Admin"));
        assert!(page.contains("no-store"));
    }

    #[tokio::test]
    async fn test_google_wait_for_redirect_rejects_state_mismatch_and_times_out() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let browser = tokio::spawn(async move { browser_get(port, "/?code=c&state=forged").await });
        let err = wait_for_redirect(listener, "expected", Duration::from_secs(5))
            .await
            .unwrap_err();
        assert_eq!(err, GoogleError::StateMismatch);
        assert!(browser.await.unwrap().starts_with("HTTP/1.1 400"));

        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let browser =
            tokio::spawn(async move { browser_get(port, "/?error=access_denied&state=x").await });
        let err = wait_for_redirect(listener, "expected", Duration::from_secs(5))
            .await
            .unwrap_err();
        assert_eq!(err, GoogleError::Cancelled);
        browser.await.unwrap();

        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let err = wait_for_redirect(listener, "expected", Duration::from_millis(100))
            .await
            .unwrap_err();
        assert_eq!(err, GoogleError::Timeout);
    }

    #[tokio::test]
    async fn test_google_exchange_code_posts_verifier() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains("code_verifier=verifier-123"))
            .and(body_string_contains("grant_type=authorization_code"))
            .and(body_string_contains("client_secret=not-really-secret"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "ya29", "id_token": id_token_with_nonce("n"), "expires_in": 3599
            })))
            .expect(1)
            .mount(&server)
            .await;
        let http = reqwest::Client::new();
        let token = exchange_code(
            &http,
            &format!("{}/token", server.uri()),
            &cfg(),
            "code-1",
            "http://127.0.0.1:1",
            "verifier-123",
        )
        .await
        .unwrap();
        assert_eq!(token.split('.').count(), 3);

        let bad = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": "invalid_grant"
            })))
            .mount(&bad)
            .await;
        assert_eq!(
            exchange_code(&http, &bad.uri(), &cfg(), "c", "r", "v")
                .await
                .unwrap_err(),
            GoogleError::TokenExchange
        );
    }

    #[test]
    fn test_google_nonce_mismatch_rejected() {
        assert!(check_nonce(&id_token_with_nonce("n-1"), "n-1").is_ok());
        assert_eq!(
            check_nonce(&id_token_with_nonce("n-1"), "n-2").unwrap_err(),
            GoogleError::NonceMismatch
        );
        assert_eq!(
            check_nonce("not-a-jwt", "n").unwrap_err(),
            GoogleError::NonceMismatch
        );
    }

    #[tokio::test]
    async fn test_google_sign_in_full_loopback_flow() {
        let token_server = MockServer::start().await;
        let seen_nonce: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let nonce_for_token = seen_nonce.clone();
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(move |_req: &Request| {
                let nonce = nonce_for_token.lock().unwrap().clone().unwrap_or_default();
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "id_token": id_token_with_nonce(&nonce) }))
            })
            .expect(1)
            .mount(&token_server)
            .await;

        let token_url = format!("{}/token", token_server.uri());
        let endpoints = SignInEndpoints {
            auth_url: "https://accounts.example.test/auth",
            token_url: &token_url,
            timeout: Duration::from_secs(5),
        };
        let fake_browser = |url: &str| {
            let redirect = query_param(url, "redirect_uri").unwrap();
            let state = query_param(url, "state").unwrap();
            *seen_nonce.lock().unwrap() = query_param(url, "nonce");
            let port: u16 = redirect.rsplit(':').next().unwrap().parse().unwrap();
            tokio::spawn(async move {
                browser_get(port, &format!("/?code=abc&state={state}")).await;
            });
            Ok(())
        };
        let id_token = sign_in(&reqwest::Client::new(), &cfg(), &endpoints, fake_browser)
            .await
            .unwrap();
        assert_eq!(id_token.split('.').count(), 3);
    }
}
