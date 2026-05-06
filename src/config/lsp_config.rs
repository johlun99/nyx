// src/config/lsp_config.rs
//! LSP server configuration: load/save lsp.json.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Per-server configuration entry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LspServerEntry {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

/// Top-level LSP config stored in lsp.json.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LspConfig {
    #[serde(default)]
    pub servers: HashMap<String, LspServerEntry>,
}

impl LspConfig {
    pub fn config_path() -> PathBuf {
        crate::config::NyxConfig::config_dir().join("lsp.json")
    }

    pub fn load_or_create(path: &Path) -> Self {
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(config) => return config,
                    Err(e) => {
                        tracing::warn!(
                            "Failed to parse lsp config at {}: {}. Using defaults.",
                            path.display(),
                            e
                        );
                        return Self::default();
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        "Failed to read lsp config at {}: {}. Using defaults.",
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

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize lsp config: {}", e))?;
        std::fs::write(path, json).map_err(|e| format!("Failed to write lsp config: {}", e))?;
        Ok(())
    }

    /// Check if a server is enabled in config.
    pub fn is_enabled(&self, name: &str) -> bool {
        self.servers.get(name).map(|e| e.enabled).unwrap_or(false)
    }

    /// Get custom command override for a server.
    pub fn custom_command(&self, name: &str) -> Option<&str> {
        self.servers.get(name).and_then(|e| e.command.as_deref())
    }

    /// Set enabled state for a server.
    pub fn set_enabled(&mut self, name: &str, enabled: bool) {
        self.servers.entry(name.to_string()).or_default().enabled = enabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_empty() {
        let config = LspConfig::default();
        assert!(config.servers.is_empty());
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let mut config = LspConfig::default();
        config.set_enabled("rust-analyzer", true);
        let json = serde_json::to_string(&config).unwrap();
        let parsed: LspConfig = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_enabled("rust-analyzer"));
    }

    #[test]
    fn is_enabled_defaults_false() {
        let config = LspConfig::default();
        assert!(!config.is_enabled("rust-analyzer"));
    }

    #[test]
    fn set_enabled_creates_entry() {
        let mut config = LspConfig::default();
        config.set_enabled("gopls", true);
        assert!(config.is_enabled("gopls"));
    }

    #[test]
    fn custom_command_none_by_default() {
        let config = LspConfig::default();
        assert!(config.custom_command("rust-analyzer").is_none());
    }

    #[test]
    fn load_creates_if_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("lsp.json");
        let config = LspConfig::load_or_create(&path);
        assert!(config.servers.is_empty());
        assert!(path.exists());
    }

    #[test]
    fn save_and_load() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("lsp.json");
        let mut config = LspConfig::default();
        config.set_enabled("rust-analyzer", true);
        config.save(&path).unwrap();

        let loaded = LspConfig::load_or_create(&path);
        assert!(loaded.is_enabled("rust-analyzer"));
    }
}
