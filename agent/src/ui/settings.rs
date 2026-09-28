use serde::{Deserialize, Serialize};
use crate::config::ConsentSettings;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsentUpdateRequest {
    pub category: String,
    pub granted: bool,
}

pub struct ConsentManager {
    worker_role: String,
    current_settings: ConsentSettings,
}

impl ConsentManager {
    pub fn new(worker_role: String, current_settings: ConsentSettings) -> Self {
        Self {
            worker_role,
            current_settings,
        }
    }

    pub fn settings(&self) -> &ConsentSettings {
        &self.current_settings
    }

    /// Updates a single consent category with validation.
    /// Rejects GPS activation if role is OFFICE.
    pub fn update_consent(&mut self, category: &str, granted: bool) -> Result<(), &'static str> {
        match category {
            "system_activity" => self.current_settings.system_activity = granted,
            "network_activity" => self.current_settings.network_activity = granted,
            "productivity_basic" => self.current_settings.productivity_basic = granted,
            "productivity_screenshots" => self.current_settings.productivity_screenshots = granted,
            "file_activity" => self.current_settings.file_activity = granted,
            "location" => {
                if granted && self.worker_role == "OFFICE" {
                    return Err("Location tracking is prohibited by compliance policy for OFFICE workers");
                }
                self.current_settings.location = granted;
            }
            _ => return Err("Unknown consent category"),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_office_worker_cannot_enable_location() {
        let mut manager = ConsentManager::new("OFFICE".into(), ConsentSettings::default());
        assert!(!manager.settings().location);

        let result = manager.update_consent("location", true);
        assert!(result.is_err());
        assert!(!manager.settings().location);
    }

    #[test]
    fn test_field_worker_can_enable_location() {
        let mut manager = ConsentManager::new("FIELD".into(), ConsentSettings::default());
        assert!(!manager.settings().location);

        let result = manager.update_consent("location", true);
        assert!(result.is_ok());
        assert!(manager.settings().location);
    }
}
