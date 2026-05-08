use serde::{Deserialize, Serialize};

/// A single tab within a panel, containing one or more stacked modules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PanelTab {
    pub modules: Vec<String>,
}

/// Serde-friendly representation matching the JSON format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelsConfig {
    #[serde(default)]
    pub left: Vec<PanelTab>,
    #[serde(default)]
    pub bottom: Vec<PanelTab>,
    #[serde(default)]
    pub right: Vec<PanelTab>,
}

impl Default for PanelsConfig {
    fn default() -> Self {
        Self {
            left: vec![PanelTab {
                modules: vec!["filetree".into()],
            }],
            bottom: vec![],
            right: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = PanelsConfig::default();
        assert_eq!(config.left.len(), 1);
        assert_eq!(config.left[0].modules, vec!["filetree"]);
        assert!(config.bottom.is_empty());
        assert!(config.right.is_empty());
    }

    #[test]
    fn serialize_roundtrip() {
        let config = PanelsConfig {
            left: vec![
                PanelTab {
                    modules: vec!["filetree".into(), "git".into()],
                },
                PanelTab {
                    modules: vec!["terminal".into()],
                },
            ],
            bottom: vec![PanelTab {
                modules: vec!["search".into()],
            }],
            right: vec![],
        };
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: PanelsConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.left.len(), 2);
        assert_eq!(parsed.left[0].modules, vec!["filetree", "git"]);
        assert_eq!(parsed.left[1].modules, vec!["terminal"]);
        assert_eq!(parsed.bottom[0].modules, vec!["search"]);
        assert!(parsed.right.is_empty());
    }

    #[test]
    fn deserialize_missing_keys_default_to_empty() {
        let json = r#"{ "left": [["filetree"]] }"#;
        let config: PanelsConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.left.len(), 1);
        assert!(config.bottom.is_empty());
        assert!(config.right.is_empty());
    }
}
