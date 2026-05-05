// src/config/schema.rs
use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineNumberMode {
    Absolute,
    Relative,
    Off,
}

impl Serialize for LineNumberMode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let s = match self {
            LineNumberMode::Absolute => "absolute",
            LineNumberMode::Relative => "relative",
            LineNumberMode::Off => "off",
        };
        serializer.serialize_str(s)
    }
}

impl<'de> Deserialize<'de> for LineNumberMode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "absolute" => Ok(LineNumberMode::Absolute),
            "relative" => Ok(LineNumberMode::Relative),
            "off" => Ok(LineNumberMode::Off),
            other => Err(de::Error::unknown_variant(
                other,
                &["absolute", "relative", "off"],
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EditorConfig {
    pub font_family: String,
    pub font_size: f32,
    pub line_numbers: LineNumberMode,
    pub cursor_blink: bool,
    pub word_wrap: bool,
    pub tab_size: usize,
}

impl Serialize for EditorConfig {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("font_family", &self.font_family)?;
        map.serialize_entry("font_size", &self.font_size)?;
        map.serialize_entry("line_numbers", &self.line_numbers)?;
        map.serialize_entry("cursor_blink", &self.cursor_blink)?;
        map.serialize_entry("word_wrap", &self.word_wrap)?;
        map.serialize_entry("tab_size", &self.tab_size)?;
        map.end()
    }
}

struct EditorConfigVisitor;

impl<'de> Visitor<'de> for EditorConfigVisitor {
    type Value = EditorConfig;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map representing EditorConfig")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<EditorConfig, A::Error> {
        let mut font_family: Option<String> = None;
        let mut font_size: Option<f32> = None;
        let mut line_numbers_value: Option<serde_json::Value> = None;
        let mut relative_line_numbers: Option<bool> = None;
        let mut cursor_blink: Option<bool> = None;
        let mut word_wrap: Option<bool> = None;
        let mut tab_size: Option<usize> = None;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "font_family" => {
                    font_family = Some(map.next_value()?);
                }
                "font_size" => {
                    font_size = Some(map.next_value()?);
                }
                "line_numbers" => {
                    line_numbers_value = Some(map.next_value()?);
                }
                "relative_line_numbers" => {
                    relative_line_numbers = Some(map.next_value()?);
                }
                "cursor_blink" => {
                    cursor_blink = Some(map.next_value()?);
                }
                "word_wrap" => {
                    word_wrap = Some(map.next_value()?);
                }
                "tab_size" => {
                    tab_size = Some(map.next_value()?);
                }
                _ => {
                    map.next_value::<serde_json::Value>()?;
                }
            }
        }

        let line_numbers = match line_numbers_value {
            Some(serde_json::Value::String(s)) => match s.as_str() {
                "absolute" => LineNumberMode::Absolute,
                "relative" => LineNumberMode::Relative,
                "off" => LineNumberMode::Off,
                other => {
                    return Err(de::Error::unknown_variant(
                        other,
                        &["absolute", "relative", "off"],
                    ))
                }
            },
            Some(serde_json::Value::Bool(true)) => {
                if relative_line_numbers.unwrap_or(false) {
                    LineNumberMode::Relative
                } else {
                    LineNumberMode::Absolute
                }
            }
            Some(serde_json::Value::Bool(false)) => LineNumberMode::Off,
            None => LineNumberMode::Relative,
            Some(other) => {
                return Err(de::Error::invalid_type(
                    de::Unexpected::Other(&format!("{other}")),
                    &"string or bool",
                ))
            }
        };

        Ok(EditorConfig {
            font_family: font_family.ok_or_else(|| de::Error::missing_field("font_family"))?,
            font_size: font_size.ok_or_else(|| de::Error::missing_field("font_size"))?,
            line_numbers,
            cursor_blink: cursor_blink.ok_or_else(|| de::Error::missing_field("cursor_blink"))?,
            word_wrap: word_wrap.ok_or_else(|| de::Error::missing_field("word_wrap"))?,
            tab_size: tab_size.ok_or_else(|| de::Error::missing_field("tab_size"))?,
        })
    }
}

impl<'de> Deserialize<'de> for EditorConfig {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(EditorConfigVisitor)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NyxConfig {
    pub editor: EditorConfig,
    pub theme: String,
    pub modules: ModulesConfig,
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
                line_numbers: LineNumberMode::Relative,
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

/// Formats a line number string based on the current mode.
///
/// - `Absolute`: returns the 1-based line number.
/// - `Relative`: the cursor line shows its absolute number; all other lines
///   show the distance (in lines) from the cursor.
/// - `Off`: returns an empty string.
pub fn format_line_number(mode: LineNumberMode, line_idx: usize, cursor_line: usize) -> String {
    match mode {
        LineNumberMode::Absolute => format!("{}", line_idx + 1),
        LineNumberMode::Relative => {
            if line_idx == cursor_line {
                format!("{}", line_idx + 1)
            } else {
                format!("{}", line_idx.abs_diff(cursor_line))
            }
        }
        LineNumberMode::Off => String::new(),
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

    // --- LineNumberMode tests ---

    #[test]
    fn line_number_mode_default_is_relative() {
        let config = NyxConfig::default();
        assert_eq!(config.editor.line_numbers, LineNumberMode::Relative);
    }

    #[test]
    fn deserialize_new_format_relative() {
        let json = r#"{
            "font_family": "Mono",
            "font_size": 14.0,
            "line_numbers": "relative",
            "cursor_blink": false,
            "word_wrap": false,
            "tab_size": 4
        }"#;
        let config: EditorConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.line_numbers, LineNumberMode::Relative);
    }

    #[test]
    fn deserialize_new_format_absolute() {
        let json = r#"{
            "font_family": "Mono",
            "font_size": 14.0,
            "line_numbers": "absolute",
            "cursor_blink": false,
            "word_wrap": false,
            "tab_size": 4
        }"#;
        let config: EditorConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.line_numbers, LineNumberMode::Absolute);
    }

    #[test]
    fn deserialize_new_format_off() {
        let json = r#"{
            "font_family": "Mono",
            "font_size": 14.0,
            "line_numbers": "off",
            "cursor_blink": false,
            "word_wrap": false,
            "tab_size": 4
        }"#;
        let config: EditorConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.line_numbers, LineNumberMode::Off);
    }

    #[test]
    fn deserialize_old_format_with_relative() {
        // Both bools true => Relative
        let json = r#"{
            "font_family": "Mono",
            "font_size": 14.0,
            "line_numbers": true,
            "relative_line_numbers": true,
            "cursor_blink": false,
            "word_wrap": false,
            "tab_size": 4
        }"#;
        let config: EditorConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.line_numbers, LineNumberMode::Relative);
    }

    #[test]
    fn deserialize_old_format_absolute_only() {
        // line_numbers true, relative_line_numbers false => Absolute
        let json = r#"{
            "font_family": "Mono",
            "font_size": 14.0,
            "line_numbers": true,
            "relative_line_numbers": false,
            "cursor_blink": false,
            "word_wrap": false,
            "tab_size": 4
        }"#;
        let config: EditorConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.line_numbers, LineNumberMode::Absolute);
    }

    #[test]
    fn deserialize_old_format_off() {
        // line_numbers false => Off (regardless of relative_line_numbers)
        let json = r#"{
            "font_family": "Mono",
            "font_size": 14.0,
            "line_numbers": false,
            "relative_line_numbers": true,
            "cursor_blink": false,
            "word_wrap": false,
            "tab_size": 4
        }"#;
        let config: EditorConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.line_numbers, LineNumberMode::Off);
    }

    #[test]
    fn serialize_produces_new_format() {
        let config = NyxConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        // New format: line_numbers is a string "relative"
        assert!(json.contains("\"line_numbers\": \"relative\""));
        // Old field must not appear
        assert!(!json.contains("relative_line_numbers"));
    }

    // --- format_line_number tests ---

    #[test]
    fn format_line_number_absolute() {
        assert_eq!(format_line_number(LineNumberMode::Absolute, 0, 5), "1");
        assert_eq!(format_line_number(LineNumberMode::Absolute, 4, 5), "5");
        assert_eq!(format_line_number(LineNumberMode::Absolute, 9, 5), "10");
    }

    #[test]
    fn format_line_number_relative_cursor_line_shows_absolute() {
        // cursor is at line index 4 (1-based: 5)
        assert_eq!(format_line_number(LineNumberMode::Relative, 4, 4), "5");
        // cursor at line 0
        assert_eq!(format_line_number(LineNumberMode::Relative, 0, 0), "1");
    }

    #[test]
    fn format_line_number_relative_other_lines_show_distance() {
        // cursor at line index 4; line index 6 is 2 away
        assert_eq!(format_line_number(LineNumberMode::Relative, 6, 4), "2");
        // line index 1 is 3 away from cursor at 4
        assert_eq!(format_line_number(LineNumberMode::Relative, 1, 4), "3");
    }

    #[test]
    fn format_line_number_off_returns_empty() {
        assert_eq!(format_line_number(LineNumberMode::Off, 0, 0), "");
        assert_eq!(format_line_number(LineNumberMode::Off, 5, 3), "");
    }
}
