use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::modules::ai_chat::provider::ProviderConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
    #[serde(default = "default_active_provider")]
    pub active_provider: String,
}

fn default_active_provider() -> String {
    "claude".to_string()
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            providers: vec![ProviderConfig {
                name: "claude".to_string(),
                command: "claude".to_string(),
                args: vec![
                    "-p".to_string(),
                    "--output-format".to_string(),
                    "stream-json".to_string(),
                    "--verbose".to_string(),
                ],
                env: vec![],
                enabled: true,
            }],
            active_provider: "claude".to_string(),
        }
    }
}

impl AiConfig {
    const FILE_NAME: &'static str = "ai.json";

    pub fn config_path() -> std::path::PathBuf {
        crate::config::NyxConfig::config_dir().join(Self::FILE_NAME)
    }

    pub fn load_or_create(path: &Path) -> Self {
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(config) => return config,
                    Err(e) => {
                        tracing::warn!(
                            "Failed to parse ai config at {}: {}. Using defaults.",
                            path.display(),
                            e
                        );
                        return Self::default();
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        "Failed to read ai config at {}: {}. Using defaults.",
                        path.display(),
                        e
                    );
                    return Self::default();
                }
            }
        }

        let config = Self::default();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&config) {
            let _ = std::fs::write(path, json);
        }
        config
    }

    #[allow(dead_code)]
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize ai config: {}", e))?;
        std::fs::write(path, json).map_err(|e| format!("Failed to write ai config: {}", e))?;
        Ok(())
    }

    pub fn active_provider_config(&self) -> Option<&ProviderConfig> {
        self.providers
            .iter()
            .find(|p| p.name == self.active_provider && p.enabled)
    }

    #[allow(dead_code)]
    pub fn enabled_provider_names(&self) -> Vec<&str> {
        self.providers
            .iter()
            .filter(|p| p.enabled)
            .map(|p| p.name.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_claude() {
        let config = AiConfig::default();
        assert_eq!(config.active_provider, "claude");
        assert_eq!(config.providers.len(), 1);
        assert_eq!(config.providers[0].name, "claude");
    }

    #[test]
    fn active_provider_config_found() {
        let config = AiConfig::default();
        let active = config.active_provider_config().unwrap();
        assert_eq!(active.name, "claude");
    }

    #[test]
    fn serialize_roundtrip() {
        let config = AiConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AiConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.active_provider, "claude");
    }

    #[test]
    fn load_creates_if_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("ai.json");
        let config = AiConfig::load_or_create(&path);
        assert_eq!(config.active_provider, "claude");
        assert!(path.exists());
    }

    #[test]
    fn save_and_load() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("ai.json");
        let config = AiConfig::default();
        config.save(&path).unwrap();
        let loaded = AiConfig::load_or_create(&path);
        assert_eq!(loaded.active_provider, "claude");
        assert_eq!(loaded.providers.len(), 1);
    }
}
