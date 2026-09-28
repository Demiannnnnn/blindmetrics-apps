use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::sync::RwLock;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AdminProfile {
    pub id: String,
    pub email: String,
    pub name: String,
    pub organization: String,
}

pub struct AdminApiClient {
    client: Client,
    base_url: RwLock<String>,
    access_token: RwLock<Option<String>>,
    current_user: RwLock<Option<AdminProfile>>,
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
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to build HTTP client"),
            base_url: RwLock::new(base_url),
            access_token: RwLock::new(None),
            current_user: RwLock::new(None),
        }
    }

    pub fn set_base_url(&self, url: String) {
        *self.base_url.write().unwrap() = url;
    }

    pub fn get_base_url(&self) -> String {
        self.base_url.read().unwrap().clone()
    }

    pub fn get_current_user(&self) -> Option<AdminProfile> {
        self.current_user.read().unwrap().clone()
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<AdminProfile, String> {
        let base = self.get_base_url();
        let url = format!("{}/auth/login", base.trim_end_matches('/'));

        let resp = self.client.post(&url)
            .json(&serde_json::json!({
                "email": email,
                "password": password
            }))
            .send()
            .await
            .map_err(|e| format!("Network error: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_body = resp.text().await.unwrap_or_default();
            return Err(format!("Login failed ({}): {}", status, err_body));
        }

        let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        
        let token = body.get("access_token")
            .and_then(|t| t.as_str())
            .ok_or_else(|| "Missing access_token in response".to_string())?;
            
        *self.access_token.write().unwrap() = Some(token.to_string());

        let user_val = body.get("user").cloned().unwrap_or_else(|| {
            serde_json::json!({
                "id": "admin-1",
                "email": email,
                "name": "Administrador",
                "organization": "Acme SpA"
            })
        });

        let profile: AdminProfile = serde_json::from_value(user_val)
            .map_err(|e| format!("Invalid profile format: {}", e))?;

        *self.current_user.write().unwrap() = Some(profile.clone());
        Ok(profile)
    }

    pub async fn register(&self, name: &str, email: &str, password: &str, organization: &str) -> Result<AdminProfile, String> {
        let base = self.get_base_url();
        let url = format!("{}/auth/register", base.trim_end_matches('/'));

        let resp = self.client.post(&url)
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

        let profile = AdminProfile {
            id: uuid::Uuid::new_v4().to_string(),
            email: email.to_string(),
            name: name.to_string(),
            organization: organization.to_string(),
        };

        *self.current_user.write().unwrap() = Some(profile.clone());
        Ok(profile)
    }

    pub fn logout(&self) {
        *self.access_token.write().unwrap() = None;
        *self.current_user.write().unwrap() = None;
    }

    pub async fn get_with_auth(&self, endpoint: &str) -> Result<reqwest::Response, String> {
        let base = self.get_base_url();
        let url = format!("{}{}", base.trim_end_matches('/'), endpoint);
        let token = self.access_token.read().unwrap().clone().unwrap_or_default();

        let res = self.client.get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if res.status() == StatusCode::UNAUTHORIZED {
            return Err("Session expired or unauthorized".to_string());
        }

        Ok(res)
    }
}
