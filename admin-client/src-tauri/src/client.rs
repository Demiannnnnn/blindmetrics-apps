use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::sync::RwLock;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AdminProfile {
    pub id: String,
    pub email: String,
    pub name: String,
    pub organization: String,
}

pub struct AdminApiClient {
    client: Client,
    base_url: String,
    access_token: RwLock<Option<String>>,
}

impl AdminApiClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .expect("Failed to build HTTP client"),
            base_url,
            access_token: RwLock::new(None),
        }
    }

    pub fn set_access_token(&self, token: String) {
        *self.access_token.write().unwrap() = Some(token);
    }

    pub fn clear_access_token(&self) {
        *self.access_token.write().unwrap() = None;
    }

    pub async fn get_with_auth(&self, endpoint: &str) -> Result<reqwest::Response, String> {
        let url = format!("{}{}", self.base_url, endpoint);
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
