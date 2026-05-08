# Panel Settings System — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the hardcoded filetree panel setting with a full panel configuration system backed by `panels.json`, a new "Panels" tab in Settings, tab-bar rendering in panels, and per-panel tab switching.

**Architecture:** New `PanelsConfig` struct loaded from `panels.json` (following the `LspConfig` pattern). Panel rendering in `app.rs` switches from hardcoded filetree-slot lookup to reading `PanelsConfig`. Settings gets a third tab ("Panels") with panel-centric editing. Migration from the old `config.json` `modules` section happens on first load.

**Tech Stack:** Rust, serde/serde_json, egui/eframe, tempfile (tests)

**Worktree:** All changes in `.worktrees/phase6a/` (branch `feature/phase6a-panels`).

**Spec:** `docs/specs/2026-05-08-panel-settings-design.md`

**Verification:** Before each commit, run all three checks:
```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
```

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `src/config/panels_config.rs` | Create | `PanelsConfig`, `PanelTab`, load/save/dedup/migration, all helper methods |
| `src/config/mod.rs` | Modify | Add `pub mod panels_config;` |
| `src/views/settings.rs` | Modify | Remove `FiletreePanel` from `SettingsField`, add `Panels` to `SettingsTab`, add `PanelsSettingsView` rendering + input |
| `src/views/mod.rs` | Modify | Export new `SettingsTab::Panels` (already re-exported via settings) |
| `src/app.rs` | Modify | Add `panels_config` + `active_tab` fields, load on startup, wire into panel rendering, tab-bar, tab switching, save on settings change |

---

### Task 1: PanelsConfig Data Model — Struct, Default, Serde

**Files:**
- Create: `src/config/panels_config.rs`
- Modify: `src/config/mod.rs`

- [ ] **Step 1: Write tests for PanelTab serde and PanelsConfig default**

Add to `src/config/panels_config.rs`:

```rust
// src/config/panels_config.rs
use serde::{Deserialize, Serialize};
use std::path::Path;

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
                PanelTab { modules: vec!["filetree".into(), "git".into()] },
                PanelTab { modules: vec!["terminal".into()] },
            ],
            bottom: vec![PanelTab { modules: vec!["search".into()] }],
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
```

- [ ] **Step 2: Register module in config/mod.rs**

In `src/config/mod.rs`, add `pub mod panels_config;` after the `lsp_config` line:

```rust
pub mod lsp_config;
pub mod panels_config;
mod schema;

pub use schema::{format_line_number, LineNumberMode, NyxConfig};
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --lib config::panels_config`

Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/config/panels_config.rs src/config/mod.rs
git commit -m "feat: add PanelsConfig struct with serde and defaults"
```

---

### Task 2: PanelsConfig Helper Methods

**Files:**
- Modify: `src/config/panels_config.rs`

- [ ] **Step 1: Write tests for helper methods**

Add these tests to the existing `tests` module in `src/config/panels_config.rs`:

```rust
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
            left: vec![PanelTab { modules: vec!["filetree".into()] }],
            bottom: vec![],
            right: vec![PanelTab { modules: vec!["git".into()] }],
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
                PanelTab { modules: vec!["filetree".into()] },
                PanelTab { modules: vec!["git".into()] },
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
        let mut config = PanelsConfig::default(); // left has filetree
        config.add_tab(PanelSlot::Right);
        config.add_module(PanelSlot::Right, 0, "filetree"); // already in left
        assert!(config.right[0].modules.is_empty());
    }

    #[test]
    fn remove_module_removes_empty_tab() {
        let mut config = PanelsConfig {
            left: vec![PanelTab { modules: vec!["filetree".into()] }],
            bottom: vec![],
            right: vec![],
        };
        config.remove_module(PanelSlot::Left, 0, "filetree");
        assert!(config.left.is_empty());
    }

    #[test]
    fn remove_module_keeps_tab_if_others_remain() {
        let mut config = PanelsConfig {
            left: vec![PanelTab { modules: vec!["filetree".into(), "git".into()] }],
            bottom: vec![],
            right: vec![],
        };
        config.remove_module(PanelSlot::Left, 0, "filetree");
        assert_eq!(config.left.len(), 1);
        assert_eq!(config.left[0].modules, vec!["git"]);
    }
```

- [ ] **Step 2: Implement helper methods**

Add the `use crate::views::PanelSlot;` import at the top of `panels_config.rs`, then add these methods inside `impl PanelsConfig`:

```rust
use crate::views::PanelSlot;

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
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib config::panels_config`

Expected: 11 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/config/panels_config.rs
git commit -m "feat: add PanelsConfig helper methods"
```

---

### Task 3: PanelsConfig Load, Save, Dedup

**Files:**
- Modify: `src/config/panels_config.rs`

- [ ] **Step 1: Write tests for load, save, and dedup**

Add to tests module:

```rust
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
            left: vec![PanelTab { modules: vec!["filetree".into(), "git".into()] }],
            bottom: vec![PanelTab { modules: vec!["terminal".into()] }],
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
        // "filetree" in left wins, removed from bottom
        assert_eq!(config.left[0].modules, vec!["filetree"]);
        // "git" in bottom wins, removed from right
        assert_eq!(config.bottom[0].modules, vec!["git"]);
        // right had only "git" which was deduped, tab removed since empty
        assert!(config.right.is_empty());
    }
```

- [ ] **Step 2: Implement load, save, dedup**

Add these methods to `impl PanelsConfig` and add `use std::collections::HashSet;` at the top:

```rust
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
                    tracing::warn!(
                        "Failed to read {}: {}. Using defaults.",
                        path.display(),
                        e
                    );
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
        std::fs::write(&path, json)
            .map_err(|e| format!("Failed to write panels config: {}", e))?;
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
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib config::panels_config`

Expected: 14 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/config/panels_config.rs
git commit -m "feat: add PanelsConfig load, save, and dedup"
```

---

### Task 4: Migration from config.json Modules

**Files:**
- Modify: `src/config/panels_config.rs`

- [ ] **Step 1: Write migration test**

Add to tests module:

```rust
    use crate::config::schema::{ModuleEntry, ModulesConfig};

    #[test]
    fn migration_from_modules_config() {
        let modules = ModulesConfig {
            filetree: ModuleEntry { enabled: true, panel: Some("left".into()) },
            terminal: ModuleEntry { enabled: true, panel: Some("bottom".into()) },
            git: ModuleEntry { enabled: false, panel: Some("right".into()) },
            search: ModuleEntry { enabled: true, panel: None },
        };
        let config = PanelsConfig::migrate_from_modules(&modules);
        // filetree enabled, panel=left
        assert_eq!(config.left.len(), 1);
        assert_eq!(config.left[0].modules, vec!["filetree"]);
        // terminal enabled, panel=bottom
        assert_eq!(config.bottom.len(), 1);
        assert_eq!(config.bottom[0].modules, vec!["terminal"]);
        // git disabled, not included
        assert!(config.right.is_empty());
        // search enabled, no panel specified -> defaults to left, separate tab
        assert_eq!(config.left.len(), 1); // same tab? No — separate tab
    }
```

Wait — spec says "default 'left' if missing". Let me re-read: "read its `panel` field (default `'left'` if missing)". So search with `panel: None` goes to left as a separate single-module tab:

```rust
    #[test]
    fn migration_from_modules_config() {
        let modules = ModulesConfig {
            filetree: ModuleEntry { enabled: true, panel: Some("left".into()) },
            terminal: ModuleEntry { enabled: true, panel: Some("bottom".into()) },
            git: ModuleEntry { enabled: false, panel: Some("right".into()) },
            search: ModuleEntry { enabled: true, panel: None },
        };
        let config = PanelsConfig::migrate_from_modules(&modules);
        // filetree + search both go to left (separate tabs)
        assert_eq!(config.left.len(), 2);
        assert_eq!(config.left[0].modules, vec!["filetree"]);
        assert_eq!(config.left[1].modules, vec!["search"]);
        // terminal enabled, panel=bottom
        assert_eq!(config.bottom.len(), 1);
        assert_eq!(config.bottom[0].modules, vec!["terminal"]);
        // git disabled
        assert!(config.right.is_empty());
    }
```

- [ ] **Step 2: Implement migration**

Add to `impl PanelsConfig`. This requires importing `ModulesConfig` and `ModuleEntry` from `schema`. Make them `pub` in `src/config/schema.rs` if not already (they are — both structs are `pub`). The `schema` module is private in `mod.rs` though, so either make it `pub(crate)` or re-export what's needed. The simplest approach: change `mod schema;` to `pub(crate) mod schema;` in `src/config/mod.rs`.

In `src/config/mod.rs`:
```rust
pub mod lsp_config;
pub mod panels_config;
pub(crate) mod schema;

pub use schema::{format_line_number, LineNumberMode, NyxConfig};
```

In `src/config/panels_config.rs`:
```rust
use crate::config::schema::ModulesConfig;

impl PanelsConfig {
    pub fn migrate_from_modules(modules: &ModulesConfig) -> Self {
        let mut config = Self {
            left: vec![],
            bottom: vec![],
            right: vec![],
        };
        let entries: &[(&str, &crate::config::schema::ModuleEntry)] = &[
            ("filetree", &modules.filetree),
            ("terminal", &modules.terminal),
            ("git", &modules.git),
            ("search", &modules.search),
        ];
        for (name, entry) in entries {
            if !entry.enabled {
                continue;
            }
            let slot_str = entry.panel.as_deref().unwrap_or("left");
            let tabs = match slot_str {
                "bottom" => &mut config.bottom,
                "right" => &mut config.right,
                _ => &mut config.left,
            };
            tabs.push(PanelTab {
                modules: vec![name.to_string()],
            });
        }
        config
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib config::panels_config`

Expected: 15 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/config/panels_config.rs src/config/mod.rs
git commit -m "feat: add PanelsConfig migration from config.json modules"
```

---

### Task 5: Remove FiletreePanel from Settings Editor Tab

**Files:**
- Modify: `src/views/settings.rs`

- [ ] **Step 1: Remove FiletreePanel variant and revert FIELD_COUNT**

In `src/views/settings.rs`:

1. Change `const FIELD_COUNT: usize = 7;` to `const FIELD_COUNT: usize = 6;`
2. Remove `use crate::views::PanelSlot;` import
3. Remove `FiletreePanel,` from `SettingsField` enum
4. Remove `6 => Some(Self::FiletreePanel),` from `from_index`
5. Remove `Self::FiletreePanel => "Filetree Panel",` from `label`
6. Remove the `Self::FiletreePanel => config.modules...` block from `display_value`
7. Remove the `SettingsField::FiletreePanel => { ... }` block from `activate_field`
8. Remove the `activate_filetree_panel_cycles` and `display_value_filetree_panel` tests

- [ ] **Step 2: Run tests**

Run: `cargo test --lib views::settings`

Expected: all settings tests pass (the two removed tests are gone, existing tests still pass since `from_index_roundtrip` and `labels_non_empty` auto-adjust to FIELD_COUNT=6).

- [ ] **Step 3: Run full verification**

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
```

- [ ] **Step 4: Commit**

```bash
git add src/views/settings.rs
git commit -m "refactor: remove FiletreePanel from Settings Editor tab"
```

---

### Task 6: Add Panels Tab to Settings — SettingsTab + Tab Switching

**Files:**
- Modify: `src/views/settings.rs`

- [ ] **Step 1: Add Panels variant to SettingsTab**

In `src/views/settings.rs`, add the variant and update tab switching:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Editor,
    LspServers,
    Panels,
}
```

Update the tab bar rendering (around line 150) to include Panels:

```rust
for (tab, label) in [
    (SettingsTab::Editor, "Editor"),
    (SettingsTab::LspServers, "LSP Servers"),
    (SettingsTab::Panels, "Panels"),
] {
```

Update the underline width/offset calculation:

```rust
let underline_width = match self.active_tab {
    SettingsTab::Editor => 42.0,
    SettingsTab::LspServers => 82.0,
    SettingsTab::Panels => 48.0,
};
let offset = match self.active_tab {
    SettingsTab::Editor => 0.0,
    SettingsTab::LspServers => 42.0 + 12.0 + 4.0,
    SettingsTab::Panels => 42.0 + 12.0 + 4.0 + 82.0 + 12.0 + 4.0,
};
```

Update the tab content match to add Panels:

```rust
SettingsTab::Panels => {
    // Placeholder for now
    ui.horizontal(|ui| {
        ui.add_space(left_margin);
        ui.label(
            egui::RichText::new("Panel settings coming soon")
                .color(theme.line_number)
                .size(12.0),
        );
    });
}
```

Update all tab-switching logic. The `Tab` key and `H`/`L` keys currently toggle between two tabs — change to cycle through three:

```rust
// Tab switching
if input.key_pressed(egui::Key::Tab) {
    self.active_tab = match self.active_tab {
        SettingsTab::Editor => SettingsTab::LspServers,
        SettingsTab::LspServers => SettingsTab::Panels,
        SettingsTab::Panels => SettingsTab::Editor,
    };
    return;
}
if input.key_pressed(egui::Key::L) {
    self.active_tab = match self.active_tab {
        SettingsTab::Editor => SettingsTab::LspServers,
        SettingsTab::LspServers => SettingsTab::Panels,
        SettingsTab::Panels => SettingsTab::Editor,
    };
    return;
}
if input.key_pressed(egui::Key::H) {
    self.active_tab = match self.active_tab {
        SettingsTab::Editor => SettingsTab::Panels,
        SettingsTab::LspServers => SettingsTab::Editor,
        SettingsTab::Panels => SettingsTab::LspServers,
    };
    return;
}
```

Also update `handle_lsp_tab_input` tab-switch logic to cycle to Panels:

```rust
if tab_switch {
    self.active_tab = SettingsTab::Panels;
    // ...
}
```

- [ ] **Step 2: Add test for Panels tab**

```rust
    #[test]
    fn panels_tab_exists_in_cycle() {
        let mut view = SettingsView::new();
        assert_eq!(view.active_tab, SettingsTab::Editor);
        view.active_tab = SettingsTab::LspServers;
        // Cycling forward from LspServers should reach Panels
        view.active_tab = match view.active_tab {
            SettingsTab::Editor => SettingsTab::LspServers,
            SettingsTab::LspServers => SettingsTab::Panels,
            SettingsTab::Panels => SettingsTab::Editor,
        };
        assert_eq!(view.active_tab, SettingsTab::Panels);
    }
```

- [ ] **Step 3: Run full verification**

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
```

- [ ] **Step 4: Commit**

```bash
git add src/views/settings.rs
git commit -m "feat: add Panels tab to Settings UI"
```

---

### Task 7: Settings Panels Tab — Rendering

**Files:**
- Modify: `src/views/settings.rs`

This task adds the actual rendering of the Panels tab content. The tab shows three sections (LEFT PANEL, BOTTOM PANEL, RIGHT PANEL) each listing their tabs and an "Add tab" row.

- [ ] **Step 1: Add PanelsConfig to SettingsView and rendering state**

Add new fields to `SettingsView`:

```rust
pub struct SettingsView {
    pub selected_row: usize,
    pub editing: Option<SettingsField>,
    pub edit_buffer: String,
    pub active_tab: SettingsTab,
    // Panels tab state
    pub panels_selected_row: usize,
    pub panels_editing_tab: Option<(PanelSlot, usize)>, // (slot, tab_index) when editing a tab's modules
}
```

Initialize new fields in `new()`:

```rust
panels_selected_row: 0,
panels_editing_tab: None,
```

- [ ] **Step 2: Implement render_panels_tab**

Add a new method to `impl SettingsView`. It receives a `&PanelsConfig` and renders the three panel sections. Each row is either a panel header (not selectable), a tab row, or an "Add tab" row.

The total row count is computed dynamically: for each panel slot, 1 header + N tab rows + 1 "Add tab" row = N+2 rows per panel. Total = sum across 3 panels.

```rust
    /// Known module names for display.
    const KNOWN_MODULES: &'static [&'static str] = &["filetree", "terminal", "git", "search"];

    fn render_panels_tab(
        &self,
        ui: &mut egui::Ui,
        panels_config: &PanelsConfig,
        theme: &Theme,
        panel_width: f32,
        left_margin: f32,
    ) {
        let slots = [
            (PanelSlot::Left, "LEFT PANEL"),
            (PanelSlot::Bottom, "BOTTOM PANEL"),
            (PanelSlot::Right, "RIGHT PANEL"),
        ];

        let mut row = 0;
        for (slot, header) in &slots {
            // Section header
            ui.horizontal(|ui| {
                ui.add_space(left_margin);
                ui.label(
                    egui::RichText::new(*header)
                        .color(theme.syntax.keyword)
                        .size(11.0)
                        .strong(),
                );
            });
            ui.add_space(4.0);

            let tabs = panels_config.tabs_for(*slot);
            if tabs.is_empty() {
                // (empty) row
                let is_selected = self.panels_selected_row == row;
                let row_bg = if is_selected {
                    theme.selection
                } else {
                    egui::Color32::TRANSPARENT
                };
                ui.horizontal(|ui| {
                    ui.add_space(left_margin);
                    let row_rect =
                        egui::Rect::from_min_size(ui.cursor().min, egui::vec2(panel_width, 24.0));
                    ui.painter().rect_filled(row_rect, 4.0, row_bg);
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("  (empty)")
                            .color(theme.line_number)
                            .size(13.0)
                            .italics(),
                    );
                });
                row += 1;
            } else {
                for (ti, tab) in tabs.iter().enumerate() {
                    let is_selected = self.panels_selected_row == row;
                    let is_editing = self.panels_editing_tab == Some((*slot, ti));
                    let row_bg = if is_selected {
                        theme.selection
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    ui.horizontal(|ui| {
                        ui.add_space(left_margin);
                        let row_rect = egui::Rect::from_min_size(
                            ui.cursor().min,
                            egui::vec2(panel_width, 24.0),
                        );
                        ui.painter().rect_filled(row_rect, 4.0, row_bg);
                        ui.add_space(8.0);

                        if is_selected {
                            ui.label(
                                egui::RichText::new("▸ ")
                                    .color(theme.syntax.keyword)
                                    .size(13.0),
                            );
                        } else {
                            ui.add_space(14.0);
                        }

                        if is_editing {
                            // Show module checkboxes
                            for module in Self::KNOWN_MODULES {
                                let checked = tab.modules.iter().any(|m| m == module);
                                let in_other = !checked && panels_config.has_module(module);
                                let label = if checked {
                                    format!("[x] {}", Self::capitalize(module))
                                } else {
                                    format!("[ ] {}", Self::capitalize(module))
                                };
                                let color = if in_other {
                                    theme.line_number
                                } else if checked {
                                    theme.foreground
                                } else {
                                    theme.line_number
                                };
                                ui.label(
                                    egui::RichText::new(&label).color(color).size(13.0),
                                );
                                ui.add_space(8.0);
                            }
                        } else {
                            let module_names: Vec<String> = tab
                                .modules
                                .iter()
                                .map(|m| Self::capitalize(m))
                                .collect();
                            let display = format!(
                                "{}: {}",
                                ti + 1,
                                if module_names.is_empty() {
                                    "(empty)".to_string()
                                } else {
                                    module_names.join(", ")
                                }
                            );
                            ui.label(
                                egui::RichText::new(&display)
                                    .color(theme.foreground)
                                    .size(13.0),
                            );
                        }
                    });
                    row += 1;
                }
            }

            // "+ Add tab..." row
            let is_selected = self.panels_selected_row == row;
            let row_bg = if is_selected {
                theme.selection
            } else {
                egui::Color32::TRANSPARENT
            };
            ui.horizontal(|ui| {
                ui.add_space(left_margin);
                let row_rect =
                    egui::Rect::from_min_size(ui.cursor().min, egui::vec2(panel_width, 24.0));
                ui.painter().rect_filled(row_rect, 4.0, row_bg);
                ui.add_space(8.0);
                if is_selected {
                    ui.label(
                        egui::RichText::new("▸ ").color(theme.syntax.keyword).size(13.0),
                    );
                } else {
                    ui.add_space(14.0);
                }
                ui.label(
                    egui::RichText::new("+ Add tab...")
                        .color(theme.syntax.string)
                        .size(13.0),
                );
            });
            row += 1;

            ui.add_space(12.0);
        }
    }

    fn capitalize(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_uppercase().to_string() + chars.as_str(),
        }
    }
```

- [ ] **Step 3: Wire render_panels_tab into the tab content match**

Replace the placeholder in the `SettingsTab::Panels` match arm:

```rust
SettingsTab::Panels => {
    self.render_panels_tab(
        ui,
        &panels_config,
        theme,
        panel_width,
        left_margin,
    );
}
```

This requires passing `panels_config` to `render()`. Update the `render` method signature to accept `&PanelsConfig`:

```rust
pub fn render(
    &mut self,
    ctx: &egui::Context,
    config: &mut NyxConfig,
    theme: &Theme,
    lsp_view: &LspServersView,
    lsp_manager: &LspManager,
    panels_config: &PanelsConfig,
) -> bool {
```

Add `use crate::config::panels_config::PanelsConfig;` import.

- [ ] **Step 4: Update app.rs to pass panels_config to settings render**

In `app.rs`, find the `self.settings_view.render(...)` call and add `&self.panels_config` as the last argument. (This will temporarily fail to compile since `panels_config` isn't on NyxApp yet — that's fine, Task 9 wires it up. For now, add a temporary `PanelsConfig::default()` to make it compile.)

Add to NyxApp struct:

```rust
panels_config: PanelsConfig,
```

Initialize in `new()`:

```rust
panels_config: PanelsConfig::default(),
```

Add import:

```rust
use crate::config::panels_config::PanelsConfig;
```

Pass to render:

```rust
let changed = self.settings_view.render(
    ctx,
    &mut self.config,
    &self.theme,
    &self.lsp_view,
    &self.lsp_manager,
    &self.panels_config,
);
```

- [ ] **Step 5: Run full verification**

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
```

- [ ] **Step 6: Commit**

```bash
git add src/views/settings.rs src/app.rs
git commit -m "feat: render Panels tab in Settings UI"
```

---

### Task 8: Settings Panels Tab — Input Handling

**Files:**
- Modify: `src/views/settings.rs`

- [ ] **Step 1: Implement handle_panels_tab_input**

Add a new method and update the input dispatch. The method needs to return `SettingsAction` and take `&mut PanelsConfig`:

Update `render` signature to take `&mut PanelsConfig` instead of `&PanelsConfig`, and update `handle_input` to route to the panels handler:

In the main `handle_input` method, add a branch for `SettingsTab::Panels`:

```rust
pub fn handle_input(
    &mut self,
    ctx: &egui::Context,
    config: &mut NyxConfig,
    lsp_view: &mut LspServersView,
    lsp_manager: &mut LspManager,
    panels_config: &mut PanelsConfig,
) -> SettingsAction {
    match self.active_tab {
        SettingsTab::Editor => self.handle_editor_tab_input(ctx, config),
        SettingsTab::LspServers => self.handle_lsp_tab_input(ctx, lsp_view, lsp_manager),
        SettingsTab::Panels => self.handle_panels_tab_input(ctx, panels_config),
    }
}
```

Implement `handle_panels_tab_input`:

```rust
    fn handle_panels_tab_input(
        &mut self,
        ctx: &egui::Context,
        panels_config: &mut PanelsConfig,
    ) -> SettingsAction {
        let mut action = SettingsAction::None;
        ctx.input(|input| {
            if input.key_pressed(egui::Key::Escape) {
                if self.panels_editing_tab.is_some() {
                    self.panels_editing_tab = None;
                } else {
                    action = SettingsAction::Close;
                }
                return;
            }

            // Tab switching (settings tabs, not panel tabs)
            if input.key_pressed(egui::Key::Tab) {
                self.active_tab = match self.active_tab {
                    SettingsTab::Editor => SettingsTab::LspServers,
                    SettingsTab::LspServers => SettingsTab::Panels,
                    SettingsTab::Panels => SettingsTab::Editor,
                };
                return;
            }
            if input.key_pressed(egui::Key::H) && self.panels_editing_tab.is_none() {
                self.active_tab = match self.active_tab {
                    SettingsTab::Editor => SettingsTab::Panels,
                    SettingsTab::LspServers => SettingsTab::Editor,
                    SettingsTab::Panels => SettingsTab::LspServers,
                };
                return;
            }
            if input.key_pressed(egui::Key::L) && self.panels_editing_tab.is_none() {
                self.active_tab = match self.active_tab {
                    SettingsTab::Editor => SettingsTab::LspServers,
                    SettingsTab::LspServers => SettingsTab::Panels,
                    SettingsTab::Panels => SettingsTab::Editor,
                };
                return;
            }

            let total_rows = Self::panels_total_rows(panels_config);

            if input.key_pressed(egui::Key::J) || input.key_pressed(egui::Key::ArrowDown) {
                if self.panels_selected_row < total_rows.saturating_sub(1) {
                    self.panels_selected_row += 1;
                }
                return;
            }
            if input.key_pressed(egui::Key::K) || input.key_pressed(egui::Key::ArrowUp) {
                if self.panels_selected_row > 0 {
                    self.panels_selected_row -= 1;
                }
                return;
            }

            if input.key_pressed(egui::Key::Enter) {
                if let Some(row_info) = Self::panels_row_info(panels_config, self.panels_selected_row) {
                    match row_info {
                        PanelsRowKind::Tab(slot, tab_idx) => {
                            self.panels_editing_tab = Some((slot, tab_idx));
                        }
                        PanelsRowKind::AddTab(slot) => {
                            panels_config.add_tab(slot);
                            let new_idx = panels_config.tabs_for(slot).len() - 1;
                            self.panels_editing_tab = Some((slot, new_idx));
                            action = SettingsAction::ConfigChanged;
                        }
                        PanelsRowKind::Empty(_) => {}
                    }
                }
                return;
            }

            if input.key_pressed(egui::Key::D) && self.panels_editing_tab.is_none() {
                if let Some(PanelsRowKind::Tab(slot, tab_idx)) =
                    Self::panels_row_info(panels_config, self.panels_selected_row)
                {
                    panels_config.remove_tab(slot, tab_idx);
                    let total = Self::panels_total_rows(panels_config);
                    if self.panels_selected_row >= total && total > 0 {
                        self.panels_selected_row = total - 1;
                    }
                    action = SettingsAction::ConfigChanged;
                }
                return;
            }

            // Module toggling in edit mode
            if let Some((slot, tab_idx)) = self.panels_editing_tab {
                for (i, key) in [
                    egui::Key::Num1,
                    egui::Key::Num2,
                    egui::Key::Num3,
                    egui::Key::Num4,
                ].iter().enumerate() {
                    if input.key_pressed(*key) {
                        if let Some(module) = Self::KNOWN_MODULES.get(i) {
                            let tab_modules = &panels_config.tabs_for(slot)[tab_idx].modules;
                            if tab_modules.iter().any(|m| m == module) {
                                panels_config.remove_module(slot, tab_idx, module);
                            } else {
                                panels_config.add_module(slot, tab_idx, module);
                            }
                            action = SettingsAction::ConfigChanged;
                        }
                        return;
                    }
                }
            }
        });
        action
    }
```

- [ ] **Step 2: Add helper types and methods for row mapping**

Add above the `impl SettingsView` or inside it:

```rust
    enum PanelsRowKind {
        Empty(PanelSlot),
        Tab(PanelSlot, usize),
        AddTab(PanelSlot),
    }

    fn panels_total_rows(panels_config: &PanelsConfig) -> usize {
        let slots = [PanelSlot::Left, PanelSlot::Bottom, PanelSlot::Right];
        slots.iter().map(|s| {
            let tab_count = panels_config.tabs_for(*s).len();
            if tab_count == 0 { 2 } else { tab_count + 1 } // empty/(tabs) + add_tab
        }).sum()
    }

    fn panels_row_info(panels_config: &PanelsConfig, target_row: usize) -> Option<PanelsRowKind> {
        let slots = [PanelSlot::Left, PanelSlot::Bottom, PanelSlot::Right];
        let mut row = 0;
        for slot in &slots {
            let tabs = panels_config.tabs_for(*slot);
            if tabs.is_empty() {
                if row == target_row {
                    return Some(PanelsRowKind::Empty(*slot));
                }
                row += 1;
            } else {
                for ti in 0..tabs.len() {
                    if row == target_row {
                        return Some(PanelsRowKind::Tab(*slot, ti));
                    }
                    row += 1;
                }
            }
            // "Add tab" row
            if row == target_row {
                return Some(PanelsRowKind::AddTab(*slot));
            }
            row += 1;
        }
        None
    }
```

- [ ] **Step 3: Write tests for panels input**

```rust
    #[test]
    fn panels_add_tab_to_empty_panel() {
        let mut config = PanelsConfig::default();
        assert!(config.is_empty(PanelSlot::Right));
        config.add_tab(PanelSlot::Right);
        assert_eq!(config.tabs_for(PanelSlot::Right).len(), 1);
    }

    #[test]
    fn panels_remove_tab() {
        let mut config = PanelsConfig::default();
        config.remove_tab(PanelSlot::Left, 0);
        assert!(config.is_empty(PanelSlot::Left));
    }

    #[test]
    fn panels_toggle_module() {
        let mut config = PanelsConfig::default();
        // Default: left has filetree. Add git to same tab.
        config.add_module(PanelSlot::Left, 0, "git");
        assert_eq!(config.left[0].modules, vec!["filetree", "git"]);
        // Remove git
        config.remove_module(PanelSlot::Left, 0, "git");
        assert_eq!(config.left[0].modules, vec!["filetree"]);
    }
```

- [ ] **Step 4: Update app.rs to pass &mut panels_config to handle_input**

Update all `handle_input` calls in `app.rs` to pass `&mut self.panels_config`. Also add saving logic: when `SettingsAction::ConfigChanged` is returned while on the Panels tab, call `self.panels_config.save(...)`.

- [ ] **Step 5: Run full verification**

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
```

- [ ] **Step 6: Commit**

```bash
git add src/views/settings.rs src/app.rs
git commit -m "feat: add input handling for Panels settings tab"
```

---

### Task 9: Wire PanelsConfig into App Startup and Panel Rendering

**Files:**
- Modify: `src/app.rs`

This task replaces the hardcoded filetree panel logic with PanelsConfig-driven rendering. Also adds `active_tab: [usize; 3]` for per-panel tab tracking.

- [ ] **Step 1: Load PanelsConfig at startup with migration**

In `NyxApp::new()`, after loading config, add:

```rust
let config_dir = NyxConfig::config_dir();
let panels_config = {
    let path = config_dir.join("panels.json");
    if path.exists() {
        PanelsConfig::load(&config_dir)
    } else {
        // Migrate from config.json modules section
        let migrated = PanelsConfig::migrate_from_modules(&config.modules);
        let _ = migrated.save(&config_dir);
        migrated
    }
};
```

Add `panel_active_tab: [0_usize; 3]` field to NyxApp struct and initialize it. (Named `panel_active_tab` to avoid confusion with `SettingsView::active_tab`.)

- [ ] **Step 2: Update panel visibility initialization**

Replace the hardcoded `left_panel_visible` logic with:

```rust
left_panel_visible: !panels_config.is_empty(PanelSlot::Left),
bottom_panel_visible: !panels_config.is_empty(PanelSlot::Bottom),
right_panel_visible: !panels_config.is_empty(PanelSlot::Right),
```

- [ ] **Step 3: Replace filetree_panel() with PanelsConfig-driven rendering**

Remove the `filetree_panel()` method. In the panel rendering section (around line 955), replace the `ft_slot` lookup and conditional rendering with a loop over the active tab's modules:

```rust
// Inside each panel's show_animated closure, replace:
//   if ft_slot == PanelSlot::Left { self.filetree.render(...) } else { render_empty }
// with:
let active_tab_idx = self.panel_active_tab[0]; // 0=left, 1=bottom, 2=right
let tabs = self.panels_config.tabs_for(PanelSlot::Left);
if let Some(tab) = tabs.get(active_tab_idx) {
    self.render_tab_modules(ui, tab, PanelSlot::Left, focused);
} else if let Some(tab) = tabs.first() {
    self.render_tab_modules(ui, tab, PanelSlot::Left, focused);
}
```

Add a helper method:

```rust
fn render_tab_modules(
    &mut self,
    ui: &mut egui::Ui,
    tab: &PanelTab,
    slot: PanelSlot,
    focused: bool,
) -> ModuleAction {
    let mut action = ModuleAction::None;
    for module in &tab.modules {
        match module.as_str() {
            "filetree" => {
                action = self.filetree.render(ui, &self.theme, focused);
            }
            other => {
                let label = format!("{} — coming soon", Self::capitalize(other));
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        egui::RichText::new(&label)
                            .color(self.theme.line_number)
                            .size(12.0)
                            .italics(),
                    );
                });
            }
        }
    }
    action
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}
```

- [ ] **Step 4: Update Cmd+B to toggle panel by PanelsConfig**

The `toggle_panel` logic should find which panel contains "filetree" and toggle that one:

```rust
if toggle_panel {
    // Find which slot contains filetree
    let slot = [PanelSlot::Left, PanelSlot::Bottom, PanelSlot::Right]
        .into_iter()
        .find(|s| {
            self.panels_config.tabs_for(*s).iter().any(|t| {
                t.modules.iter().any(|m| m == "filetree")
            })
        })
        .unwrap_or(PanelSlot::Left);
    let new_vis = !self.panel_visible(slot);
    self.set_panel_visible(slot, new_vis);
    if !new_vis {
        self.panel_focus = PanelFocus::Editor;
    }
    return;
}
```

- [ ] **Step 5: Update Ctrl+H/J/L to be no-op on empty panels**

```rust
if let Some(slot) = focus_panel_slot {
    if self.active_view == AppView::Editor && !self.panels_config.is_empty(slot) {
        // existing focus toggle logic
    }
}
```

- [ ] **Step 6: Run full verification**

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
```

- [ ] **Step 7: Commit**

```bash
git add src/app.rs
git commit -m "feat: wire PanelsConfig into app startup and panel rendering"
```

---

### Task 10: Tab Bar Rendering and Tab Switching

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Render tab bar when panel has multiple tabs**

In each panel's `show_animated` closure, before rendering modules, add tab bar rendering:

```rust
fn render_panel_tab_bar(
    &self,
    ui: &mut egui::Ui,
    tabs: &[PanelTab],
    active_idx: usize,
    theme: &Theme,
) {
    if tabs.len() <= 1 {
        return;
    }
    ui.horizontal(|ui| {
        for (i, tab) in tabs.iter().enumerate() {
            let is_active = i == active_idx;
            let label = format!(
                "{}: {}",
                i + 1,
                tab.modules.first().map(|m| Self::capitalize(m)).unwrap_or_default()
            );
            let color = if is_active {
                theme.syntax.keyword
            } else {
                theme.line_number
            };
            ui.label(egui::RichText::new(&label).color(color).size(11.0).strong());
            ui.add_space(8.0);
        }
    });
    // Separator line
    let rect = egui::Rect::from_min_size(
        ui.cursor().min,
        egui::vec2(ui.available_width(), 1.0),
    );
    ui.painter().rect_filled(rect, 0.0, theme.line_number);
    ui.allocate_space(egui::vec2(ui.available_width(), 1.0));
    ui.add_space(4.0);
}
```

Call it inside each panel closure before `render_tab_modules`.

- [ ] **Step 2: Add tab switching with 1-9 keys when panel focused**

In the panel input routing section (where escape is handled for focused panels), add number key handling:

```rust
// Tab switching within focused panel
if let Some(slot) = Self::slot_for_focus(self.panel_focus) {
    if self.panel_visible(slot) {
        // ... existing escape handling ...

        // Number keys switch tabs
        let tab_switch = ctx.input(|input| {
            for n in 1..=9u8 {
                let key = match n {
                    1 => egui::Key::Num1,
                    2 => egui::Key::Num2,
                    3 => egui::Key::Num3,
                    4 => egui::Key::Num4,
                    5 => egui::Key::Num5,
                    6 => egui::Key::Num6,
                    7 => egui::Key::Num7,
                    8 => egui::Key::Num8,
                    9 => egui::Key::Num9,
                    _ => unreachable!(),
                };
                if input.key_pressed(key) {
                    return Some((n - 1) as usize);
                }
            }
            None
        });
        if let Some(idx) = tab_switch {
            let slot_idx = match slot {
                PanelSlot::Left => 0,
                PanelSlot::Bottom => 1,
                PanelSlot::Right => 2,
            };
            let tab_count = self.panels_config.tabs_for(slot).len();
            if idx < tab_count {
                self.panel_active_tab[slot_idx] = idx;
            }
            return;
        }
    }
}
```

- [ ] **Step 3: Run full verification**

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
```

- [ ] **Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat: add tab bar rendering and number-key tab switching"
```

---

### Task 11: Save PanelsConfig on Settings Changes

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Save panels_config when settings change it**

In `app.rs`, where `SettingsAction::ConfigChanged` is handled, add panels saving:

```rust
SettingsAction::ConfigChanged => {
    self.apply_config_change(ctx);
    // Save panels config if we're on the Panels tab
    if self.settings_view.active_tab == SettingsTab::Panels {
        let config_dir = NyxConfig::config_dir();
        if let Err(e) = self.panels_config.save(&config_dir) {
            tracing::warn!("Failed to save panels config: {}", e);
        }
        // Update panel visibility based on new config
        self.left_panel_visible = !self.panels_config.is_empty(PanelSlot::Left);
        self.bottom_panel_visible = !self.panels_config.is_empty(PanelSlot::Bottom);
        self.right_panel_visible = !self.panels_config.is_empty(PanelSlot::Right);
    }
}
```

- [ ] **Step 2: Run full verification**

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
```

- [ ] **Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat: save panels config on settings changes"
```

---

### Task 12: Final Cleanup and Manual Testing

**Files:**
- Possibly: `src/app.rs`, `src/views/settings.rs`

- [ ] **Step 1: Remove dead code**

Remove `filetree_panel()` method if still present. Remove `PanelSlot::next()` if no longer used (check with clippy). Remove `render_empty_panel()` if replaced by the new module rendering.

- [ ] **Step 2: Run full verification**

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test
```

- [ ] **Step 3: Manual testing checklist**

1. Start app fresh (delete `panels.json`) — verify it migrates from `config.json`
2. Open Settings > Panels — verify LEFT/BOTTOM/RIGHT sections render
3. Navigate with j/k — verify row selection moves correctly
4. Press Enter on "Add tab" — verify new tab is created
5. In edit mode, press 1-4 to toggle modules — verify checkmarks update
6. Press d to delete a tab — verify it disappears
7. Press Escape to exit edit mode
8. Close settings, verify filetree renders in correct panel
9. Switch filetree to right panel via settings — verify it moves
10. Add two tabs to a panel — verify tab bar appears with `1: Name  2: Name`
11. Focus panel with Ctrl+H, press 1/2 to switch tabs — verify content changes
12. Restart app — verify `panels.json` persists all changes

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore: cleanup dead code after panel settings refactor"
```
