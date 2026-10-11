//! HTTP client of the admin app (TASK-INT-01, TASK-INT-02).
//!
//! * Access token only in RAM (`Zeroizing`), refresh token only in the OS keystore (SEC-05).
//! * **Single-flight refresh** (SEC-14): the API revokes the whole token family when a refresh
//!   token is presented twice, so concurrent 401s wait for one refresh and reuse its token.
//! * Typed [`ApiError`] from the canonical `{ error: { code, message, details } }` envelope.
//! * `401 DEVICE_REVOKED` expires the session without refreshing; `402`/`403 SUBSCRIPTION_*`
//!   notify the UI (`subscription-blocked`, TASK-ADMIN-12); `X-Trial-Days-Left` is recorded.

use reqwest::header::RETRY_AFTER;
use reqwest::{Client, Method, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use zeroize::Zeroizing;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_VERSION_HEADER: &str = "X-App-Version";
pub const TRIAL_DAYS_HEADER: &str = "x-trial-days-left";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
/// Largest `Retry-After` honoured (24 h); anything else is ignored.
const MAX_RETRY_AFTER_SECONDS: u64 = 86_400;

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
    #[serde(default)]
    pub is_new_user: bool,
}

#[derive(Deserialize, Clone, PartialEq)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub user: UserDto,
}

impl std::fmt::Debug for AuthResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print tokens (security-model §7).
        f.debug_struct("AuthResponse")
            .field("user", &self.user)
            .finish_non_exhaustive()
    }
}

fn default_true() -> bool {
    true
}

/// `GET /admins/me` (api-contract §3).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AdminMeResponse {
    pub id: String,
    pub email: String,
    pub name: String,
    #[serde(default)]
    pub organization: String,
    #[serde(default)]
    pub public_key: Option<String>,
    #[serde(default)]
    pub public_key_fingerprint: Option<String>,
    #[serde(default = "default_true")]
    pub has_password: bool,
    #[serde(default)]
    pub google_linked: bool,
    #[serde(default)]
    pub key_backup_enabled: bool,
    #[serde(default)]
    pub onboarding_completed: bool,
}

/// Profile exposed to the UI. Never contains tokens or keys other than the public key.
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
    #[serde(default = "default_true")]
    pub has_password: bool,
    #[serde(default)]
    pub google_linked: bool,
    #[serde(default)]
    pub key_backup_enabled: bool,
    #[serde(default)]
    pub onboarding_completed: bool,
    #[serde(default)]
    pub is_new_user: bool,
    #[serde(default)]
    pub trial_days_left: Option<i64>,
}

impl AdminProfile {
    pub fn from_me(me: AdminMeResponse, is_new_user: bool) -> Self {
        Self {
            id: me.id,
            email: me.email,
            name: me.name,
            organization: me.organization,
            public_key: me.public_key,
            public_key_fingerprint: me.public_key_fingerprint,
            has_password: me.has_password,
            google_linked: me.google_linked,
            key_backup_enabled: me.key_backup_enabled,
            onboarding_completed: me.onboarding_completed,
            is_new_user,
            trial_days_left: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RegisterPublicKeyResponse {
    pub public_key: String,
    pub public_key_fingerprint: String,
    #[serde(default)]
    pub public_key_registered_at: Option<String>,
}

/// Typed API error (RF-INT-02-1).
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ApiError {
    /// HTTP status; `0` for network/local errors.
    pub status: u16,
    /// `error.code` of the API or a local code (`NETWORK_ERROR`, `NOT_AUTHENTICATED`, …).
    pub code: String,
    pub message: String,
    /// `error.details.reason`.
    pub reason: Option<String>,
    /// Raw `error.details` (filtered with [`ApiError::safe_details`] before reaching the UI).
    pub details: Option<Box<serde_json::Value>>,
    pub retry_after_seconds: Option<u64>,
}

/// `details` keys that may cross the IPC boundary (RF-INT-02-2).
const SAFE_DETAIL_KEYS: [&str; 10] = [
    "reason",
    "portal_url",
    "support_email",
    "subscription_status",
    "retry_after_seconds",
    "hint",
    "status",
    "current_consent_version",
    "categories",
    "suspended_at",
];

impl ApiError {
    pub fn local(code: &str, message: impl Into<String>) -> Self {
        Self {
            status: 0,
            code: code.to_string(),
            message: message.into(),
            reason: None,
            details: None,
            retry_after_seconds: None,
        }
    }

    pub fn network(err: impl std::fmt::Display) -> Self {
        tracing::warn!("API request failed: {}", err);
        Self::local(
            "NETWORK_ERROR",
            "No se pudo contactar al servidor. Verifica la dirección y tu conexión.",
        )
    }

    pub fn deserialization(err: impl std::fmt::Display) -> Self {
        tracing::error!("Unexpected API response shape: {}", err);
        Self::local(
            "DESERIALIZATION_ERROR",
            "La respuesta del servidor no tiene el formato esperado.",
        )
    }

    /// Builds the error from a non-success response (canonical envelope or anything else).
    pub async fn from_response(resp: reqwest::Response) -> Self {
        let status = resp.status().as_u16();
        let retry_header = resp
            .headers()
            .get(RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .filter(|s| *s <= MAX_RETRY_AFTER_SECONDS);
        let bytes = resp.bytes().await.unwrap_or_default();
        Self::from_parts(status, retry_header, &bytes)
    }

    pub fn from_parts(status: u16, retry_header: Option<u64>, body: &[u8]) -> Self {
        let parsed: Option<serde_json::Value> = serde_json::from_slice(body).ok();
        let Some(err) = parsed.as_ref().and_then(|b| b.get("error")) else {
            return Self {
                status,
                code: format!("HTTP_{status}"),
                message: generic_message(status).to_string(),
                reason: None,
                details: None,
                retry_after_seconds: retry_header,
            };
        };
        let details = err
            .get("details")
            .filter(|d| d.is_object())
            .cloned()
            .map(Box::new);
        let reason = details
            .as_deref()
            .and_then(|d| d.get("reason"))
            .and_then(|r| r.as_str())
            .map(str::to_string);
        let retry_detail = details
            .as_deref()
            .and_then(|d| d.get("retry_after_seconds"))
            .and_then(|r| r.as_u64())
            .filter(|s| *s <= MAX_RETRY_AFTER_SECONDS);
        Self {
            status,
            code: err
                .get("code")
                .and_then(|c| c.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("HTTP_{status}")),
            message: err
                .get("message")
                .and_then(|m| m.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| generic_message(status).to_string()),
            reason,
            details,
            retry_after_seconds: retry_header.or(retry_detail),
        }
    }

    pub fn is_subscription_block(&self) -> bool {
        matches!(
            self.code.as_str(),
            "SUBSCRIPTION_SUSPENDED" | "SUBSCRIPTION_CANCELLED"
        )
    }

    pub fn has_reason(&self, reason: &str) -> bool {
        self.reason.as_deref() == Some(reason)
    }

    /// Whitelisted `details` for the UI (never raw server bodies).
    pub fn safe_details(&self) -> Option<Box<serde_json::Value>> {
        let details = self.details.as_deref()?.as_object()?;
        let mut safe = serde_json::Map::new();
        for key in SAFE_DETAIL_KEYS {
            if let Some(value) = details.get(key) {
                safe.insert(key.to_string(), value.clone());
            }
        }
        if let Some(retry) = self.retry_after_seconds {
            safe.insert("retry_after_seconds".into(), retry.into());
        }
        (!safe.is_empty()).then(|| Box::new(serde_json::Value::Object(safe)))
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.reason {
            Some(reason) => write!(f, "{} ({}): {}", self.code, reason, self.message),
            None => write!(f, "{}: {}", self.code, self.message),
        }
    }
}

impl std::error::Error for ApiError {}

fn generic_message(status: u16) -> &'static str {
    match status {
        400 => "Solicitud inválida.",
        401 => "La sesión no es válida.",
        402 => "La suscripción está suspendida.",
        403 => "No tienes permiso para esta acción.",
        404 => "Recurso no encontrado.",
        409 => "La acción entra en conflicto con el estado actual.",
        413 => "El contenido es demasiado grande.",
        415 => "Formato de contenido no soportado.",
        429 => "Demasiadas solicitudes; espera un momento.",
        500..=599 => "El servidor tuvo un problema; inténtalo más tarde.",
        _ => "Error inesperado del servidor.",
    }
}

/// Payload of the `subscription-blocked` event (TASK-ADMIN-12).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SubscriptionBlock {
    /// `SUSPENDED` | `CANCELLED`.
    pub status: String,
    pub reason: Option<String>,
    pub portal_url: Option<String>,
    pub support_email: Option<String>,
}

impl SubscriptionBlock {
    pub fn from_error(err: &ApiError) -> Option<Self> {
        if !err.is_subscription_block() {
            return None;
        }
        let detail = |key: &str| {
            err.details
                .as_deref()
                .and_then(|d| d.get(key))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        };
        let status = if err.code == "SUBSCRIPTION_CANCELLED" {
            "CANCELLED"
        } else {
            "SUSPENDED"
        };
        Some(Self {
            status: status.to_string(),
            reason: err.reason.clone(),
            portal_url: detail("portal_url"),
            support_email: detail("support_email"),
        })
    }
}

/// Server-provided billing portal data (the only URLs `open_billing_portal` may open, SEC-17).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PortalInfo {
    pub portal_url: Option<String>,
    pub support_email: Option<String>,
}

/// Sink for events the UI must react to (implemented with Tauri's emitter in `main.rs`).
pub trait ClientEvents: Send + Sync {
    fn session_expired(&self, reason: &str);
    fn subscription_blocked(&self, block: &SubscriptionBlock);
}

pub struct AdminApiClient {
    http: Client,
    base_url: RwLock<String>,
    access_token: RwLock<Option<Zeroizing<String>>>,
    current_user: RwLock<Option<AdminProfile>>,
    /// RF-INT-02-3 / SEC-14: one refresh at a time.
    refresh_lock: tokio::sync::Mutex<()>,
    trial_days_left: RwLock<Option<i64>>,
    portal: RwLock<PortalInfo>,
    events: RwLock<Option<Arc<dyn ClientEvents>>>,
    failed_unlock_attempts: AtomicUsize,
}

static API_CLIENT: std::sync::OnceLock<AdminApiClient> = std::sync::OnceLock::new();

/// Server the admin app is built for, fixed at compile time by `build.rs` from
/// `TELEMETRY_API_URL` (`admin-client/.env` or the environment). Never changed at runtime.
pub const API_BASE_URL: &str = env!("TELEMETRY_API_URL");

pub fn get_api_client() -> &'static AdminApiClient {
    API_CLIENT.get_or_init(|| AdminApiClient::new(API_BASE_URL.to_string()))
}

fn user_agent() -> String {
    format!("telemetry-admin/{APP_VERSION}")
}

impl AdminApiClient {
    pub fn new(base_url: String) -> Self {
        let http = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(user_agent())
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            http,
            base_url: RwLock::new(base_url.trim_end_matches('/').to_string()),
            access_token: RwLock::new(None),
            current_user: RwLock::new(None),
            refresh_lock: tokio::sync::Mutex::new(()),
            trial_days_left: RwLock::new(None),
            portal: RwLock::new(PortalInfo::default()),
            events: RwLock::new(None),
            failed_unlock_attempts: AtomicUsize::new(0),
        }
    }

    pub fn http(&self) -> &Client {
        &self.http
    }

    pub fn set_event_sink(&self, sink: Arc<dyn ClientEvents>) {
        *self.events.write().unwrap_or_else(|p| p.into_inner()) = Some(sink);
    }

    pub fn get_base_url(&self) -> String {
        self.base_url
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Joins the base URL (`…/api/v1`) and an endpoint starting with `/`.
    pub fn url(&self, endpoint: &str) -> String {
        format!(
            "{}/{}",
            self.get_base_url().trim_end_matches('/'),
            endpoint.trim_start_matches('/')
        )
    }

    pub fn get_current_user(&self) -> Option<AdminProfile> {
        let mut profile = self
            .current_user
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()?;
        profile.trial_days_left = self.trial_days_left();
        Some(profile)
    }

    pub fn set_current_user(&self, profile: Option<AdminProfile>) {
        *self.current_user.write().unwrap_or_else(|p| p.into_inner()) = profile;
    }

    /// Applies `f` to the cached profile, if any.
    pub fn update_current_user(&self, f: impl FnOnce(&mut AdminProfile)) {
        if let Some(profile) = self
            .current_user
            .write()
            .unwrap_or_else(|p| p.into_inner())
            .as_mut()
        {
            f(profile);
        }
    }

    pub fn set_current_user_public_key(&self, pubkey: String, fp: String) {
        self.update_current_user(|profile| {
            profile.public_key = Some(pubkey);
            profile.public_key_fingerprint = Some(fp);
            profile.onboarding_completed = !profile.organization.trim().is_empty();
        });
    }

    pub fn trial_days_left(&self) -> Option<i64> {
        *self
            .trial_days_left
            .read()
            .unwrap_or_else(|p| p.into_inner())
    }

    pub fn set_trial_days_left(&self, days: Option<i64>) {
        *self
            .trial_days_left
            .write()
            .unwrap_or_else(|p| p.into_inner()) = days;
    }

    /// Remembers the billing portal data received from the server.
    pub fn record_portal(&self, portal_url: Option<String>, support_email: Option<String>) {
        let mut portal = self.portal.write().unwrap_or_else(|p| p.into_inner());
        if portal_url.is_some() {
            portal.portal_url = portal_url;
        }
        if support_email.is_some() {
            portal.support_email = support_email;
        }
    }

    pub fn portal(&self) -> PortalInfo {
        self.portal
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    fn current_access_token(&self) -> Option<Zeroizing<String>> {
        self.access_token
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    fn set_access_token(&self, token: &str) {
        *self.access_token.write().unwrap_or_else(|p| p.into_inner()) =
            Some(Zeroizing::new(token.to_string()));
    }

    #[cfg(test)]
    pub fn set_access_token_for_test(&self, token: Option<String>) {
        *self.access_token.write().unwrap_or_else(|p| p.into_inner()) = token.map(Zeroizing::new);
    }

    #[cfg(test)]
    pub fn get_access_token_for_test(&self) -> Option<String> {
        self.current_access_token().map(|t| t.to_string())
    }

    fn emit_session_expired(&self, reason: &str) {
        let sink = self
            .events
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if let Some(sink) = sink {
            sink.session_expired(reason);
        }
    }

    fn emit_subscription_blocked(&self, block: &SubscriptionBlock) {
        let sink = self
            .events
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if let Some(sink) = sink {
            sink.subscription_blocked(block);
        }
    }

    /// Clears the local session and tells the UI why (RF-INT-02-3).
    pub fn expire_session(&self, reason: &str) {
        tracing::warn!(reason, "Admin session expired");
        self.logout();
        self.emit_session_expired(reason);
    }

    /// Reacts to errors every screen must handle (subscription block).
    fn observe_error(&self, err: &ApiError) {
        if let Some(block) = SubscriptionBlock::from_error(err) {
            tracing::warn!(
                status = %block.status,
                reason = ?block.reason,
                "Subscription blocked"
            );
            self.record_portal(block.portal_url.clone(), block.support_email.clone());
            self.emit_subscription_blocked(&block);
        }
    }

    fn observe_success_headers(&self, resp: &reqwest::Response) {
        if let Some(days) = resp
            .headers()
            .get(TRIAL_DAYS_HEADER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<i64>().ok())
            .filter(|d| *d >= 0)
        {
            self.set_trial_days_left(Some(days));
        }
    }

    // ------------------------------------------------------------------ public calls

    /// Unauthenticated `POST` returning `data` (register, login, Google, logout).
    async fn post_public<B: Serialize, T: DeserializeOwned>(
        &self,
        endpoint: &str,
        body: &B,
    ) -> Result<T, ApiError> {
        let resp = self
            .http
            .post(self.url(endpoint))
            .header(APP_VERSION_HEADER, APP_VERSION)
            .json(body)
            .send()
            .await
            .map_err(ApiError::network)?;
        if !resp.status().is_success() {
            return Err(ApiError::from_response(resp).await);
        }
        let wrapped: SingleResponse<T> = resp.json().await.map_err(ApiError::deserialization)?;
        Ok(wrapped.data)
    }

    /// `POST /auth/login` with `role: ADMIN` and the device descriptor (RF-INT-02-5).
    pub async fn login(&self, email: &str, password: &str) -> Result<AdminProfile, ApiError> {
        let body = serde_json::json!({
            "email": email.trim(),
            "password": password,
            "role": "ADMIN",
            "device": crate::device::descriptor(),
        });
        let auth: AuthResponse = self.post_public("/auth/login", &body).await?;
        self.establish_session(auth).await
    }

    /// `POST /auth/register` (starts a 14-day trial).
    pub async fn register(
        &self,
        name: &str,
        email: &str,
        password: &str,
        organization: &str,
    ) -> Result<AdminProfile, ApiError> {
        let body = serde_json::json!({
            "name": name.trim(),
            "email": email.trim(),
            "password": password,
            "organization": organization.trim(),
            "device": crate::device::descriptor(),
        });
        let auth: AuthResponse = self.post_public("/auth/register", &body).await?;
        self.establish_session(auth).await
    }

    /// `POST /auth/google` with a Google ID token obtained through the loopback flow.
    pub async fn login_google(&self, id_token: &str) -> Result<AdminProfile, ApiError> {
        let body = serde_json::json!({
            "id_token": id_token,
            "role": "ADMIN",
            "device": crate::device::descriptor(),
        });
        let auth: AuthResponse = self.post_public("/auth/google", &body).await?;
        self.establish_session(auth).await
    }

    /// Stores the tokens, then loads the real profile. No fallback data (PLAT-02): if the
    /// profile cannot be read, the new session is revoked and the error is returned.
    async fn establish_session(&self, auth: AuthResponse) -> Result<AdminProfile, ApiError> {
        if let Err(e) = crate::keystore::save_admin_refresh_token(&auth.refresh_token) {
            tracing::error!(
                "Refresh token could not be stored in the OS keystore: {}",
                e
            );
        }
        self.set_access_token(&auth.access_token);
        self.set_trial_days_left(None);
        self.failed_unlock_attempts.store(0, Ordering::SeqCst);

        match self.get_admin_me().await {
            Ok(me) => {
                let profile = AdminProfile::from_me(me, auth.user.is_new_user);
                self.set_current_user(Some(profile.clone()));
                tracing::info!(is_new_user = auth.user.is_new_user, "Admin session opened");
                Ok(self.get_current_user().unwrap_or(profile))
            }
            Err(err) => {
                let refresh = Zeroizing::new(auth.refresh_token.clone());
                self.revoke_refresh_token(&refresh).await;
                self.logout();
                Err(err)
            }
        }
    }

    // ------------------------------------------------------------------ refresh

    /// Refresh after a 401 obtained with `failed_token` (single-flight, RF-INT-02-3).
    async fn refresh_after_401(&self, failed_token: &str) -> Result<Zeroizing<String>, ApiError> {
        let _guard = self.refresh_lock.lock().await;
        if let Some(current) = self.current_access_token() {
            if current.as_str() != failed_token {
                tracing::info!("refresh skipped: token already rotated by another task");
                return Ok(current);
            }
        }
        self.refresh_locked().await
    }

    /// Performs the refresh. Must be called with `refresh_lock` held.
    async fn refresh_locked(&self) -> Result<Zeroizing<String>, ApiError> {
        let Ok(refresh_token) = crate::keystore::load_admin_refresh_token() else {
            self.expire_session("SESSION_EXPIRED");
            return Err(ApiError::local(
                "SESSION_EXPIRED",
                "La sesión expiró; inicia sesión nuevamente.",
            ));
        };
        let resp = self
            .http
            .post(self.url("/auth/refresh"))
            .header(APP_VERSION_HEADER, APP_VERSION)
            .json(&serde_json::json!({ "refresh_token": refresh_token.as_str() }))
            .send()
            .await
            .map_err(ApiError::network)?;

        if !resp.status().is_success() {
            let err = ApiError::from_response(resp).await;
            if err.status == 401 {
                let reason = err
                    .reason
                    .clone()
                    .unwrap_or_else(|| "SESSION_EXPIRED".into());
                self.expire_session(&reason);
            }
            return Err(err);
        }

        let wrapped: SingleResponse<AuthResponse> =
            resp.json().await.map_err(ApiError::deserialization)?;
        let auth = wrapped.data;
        // Single-use refresh token: persist the new one before using the new access token.
        if let Err(e) = crate::keystore::save_admin_refresh_token(&auth.refresh_token) {
            tracing::error!("Rotated refresh token could not be stored: {}", e);
        }
        self.set_access_token(&auth.access_token);
        tracing::info!("refresh rotated (single-flight)");
        Ok(Zeroizing::new(auth.access_token))
    }

    // ------------------------------------------------------------------ authenticated calls

    async fn send_once(
        &self,
        method: &Method,
        endpoint: &str,
        body: Option<&serde_json::Value>,
        token: &str,
    ) -> Result<reqwest::Response, ApiError> {
        let mut req = self
            .http
            .request(method.clone(), self.url(endpoint))
            .header(APP_VERSION_HEADER, APP_VERSION)
            .bearer_auth(token);
        if let Some(b) = body {
            req = req.json(b);
        }
        req.send().await.map_err(ApiError::network)
    }

    /// Sends an authenticated request. Returns only 2xx responses; everything else is an
    /// [`ApiError`] after handling 401 (refresh or expiry) and subscription blocks.
    pub async fn send_with_auth(
        &self,
        method: Method,
        endpoint: &str,
        body: Option<serde_json::Value>,
    ) -> Result<reqwest::Response, ApiError> {
        let Some(token) = self.current_access_token() else {
            return Err(ApiError::local(
                "NOT_AUTHENTICATED",
                "Inicia sesión para continuar.",
            ));
        };
        let resp = self
            .send_once(&method, endpoint, body.as_ref(), &token)
            .await?;
        let resp = if resp.status() == StatusCode::UNAUTHORIZED {
            let err = ApiError::from_response(resp).await;
            if err.has_reason("DEVICE_REVOKED") {
                self.expire_session("DEVICE_REVOKED");
                return Err(err);
            }
            let fresh = self.refresh_after_401(&token).await?;
            let retry = self
                .send_once(&method, endpoint, body.as_ref(), &fresh)
                .await?;
            if retry.status() == StatusCode::UNAUTHORIZED {
                let err = ApiError::from_response(retry).await;
                let reason = err
                    .reason
                    .clone()
                    .unwrap_or_else(|| "SESSION_EXPIRED".into());
                self.expire_session(&reason);
                return Err(err);
            }
            retry
        } else {
            resp
        };

        if resp.status().is_success() {
            self.observe_success_headers(&resp);
            return Ok(resp);
        }
        let err = ApiError::from_response(resp).await;
        self.observe_error(&err);
        Err(err)
    }

    async fn json_data<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T, ApiError> {
        let wrapped: SingleResponse<T> = resp.json().await.map_err(ApiError::deserialization)?;
        Ok(wrapped.data)
    }

    pub async fn get_json<T: DeserializeOwned>(&self, endpoint: &str) -> Result<T, ApiError> {
        let resp = self.send_with_auth(Method::GET, endpoint, None).await?;
        Self::json_data(resp).await
    }

    pub async fn get_collection<T: DeserializeOwned>(
        &self,
        endpoint: &str,
    ) -> Result<CollectionResponse<T>, ApiError> {
        let resp = self.send_with_auth(Method::GET, endpoint, None).await?;
        resp.json().await.map_err(ApiError::deserialization)
    }

    async fn send_json<B: Serialize, T: DeserializeOwned>(
        &self,
        method: Method,
        endpoint: &str,
        body: &B,
    ) -> Result<T, ApiError> {
        let value = serde_json::to_value(body).map_err(ApiError::deserialization)?;
        let resp = self.send_with_auth(method, endpoint, Some(value)).await?;
        Self::json_data(resp).await
    }

    pub async fn post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        endpoint: &str,
        body: &B,
    ) -> Result<T, ApiError> {
        self.send_json(Method::POST, endpoint, body).await
    }

    pub async fn put_json<B: Serialize, T: DeserializeOwned>(
        &self,
        endpoint: &str,
        body: &B,
    ) -> Result<T, ApiError> {
        self.send_json(Method::PUT, endpoint, body).await
    }

    pub async fn patch_json<B: Serialize, T: DeserializeOwned>(
        &self,
        endpoint: &str,
        body: &B,
    ) -> Result<T, ApiError> {
        self.send_json(Method::PATCH, endpoint, body).await
    }

    /// `DELETE` expecting `204 No Content`.
    pub async fn delete(&self, endpoint: &str) -> Result<(), ApiError> {
        self.send_with_auth(Method::DELETE, endpoint, None).await?;
        Ok(())
    }

    pub async fn get_admin_me(&self) -> Result<AdminMeResponse, ApiError> {
        self.get_json("/admins/me").await
    }

    pub async fn register_public_key(
        &self,
        public_key: &str,
    ) -> Result<RegisterPublicKeyResponse, ApiError> {
        self.put_json(
            "/admins/me/public-key",
            &serde_json::json!({ "public_key": public_key }),
        )
        .await
    }

    /// Reloads `/admins/me` into the cached profile (keeps `is_new_user`).
    pub async fn reload_profile(&self) -> Result<AdminProfile, ApiError> {
        let me = self.get_admin_me().await?;
        let is_new = self
            .current_user
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map(|p| p.is_new_user)
            .unwrap_or(false);
        let profile = AdminProfile::from_me(me, is_new);
        self.set_current_user(Some(profile.clone()));
        Ok(self.get_current_user().unwrap_or(profile))
    }

    // ------------------------------------------------------------------ lock / unlock / logout

    pub fn lock_and_flush(&self) {
        *self.access_token.write().unwrap_or_else(|p| p.into_inner()) = None;
        tracing::info!("Admin lock_and_flush: access_token zeroized in RAM (SEC-11)");
    }

    pub fn is_locked(&self) -> bool {
        self.current_access_token().is_none()
            && self
                .current_user
                .read()
                .unwrap_or_else(|p| p.into_inner())
                .is_some()
    }

    pub async fn unlock_with_password(&self, password: &str) -> Result<AdminProfile, ApiError> {
        let email = self
            .current_user
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .map(|u| u.email.clone())
            .ok_or_else(|| {
                ApiError::local("NOT_AUTHENTICATED", "No hay una sesión que desbloquear.")
            })?;

        match self.login(&email, password).await {
            Ok(profile) => {
                self.failed_unlock_attempts.store(0, Ordering::SeqCst);
                tracing::info!("Workspace unlocked successfully via password verification");
                Ok(profile)
            }
            Err(e) if e.status == 401 || e.status == 400 => {
                let attempts = self.failed_unlock_attempts.fetch_add(1, Ordering::SeqCst) + 1;
                tracing::warn!("Failed unlock attempt {}/5", attempts);
                if attempts >= 5 {
                    self.logout();
                    tracing::error!(
                        "Maximum unlock attempts exceeded (5). Session purged and revoked (SEC-11)"
                    );
                    return Err(ApiError::local(
                        "UNLOCK_LIMIT_REACHED",
                        "Límite de intentos fallidos alcanzado (5). Sesión cerrada por seguridad.",
                    ));
                }
                Err(ApiError::local(
                    "UNLOCK_FAILED",
                    format!("Contraseña incorrecta (intento {attempts} de 5)."),
                ))
            }
            Err(e) => Err(e),
        }
    }

    /// Unlocks with the stored refresh token (OS-protected keystore, "biometric" unlock).
    pub async fn unlock_with_refresh(&self) -> Result<AdminProfile, ApiError> {
        {
            let _guard = self.refresh_lock.lock().await;
            self.refresh_locked().await?;
        }
        self.failed_unlock_attempts.store(0, Ordering::SeqCst);
        tracing::info!("Workspace unlocked via stored refresh token");
        self.reload_profile().await
    }

    /// Local logout: zeroizes the access token, forgets the profile, deletes the refresh token.
    pub fn logout(&self) {
        *self.access_token.write().unwrap_or_else(|p| p.into_inner()) = None;
        self.set_current_user(None);
        self.set_trial_days_left(None);
        self.failed_unlock_attempts.store(0, Ordering::SeqCst);
        if let Err(e) = crate::keystore::delete_admin_refresh_token() {
            tracing::warn!(
                "Refresh token could not be deleted from the keystore: {}",
                e
            );
        }
    }

    /// `POST /auth/logout` for `refresh_token` (best effort; `204` has no body).
    async fn revoke_refresh_token(&self, refresh_token: &str) {
        let result = self
            .http
            .post(self.url("/auth/logout"))
            .header(APP_VERSION_HEADER, APP_VERSION)
            .json(&serde_json::json!({ "refresh_token": refresh_token }))
            .send()
            .await;
        match result {
            Ok(resp) if resp.status().is_success() => {}
            Ok(resp) => tracing::warn!("Server logout answered {}", resp.status()),
            Err(e) => tracing::warn!("Server logout failed (revoked locally only): {}", e),
        }
    }

    /// Revokes the refresh token on the server (best effort), then logs out locally.
    pub async fn logout_remote(&self) {
        if let Ok(refresh_token) = crate::keystore::load_admin_refresh_token() {
            self.revoke_refresh_token(&refresh_token).await;
        }
        self.logout();
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::sync::Mutex;

    /// Records the events the client emits.
    #[derive(Default)]
    pub struct RecordingEvents {
        pub expired: Mutex<Vec<String>>,
        pub blocked: Mutex<Vec<SubscriptionBlock>>,
    }

    impl ClientEvents for RecordingEvents {
        fn session_expired(&self, reason: &str) {
            self.expired
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(reason.to_string());
        }
        fn subscription_blocked(&self, block: &SubscriptionBlock) {
            self.blocked
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .push(block.clone());
        }
    }

    pub fn admin_me_json(organization: &str, fingerprint: Option<&str>) -> serde_json::Value {
        serde_json::json!({
            "data": {
                "id": "admin-uuid-1",
                "email": "ana@acme.cl",
                "name": "Ana Admin",
                "organization": organization,
                "is_active": true,
                "worker_count": 5,
                "public_key": null,
                "public_key_fingerprint": fingerprint,
                "has_password": true,
                "google_linked": false,
                "key_backup_enabled": false,
                "onboarding_completed": false,
                "created_at": "2026-09-28T00:00:00Z",
                "updated_at": "2026-09-28T00:00:00Z"
            }
        })
    }

    pub fn auth_json(access: &str, refresh: &str, is_new_user: bool) -> serde_json::Value {
        serde_json::json!({
            "data": {
                "access_token": access,
                "refresh_token": refresh,
                "token_type": "Bearer",
                "expires_in": 900,
                "user": {
                    "id": "admin-uuid-1",
                    "email": "ana@acme.cl",
                    "name": "Ana Admin",
                    "role": "ADMIN",
                    "is_new_user": is_new_user
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_admin_login_sends_role_admin_and_reads_data_envelope() {
        // login() persists the refresh token: isolate from the real OS keystore
        // and from other tests sharing the in-memory vault.
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/auth/login"))
            .and(header(APP_VERSION_HEADER, APP_VERSION))
            .and(body_partial_json(serde_json::json!({
                "email": "ana@acme.cl", "role": "ADMIN"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(auth_json(
                "mock_access_token_xyz",
                "mock_refresh_token_abc",
                false,
            )))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/admins/me"))
            .and(header("Authorization", "Bearer mock_access_token_xyz"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(admin_me_json("Acme Global", None))
                    .insert_header(TRIAL_DAYS_HEADER, "9"),
            )
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
        assert!(profile.has_password);
        assert_eq!(profile.trial_days_left, Some(9));
        assert_eq!(
            crate::keystore::load_admin_refresh_token()
                .unwrap()
                .as_str(),
            "mock_refresh_token_abc"
        );
    }

    #[tokio::test]
    async fn test_login_sends_device_and_fails_without_profile() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/auth/login"))
            .respond_with(ResponseTemplate::new(200).set_body_json(auth_json("a1", "r1", false)))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/admins/me"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        // The session that could not load its profile is revoked on the server.
        Mock::given(method("POST"))
            .and(path("/auth/logout"))
            .and(body_partial_json(
                serde_json::json!({ "refresh_token": "r1" }),
            ))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        let client = AdminApiClient::new(server.uri());
        let err = client.login("ana@acme.cl", "Secret123!").await.unwrap_err();
        assert_eq!(err.status, 503);
        assert!(client.get_current_user().is_none(), "no fake profile");
        assert!(client.get_access_token_for_test().is_none());
        assert!(crate::keystore::load_admin_refresh_token().is_err());

        // The descriptor sent to /auth/login is a stable installation id.
        let requests = server.received_requests().await.unwrap();
        let login: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        let fingerprint = login["device"]["fingerprint"].as_str().unwrap();
        assert!(crate::device::is_valid_installation_id(fingerprint));
        assert!(!login["device"]["name"].as_str().unwrap().is_empty());
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
            .and(body_partial_json(serde_json::json!({
                "refresh_token": "valid_initial_refresh_token"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(auth_json(
                "brand_new_access_token",
                "brand_new_refresh_token",
                false,
            )))
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

        let resp: CollectionResponse<serde_json::Value> = client
            .get_collection("/workers")
            .await
            .expect("Request should succeed after auto-refresh");
        assert!(resp.data.is_empty());

        assert_eq!(
            client.get_access_token_for_test(),
            Some("brand_new_access_token".to_string())
        );
        assert_eq!(
            crate::keystore::load_admin_refresh_token()
                .unwrap()
                .as_str(),
            "brand_new_refresh_token"
        );
    }

    #[tokio::test]
    async fn test_client_concurrent_401_refreshes_once() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;
        crate::keystore::save_admin_refresh_token("r-old").unwrap();

        Mock::given(method("GET"))
            .and(path("/workers"))
            .and(header("Authorization", "Bearer a-old"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/auth/refresh"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(auth_json("a-new", "r-new", false))
                    .set_delay(Duration::from_millis(50)),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/workers"))
            .and(header("Authorization", "Bearer a-new"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "data": [] })),
            )
            .expect(10)
            .mount(&server)
            .await;

        let client = Arc::new(AdminApiClient::new(server.uri()));
        client.set_access_token_for_test(Some("a-old".into()));
        let tasks: Vec<_> = (0..10)
            .map(|_| {
                let client = client.clone();
                tokio::spawn(
                    async move { client.get_collection::<serde_json::Value>("/workers").await },
                )
            })
            .collect();
        for task in tasks {
            assert!(task.await.unwrap().is_ok());
        }
        assert_eq!(
            crate::keystore::load_admin_refresh_token()
                .unwrap()
                .as_str(),
            "r-new"
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
                    "code": "UNAUTHORIZED",
                    "message": "Token family revoked",
                    "details": { "reason": "TOKEN_REUSE_DETECTED" }
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let events = Arc::new(RecordingEvents::default());
        let client = AdminApiClient::new(server.uri());
        client.set_event_sink(events.clone());
        client.set_access_token_for_test(Some("expired_token".to_string()));

        let err = client
            .get_collection::<serde_json::Value>("/workers")
            .await
            .unwrap_err();
        assert!(err.has_reason("TOKEN_REUSE_DETECTED"));
        assert!(client.get_access_token_for_test().is_none());
        assert!(crate::keystore::load_admin_refresh_token().is_err());
        assert_eq!(
            *events.expired.lock().unwrap(),
            vec!["TOKEN_REUSE_DETECTED"]
        );
    }

    #[tokio::test]
    async fn test_client_device_revoked_expires_session_without_refresh() {
        let _guard = crate::keystore::KEYSTORE_TEST_MUTEX.lock().await;
        crate::keystore::init_test_vault();
        let server = MockServer::start().await;
        crate::keystore::save_admin_refresh_token("r").unwrap();

        Mock::given(method("GET"))
            .and(path("/workers"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": { "code": "UNAUTHORIZED", "message": "Device revoked",
                           "details": { "reason": "DEVICE_REVOKED" } }
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/auth/refresh"))
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(&server)
            .await;

        let events = Arc::new(RecordingEvents::default());
        let client = AdminApiClient::new(server.uri());
        client.set_event_sink(events.clone());
        client.set_access_token_for_test(Some("a".into()));
        let err = client
            .get_collection::<serde_json::Value>("/workers")
            .await
            .unwrap_err();
        assert!(err.has_reason("DEVICE_REVOKED"));
        assert!(client.get_access_token_for_test().is_none());
        assert_eq!(*events.expired.lock().unwrap(), vec!["DEVICE_REVOKED"]);
    }

    #[tokio::test]
    async fn test_client_subscription_402_maps_code_details_and_event() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/workers"))
            .respond_with(ResponseTemplate::new(402).set_body_json(serde_json::json!({
                "error": {
                    "code": "SUBSCRIPTION_SUSPENDED",
                    "message": "Tu período de prueba terminó.",
                    "details": {
                        "reason": "TRIAL_EXPIRED", "subscription_status": "SUSPENDED",
                        "portal_url": "https://app.example.cl/dashboard/facturacion",
                        "support_email": "soporte@example.cl",
                        "internal_note": "never forwarded"
                    }
                }
            })))
            .mount(&server)
            .await;

        let events = Arc::new(RecordingEvents::default());
        let client = AdminApiClient::new(server.uri());
        client.set_event_sink(events.clone());
        client.set_access_token_for_test(Some("a".into()));
        let err = client
            .get_collection::<serde_json::Value>("/workers")
            .await
            .unwrap_err();
        assert_eq!(err.status, 402);
        assert_eq!(err.code, "SUBSCRIPTION_SUSPENDED");
        assert!(err.has_reason("TRIAL_EXPIRED"));
        let safe = err.safe_details().unwrap();
        assert_eq!(
            safe["portal_url"],
            "https://app.example.cl/dashboard/facturacion"
        );
        assert!(safe.get("internal_note").is_none());

        let blocked = events.blocked.lock().unwrap().clone();
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].status, "SUSPENDED");
        assert_eq!(blocked[0].reason.as_deref(), Some("TRIAL_EXPIRED"));
        assert_eq!(
            client.portal().portal_url.as_deref(),
            Some("https://app.example.cl/dashboard/facturacion")
        );
        // The session survives: paying and re-checking must not require a new login.
        assert!(client.get_access_token_for_test().is_some());
    }

    #[test]
    fn test_client_error_without_json_body_is_generic() {
        let err = ApiError::from_parts(502, None, b"<html>Bad gateway</html>");
        assert_eq!(err.code, "HTTP_502");
        assert!(!err.message.contains("html"));
        assert!(err.reason.is_none());
    }

    #[test]
    fn test_client_retry_after_header_parsed() {
        let body = br#"{"error":{"code":"RATE_LIMITED","message":"slow down","details":{"reason":"ENROLLMENT_LOCKED","retry_after_seconds":120}}}"#;
        let from_header = ApiError::from_parts(429, Some(900), body);
        assert_eq!(from_header.retry_after_seconds, Some(900));
        let from_details = ApiError::from_parts(429, None, body);
        assert_eq!(from_details.retry_after_seconds, Some(120));
        assert_eq!(
            from_details.safe_details().unwrap()["retry_after_seconds"],
            120
        );
    }

    #[tokio::test]
    async fn test_client_trial_days_left_header_is_recorded() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/workers"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "data": [] }))
                    .insert_header("X-Trial-Days-Left", "3"),
            )
            .mount(&server)
            .await;
        let client = AdminApiClient::new(server.uri());
        client.set_access_token_for_test(Some("a".into()));
        client
            .get_collection::<serde_json::Value>("/workers")
            .await
            .unwrap();
        assert_eq!(client.trial_days_left(), Some(3));
    }

    #[test]
    fn test_client_url_join_never_doubles_slashes() {
        let client = AdminApiClient::new("https://api.example.cl/api/v1/".into());
        assert_eq!(
            client.url("/workers"),
            "https://api.example.cl/api/v1/workers"
        );
        assert_eq!(
            client.url("workers"),
            "https://api.example.cl/api/v1/workers"
        );
    }

    #[test]
    fn test_auth_response_debug_hides_tokens() {
        let auth: AuthResponse =
            serde_json::from_value(auth_json("secret-a", "secret-r", false)["data"].clone())
                .unwrap();
        let printed = format!("{auth:?}");
        assert!(!printed.contains("secret-a") && !printed.contains("secret-r"));
    }
}
