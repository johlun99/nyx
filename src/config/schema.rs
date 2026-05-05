// src/config/schema.rs
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NyxConfig {
    pub editor: EditorConfig,
    pub theme: String,
    pub modules: ModulesConfig,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EditorConfig {
    pub font_family: String,
    pub font_size: f32,
    pub line_numbers: bool,
    pub relative_line_numbers: bool,
    pub cursor_blink: bool,
    pub word_wrap: bool,
    pub tab_size: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModulesConfig {
    pub filetree: ModuleEntry,
    pub terminal: ModuleEntry,
    pub git: ModuleEntry,
    pub search: ModuleEntry,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModuleEntry {
    pub enabled: bool,
    pub panel: Option<String>,
}

impl Default for NyxConfig {
    fn default() -> Self {
        Self {
            editor: EditorConfig {
                font_family: "JetBrains Mono".into(),
                font_size: 14.0,
                line_numbers: true,
                relative_line_numbers: true,
                cursor_blink: false,
                word_wrap: false,
                tab_size: 4,
            },
            theme: "default-dark".into(),
            modules: ModulesConfig {
                filetree: ModuleEntry {
                    enabled: true,
                    panel: Some("left".into()),
                },
                terminal: ModuleEntry {
                    enabled: false,
                    panel: Some("bottom".into()),
                },
                git: ModuleEntry {
                    enabled: false,
                    panel: Some("right".into()),
                },
                search: ModuleEntry {
                    enabled: false,
                    panel: None,
                },
            },
        }
    }
}

impl NyxConfig {
    /// Loads config from path. If missing, creates default and writes it.
    /// If malformed, returns defaults WITHOUT overwriting the existing file.
    pub fn load_or_create(path: &Path) -> Self {
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(config) => return config,
                    Err(e) => {
                        tracing::warn!(
                            "Failed to parse config at {}: {}. Using defaults (file not overwritten).",
                            path.display(),
                            e
                        );
                        return Self::default();
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        "Failed to read config at {}: {}. Using defaults.",
                        path.display(),
                        e
                    );
                    return Self::default();
                }
            }
        }

        // File does not exist — create with defaults
        let config = Self::default();
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!("Failed to create config directory: {}", e);
                return config;
            }
        }
        match serde_json::to_string_pretty(&config) {
            Ok(json) => {
                if let Err(e) = crate::file_io::write_file(path, &json) {
                    tracing::warn!("Failed to write default config: {}", e);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to serialize default config: {}", e);
            }
        }
        config
    }

    pub fn config_dir() -> std::path::PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| {
                tracing::warn!("Could not determine config directory, using current directory");
                std::path::PathBuf::from(".")
            })
            .join("nyx")
    }

    pub fn config_path() -> std::path::PathBuf {
        Self::config_dir().join("config.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn default_config() {
        let config = NyxConfig::default();
        assert_eq!(config.theme, "default-dark");
        assert!(config.modules.filetree.enabled);
        assert!(!config.modules.terminal.enabled);
        assert!(!config.modules.git.enabled);
        assert!(!config.modules.search.enabled);
        assert_eq!(config.editor.font_size, 14.0);
        assert_eq!(config.editor.tab_size, 4);
    }

    #[test]
    fn serialize_and_deserialize() {
        let config = NyxConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: NyxConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.theme, config.theme);
        assert_eq!(parsed.editor.font_size, config.editor.font_size);
    }

    #[test]
    fn load_creates_default_if_missing() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.json");
        let config = NyxConfig::load_or_create(&config_path);
        assert_eq!(config.theme, "default-dark");
        assert!(config_path.exists());
    }

    #[test]
    fn load_reads_existing_config() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.json");
        let mut config = NyxConfig::default();
        config.editor.font_size = 20.0;
        let json = serde_json::to_string_pretty(&config).unwrap();
        std::fs::write(&config_path, json).unwrap();

        let loaded = NyxConfig::load_or_create(&config_path);
        assert_eq!(loaded.editor.font_size, 20.0);
    }

    #[test]
    fn malformed_config_falls_back_without_overwriting() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.json");
        std::fs::write(&config_path, "{ invalid json }").unwrap();

        let loaded = NyxConfig::load_or_create(&config_path);
        assert_eq!(loaded.theme, "default-dark"); // got defaults

        // Original file should NOT be overwritten
        let content = std::fs::read_to_string(&config_path).unwrap();
        assert_eq!(content, "{ invalid json }");
    }
}
