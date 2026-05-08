# Panel Settings System — Design Spec

## Overview

Replace the single "Filetree Panel" cycle in Settings with a full panel configuration system. Panels are configured in a dedicated `panels.json` file and managed through a new "Panels" tab in Settings.

All implementation happens in the `phase6a` worktree (`.worktrees/phase6a/`, branch `feature/phase6a-panels`).

## Config Format

### File: `panels.json`

Located alongside `config.json` (same directory, e.g. `~/.config/nyx/`).

```json
{
  "left": [
    ["filetree", "git"],
    ["terminal"]
  ],
  "bottom": [
    ["search"]
  ],
  "right": []
}
```

Structure: each panel key (`left`, `bottom`, `right`) maps to an array of tabs. Each tab is an array of module names (strings). Within a tab, modules are stacked. Side panels stack vertically; the bottom panel stacks horizontally.

### Defaults

When `panels.json` does not exist:

```json
{
  "left": [["filetree"]],
  "bottom": [],
  "right": []
}
```

### Rules

- Missing panel key = empty (`[]`).
- Empty array = panel is hidden.
- A module may appear in only one tab across all panels. First occurrence wins; duplicates are silently dropped at load time. Duplicates are not removed from the file — only from the runtime model.
- Unknown module names are preserved in the file (forward-compatible) but ignored at render time.

### Known modules

`filetree`, `terminal`, `git`, `search`. Only `filetree` has a real implementation. The other three render a placeholder ("Coming soon") in the panel.

## Data Model

### New file: `src/config/panels_config.rs`

```rust
pub struct PanelsConfig {
    pub left: Vec<PanelTab>,
    pub bottom: Vec<PanelTab>,
    pub right: Vec<PanelTab>,
}

pub struct PanelTab {
    pub modules: Vec<String>,
}
```

Serde: deserialize from / serialize to `{ "left": [["filetree"]], ... }`. `PanelTab` serializes as `Vec<String>`.

### Methods

| Method | Description |
|---|---|
| `load(dir: &Path) -> Self` | Load from `panels.json` in `dir`. Returns `Default` if missing or malformed. |
| `save(&self, dir: &Path)` | Write `panels.json` atomically. |
| `Default` | `left: [["filetree"]], bottom: [], right: []` |
| `tabs_for(&self, slot: PanelSlot) -> &[PanelTab]` | Borrow tabs for a slot. |
| `is_empty(&self, slot: PanelSlot) -> bool` | True if the slot has no tabs (panel hidden). |
| `has_module(&self, name: &str) -> bool` | True if any tab in any panel contains the module. |
| `add_tab(&mut self, slot: PanelSlot)` | Append an empty tab. |
| `remove_tab(&mut self, slot: PanelSlot, index: usize)` | Remove tab at index. |
| `add_module(&mut self, slot: PanelSlot, tab: usize, module: &str)` | Append module to tab. No-op if module already exists elsewhere. |
| `remove_module(&mut self, slot: PanelSlot, tab: usize, module: &str)` | Remove module from tab. If tab becomes empty, remove the tab. |

### Dedup at load

After deserializing, walk all panels in order `left -> bottom -> right`, all tabs in order, all modules in order. Track a `HashSet<String>` of seen modules. Drop any module already seen.

## Migration from `config.json`

On startup, if `panels.json` does not exist but `config.json` contains a `modules` section:

1. For each module entry where `enabled: true`, read its `panel` field (default `"left"` if missing).
2. Place the module into a single-module tab in the corresponding panel slot.
3. Save as `panels.json`.
4. The `modules` key in `config.json` is left untouched but ignored at runtime going forward.

If `panels.json` already exists, `config.json` modules are ignored entirely.

## Settings UI

### New tab: "Panels"

Added to `SettingsTab` enum alongside `Editor` and `LspServers`.

```
Settings                            ESC to close | Tab: switch tab
Editor   LSP Servers   Panels
────────────────────────────

LEFT PANEL
  1: Filetree, Git
  2: Terminal
  + Add tab...

BOTTOM PANEL
  (empty)
  + Add tab...

RIGHT PANEL
  1: Search
  + Add tab...
```

### Navigation

Same vim-style pattern as the Editor tab:

| Key | Action |
|---|---|
| `j` / `k` | Move between rows (tabs, "Add tab" rows, panel headers) |
| `Enter` on a tab | Enter edit mode for that tab (add/remove modules) |
| `Enter` on "Add tab" | Create a new empty tab, enter edit mode |
| `d` on a tab | Delete the tab (with its modules) |
| `Escape` in edit mode | Exit edit mode |

### Tab edit mode

When editing a tab, show a list of all known modules with checkmarks for assigned ones:

```
  1: [x] Filetree  [ ] Terminal  [ ] Git  [ ] Search
```

Press the module's key or Enter to toggle assignment. A module already assigned to another tab is shown dimmed with its location: `Git (right:1)`.

### Remove `FiletreePanel` from Editor tab

The `FiletreePanel` variant is removed from `SettingsField`. `FIELD_COUNT` returns to 6. The `PanelSlot::next()` and `PanelSlot::label()` methods remain (used elsewhere).

## Panel Rendering Changes

### Visibility

- A panel is visible at runtime if `panels_config.tabs_for(slot)` is non-empty AND the runtime toggle (`left_panel_visible` etc.) is true.
- Runtime toggles start as `true` for non-empty panels, `false` for empty.
- `Cmd+B` toggles the filetree's panel visibility (same as today).
- `Ctrl+H/J/L` toggle focus to left/bottom/right. If the panel is empty (no tabs), the keybinding is a no-op.

### Tab bar

Rendered at the top of each panel, only when the panel has more than one tab:

```
1: Filetree  2: Terminal
─────────────────────────
[filetree content]
```

- Active tab highlighted with `theme.syntax.keyword` color.
- Inactive tabs in `theme.line_number` color.
- Number prefix matches the key for quick-switching.

### Tab switching

When a panel has focus, pressing `1`-`9` switches to that tab (if it exists).

### Module rendering within a tab

Active tab's modules are rendered in order:

- **Side panels (left/right)**: modules stacked vertically, each getting an equal share of height (or a collapsible header — start with equal split).
- **Bottom panel**: modules stacked horizontally, each getting an equal share of width.

Module rendering dispatch:

| Module | Renderer |
|---|---|
| `filetree` | `FiletreeModule::render()` |
| `terminal` | Placeholder: "Terminal — coming soon" |
| `git` | Placeholder: "Git — coming soon" |
| `search` | Placeholder: "Search — coming soon" |

### NyxApp struct changes

- Add `panels_config: PanelsConfig` field.
- Add `active_tab: [usize; 3]` (one per panel slot) tracking which tab is active.
- Replace `filetree_panel()` lookup with `panels_config` queries.
- Panel rendering loop reads from `panels_config` instead of hardcoded filetree slot.

## Testing

### `panels_config.rs` unit tests

- `default_config`: left has one tab with filetree, bottom and right empty.
- `load_missing_file`: returns default.
- `load_valid_json`: parses correctly.
- `save_and_load_roundtrip`: serialize then deserialize matches.
- `dedup_removes_second_occurrence`: module in left and right, right copy dropped.
- `add_module_dedup`: adding module that exists elsewhere is a no-op.
- `remove_module_removes_empty_tab`: removing last module from tab removes the tab.
- `is_empty_for_empty_panel`: true when no tabs.
- `has_module`: finds module across panels.
- `migration_from_config`: given a `ModulesConfig`, produces correct `PanelsConfig`.

### Settings UI tests

- `panels_tab_exists`: `SettingsTab::Panels` variant exists and tab switching works.
- `add_tab_to_panel`: adds empty tab to a slot.
- `remove_tab_from_panel`: removes tab and its modules.
- `toggle_module_in_tab`: adds/removes module from a tab.

### Integration (manual)

- Open Settings > Panels, verify three panel sections render.
- Add filetree to right panel, verify it moves from left.
- Add two tabs to left panel, verify tab bar appears and `1`/`2` switches.
- Delete all tabs from a panel, verify panel hides.
- Restart app, verify `panels.json` persists.
