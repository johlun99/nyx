use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::views::PanelSlot;

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

#[allow(dead_code)]
impl PanelsConfig {
    fn slots_mut(&mut self, slot: PanelSlot) -> &mut Vec<PanelTab> {
        match slot {
            PanelSlot::Left => &mut self.left,
            PanelSlot::Bottom => &mut self.bottom,
            PanelSlot::Right => &mut self.right,
        }
    }

    pub fn tabs_for(&self, slot: PanelSlot) -> &[PanelTab] {
        match slot {
            PanelSlot::Left => &self.left,
            PanelSlot::Bottom => &self.bottom,
            PanelSlot::Right => &self.right,
        }
    }

    pub fn is_empty(&self, slot: PanelSlot) -> bool {
        self.tabs_for(slot).is_empty()
    }

    pub fn has_module(&self, name: &str) -> bool {
        [&self.left, &self.bottom, &self.right]
            .iter()
            .any(|tabs| tabs.iter().any(|tab| tab.modules.iter().any(|m| m == name)))
    }

    pub fn add_tab(&mut self, slot: PanelSlot) {
        self.slots_mut(slot).push(PanelTab { modules: vec![] });
    }

    pub fn remove_tab(&mut self, slot: PanelSlot, index: usize) {
        let tabs = self.slots_mut(slot);
        if index < tabs.len() {
            tabs.remove(index);
        }
    }

    pub fn add_module(&mut self, slot: PanelSlot, tab: usize, module: &str) {
        if self.has_module(module) {
            return;
        }
        let tabs = self.slots_mut(slot);
        if let Some(t) = tabs.get_mut(tab) {
            t.modules.push(module.to_string());
        }
    }

    pub fn remove_module(&mut self, slot: PanelSlot, tab: usize, module: &str) {
        let tabs = self.slots_mut(slot);
        if let Some(t) = tabs.get_mut(tab) {
            t.modules.retain(|m| m != module);
            if t.modules.is_empty() {
                tabs.remove(tab);
            }
        }
    }

    const FILE_NAME: &'static str = "panels.json";

    pub fn load(dir: &Path) -> Self {
        let path = dir.join(Self::FILE_NAME);
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str::<PanelsConfig>(&content) {
                    Ok(mut config) => {
                        config.dedup();
                        return config;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to parse {}: {}. Using defaults.",
                            path.display(),
                            e
                        );
                    }
                },
                Err(e) => {
                    tracing::warn!("Failed to read {}: {}. Using defaults.", path.display(), e);
                }
            }
        }
        Self::default()
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let path = dir.join(Self::FILE_NAME);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize panels config: {}", e))?;
        std::fs::write(&path, json).map_err(|e| format!("Failed to write panels config: {}", e))?;
        Ok(())
    }

    fn dedup(&mut self) {
        let mut seen = HashSet::new();
        for tabs in [&mut self.left, &mut self.bottom, &mut self.right] {
            for tab in tabs.iter_mut() {
                tab.modules.retain(|m| seen.insert(m.clone()));
            }
            tabs.retain(|tab| !tab.modules.is_empty());
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

    #[test]
    fn tabs_for_slot() {
        let config = PanelsConfig::default();
        assert_eq!(config.tabs_for(PanelSlot::Left).len(), 1);
        assert!(config.tabs_for(PanelSlot::Bottom).is_empty());
        assert!(config.tabs_for(PanelSlot::Right).is_empty());
    }

    #[test]
    fn is_empty_for_empty_panel() {
        let config = PanelsConfig::default();
        assert!(!config.is_empty(PanelSlot::Left));
        assert!(config.is_empty(PanelSlot::Bottom));
    }

    #[test]
    fn has_module_finds_across_panels() {
        let config = PanelsConfig {
            left: vec![PanelTab {
                modules: vec!["filetree".into()],
            }],
            bottom: vec![],
            right: vec![PanelTab {
                modules: vec!["git".into()],
            }],
        };
        assert!(config.has_module("filetree"));
        assert!(config.has_module("git"));
        assert!(!config.has_module("terminal"));
    }

    #[test]
    fn add_tab_appends_empty() {
        let mut config = PanelsConfig::default();
        config.add_tab(PanelSlot::Right);
        assert_eq!(config.right.len(), 1);
        assert!(config.right[0].modules.is_empty());
    }

    #[test]
    fn remove_tab_by_index() {
        let mut config = PanelsConfig {
            left: vec![
                PanelTab {
                    modules: vec!["filetree".into()],
                },
                PanelTab {
                    modules: vec!["git".into()],
                },
            ],
            bottom: vec![],
            right: vec![],
        };
        config.remove_tab(PanelSlot::Left, 0);
        assert_eq!(config.left.len(), 1);
        assert_eq!(config.left[0].modules, vec!["git"]);
    }

    #[test]
    fn add_module_appends_to_tab() {
        let mut config = PanelsConfig::default();
        config.add_module(PanelSlot::Left, 0, "git");
        assert_eq!(config.left[0].modules, vec!["filetree", "git"]);
    }

    #[test]
    fn add_module_dedup_is_noop() {
        let mut config = PanelsConfig::default();
        config.add_tab(PanelSlot::Right);
        config.add_module(PanelSlot::Right, 0, "filetree");
        assert!(config.right[0].modules.is_empty());
    }

    #[test]
    fn remove_module_removes_empty_tab() {
        let mut config = PanelsConfig {
            left: vec![PanelTab {
                modules: vec!["filetree".into()],
            }],
            bottom: vec![],
            right: vec![],
        };
        config.remove_module(PanelSlot::Left, 0, "filetree");
        assert!(config.left.is_empty());
    }

    #[test]
    fn remove_module_keeps_tab_if_others_remain() {
        let mut config = PanelsConfig {
            left: vec![PanelTab {
                modules: vec!["filetree".into(), "git".into()],
            }],
            bottom: vec![],
            right: vec![],
        };
        config.remove_module(PanelSlot::Left, 0, "filetree");
        assert_eq!(config.left.len(), 1);
        assert_eq!(config.left[0].modules, vec!["git"]);
    }

    #[test]
    fn load_missing_file_returns_default() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config = PanelsConfig::load(tmp.path());
        assert_eq!(config.left.len(), 1);
        assert_eq!(config.left[0].modules, vec!["filetree"]);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let config = PanelsConfig {
            left: vec![PanelTab {
                modules: vec!["filetree".into(), "git".into()],
            }],
            bottom: vec![PanelTab {
                modules: vec!["terminal".into()],
            }],
            right: vec![],
        };
        config.save(tmp.path()).unwrap();
        let loaded = PanelsConfig::load(tmp.path());
        assert_eq!(loaded.left[0].modules, vec!["filetree", "git"]);
        assert_eq!(loaded.bottom[0].modules, vec!["terminal"]);
    }

    #[test]
    fn dedup_removes_second_occurrence() {
        let json = r#"{
            "left": [["filetree"]],
            "bottom": [["filetree", "git"]],
            "right": [["git"]]
        }"#;
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("panels.json"), json).unwrap();
        let config = PanelsConfig::load(tmp.path());
        assert_eq!(config.left[0].modules, vec!["filetree"]);
        assert_eq!(config.bottom[0].modules, vec!["git"]);
        assert!(config.right.is_empty());
    }
}
