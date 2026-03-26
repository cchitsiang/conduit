use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub poll_interval_secs: u64,
    pub launch_at_login: bool,
    pub provider_visibility: HashMap<String, bool>,
    #[serde(default)]
    pub wireguard_last_interface: Option<String>,
    #[serde(default)]
    pub pritunl_last_profile: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        let mut visibility = HashMap::new();
        visibility.insert("Tailscale".to_string(), true);
        visibility.insert("WARP".to_string(), true);
        visibility.insert("WireGuard".to_string(), true);
        visibility.insert("Pritunl".to_string(), true);

        Self {
            poll_interval_secs: 3,
            launch_at_login: false,
            provider_visibility: visibility,
            wireguard_last_interface: None,
            pritunl_last_profile: None,
        }
    }
}

impl AppSettings {
    pub fn load(path: &PathBuf) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &PathBuf) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())
    }

    pub fn is_provider_visible(&self, name: &str) -> bool {
        self.provider_visibility
            .get(name)
            .copied()
            .unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = AppSettings::default();
        assert_eq!(settings.poll_interval_secs, 3);
        assert!(!settings.launch_at_login);
        assert!(settings.is_provider_visible("Tailscale"));
        assert!(settings.is_provider_visible("WARP"));
        assert!(settings.is_provider_visible("WireGuard"));
    }

    #[test]
    fn test_save_and_load() {
        let dir = std::env::temp_dir().join("conduit_test_settings");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_settings.json");

        let mut settings = AppSettings::default();
        settings.poll_interval_secs = 5;
        settings.save(&path).unwrap();

        let loaded = AppSettings::load(&path);
        assert_eq!(loaded.poll_interval_secs, 5);

        // Cleanup
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn test_load_missing_file() {
        let path = PathBuf::from("/tmp/nonexistent_conduit_test_12345.json");
        let settings = AppSettings::load(&path);
        assert_eq!(settings.poll_interval_secs, 3); // defaults
    }
}
