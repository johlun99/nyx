# Nyx Editor — Design Spec

## Overview

Nyx is a minimalist, keyboard-first desktop code editor written in Rust with egui. It draws inspiration from Vim's modal editing and JetBrains Fleet's clean, distraction-free UI. The core philosophy: nothing is visible except the text you're editing unless you explicitly opt in.

**Target platforms:** macOS, Linux
**Language:** Rust
**GUI framework:** egui (immediate-mode, GPU-rendered)

## Architecture

### Approach: Monolithic binary with modular internals

A single compiled binary containing all core modules (filetree, terminal, git, search). Modules are compiled in but only initialized when enabled in config. Zero runtime overhead for disabled modules.

Future external plugins will use the same trait-based interface as built-in modules.

### Core Components

| Component | Responsibility |
|-----------|---------------|
| **TextBuffer** | Rope-based data structure for text manipulation. O(log n) inserts, deletes, undo/redo |
| **VimEngine** | Full vim emulation: all modes (Normal, Insert, Visual, Visual-Line, Visual-Block, Command, Replace), motions, operators, registers, macros, marks, dot-repeat, ex-commands |
| **Renderer** | egui-based GPU-rendered view. Only renders visible lines. No chrome by default |
| **KeyMapper** | Receives input, routes to VimEngine or active modules. Handles keybinding overrides from config |
| **ConfigManager** | Reads/writes config files. Live reload support. Import/export |
| **ModuleRegistry** | Trait-based interface for all modules. Built-in modules registered at compile-time, future external plugins at runtime |

### NyxModule Trait

```rust
pub trait NyxModule: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn init(&mut self, ctx: &mut EditorContext) -> Result<()>;
    fn shutdown(&mut self);
    fn on_event(&mut self, event: &EditorEvent) -> Option<EditorEvent>;
    fn render(&self, ui: &mut egui::Ui);
    fn panel_position(&self) -> Option<PanelSlot>; // Left, Bottom, Right, None
    fn commands(&self) -> Vec<Command>;
}
```

This trait is the foundation for future external plugins — same interface regardless of whether a module is built-in or external.

## Panel System & Layout

Three docking zones around the editor, inspired by JetBrains Fleet.

```
┌──────────┬────────────────────────┬──────────┐
│          │                        │          │
│  Left    │                        │  Right   │
│  Panel   │     Editor Area        │  Panel   │
│  (Cmd+1) │                        │  (Cmd+3) │
│          │                        │          │
├──────────┴────────────────────────┴──────────┤
│                Bottom Panel (Cmd+2)           │
└───────────────────────────────────────────────┘
```

### Rules

- All panels hidden by default. Editor starts as a clean text surface
- Toggle with `Cmd+1` / `Cmd+2` / `Cmd+3` (macOS) or `Ctrl+1/2/3` (Linux)
- Toggle behavior: if panel is closed it opens and receives focus, if open it closes and focus returns to editor
- Modules assign themselves to a panel via `panel_position()`. User can override in config
- Panels have configurable width/height (persisted in config), resizable via keyboard
- Multiple modules in same panel render as tabs
- Focus moves between editor and panels via `Ctrl+h/j/k/l` (all platforms, avoids `Cmd+H` = macOS "hide window" conflict)

### Default Panel Mapping (when modules are enabled)

| Panel | Module |
|-------|--------|
| Left | Filetree |
| Bottom | Terminal |
| Right | Git |

## Keyboard-First UX

### Principles

- No visible icons, buttons, or toolbars — ever
- All actions reachable via keybindings or command palette
- Mouse support exists (click to place cursor, scroll) but is never the primary path
- All keybindings fully configurable in `keybindings.json`
- Keybindings mapped to module actions are silently ignored if the module is disabled

### Direct Keybindings (defaults)

| Keybinding | Action |
|------------|--------|
| `Cmd+1` | `panel:toggle left` |
| `Cmd+2` | `panel:toggle bottom` |
| `Cmd+3` | `panel:toggle right` |
| `Cmd+K` | `palette:open` |
| `Cmd+P` | `search:fuzzy_files` |
| `Cmd+Shift+F` | `search:fuzzy_content` |
| `Cmd+R` | `terminal:run` |
| `Cmd+,` | `settings:open` |
| `Ctrl+h/j/k/l` | `panel:focus <direction>` |

Direct keybindings are the primary interaction method. The command palette (`Cmd+K`) is a discovery tool for actions you use rarely or don't remember the binding for.

### Command Palette

- Searchable list of all registered actions from all active modules
- Fuzzy search
- Shows keybinding next to each command
- Vim-style navigation (`j/k`, `Enter`)

### Action System

Every module exposes named actions (`terminal:run`, `git:commit`, `filetree:toggle`). Keybindings map directly to these. `chain` allows combining multiple actions:

```json
{
  "Cmd+B": "chain:['terminal:run cargo build', 'panel:focus bottom']"
}
```

## Config System

All configuration lives in `~/.config/nyx/`:

```
~/.config/nyx/
├── config.json
├── keybindings.json
└── themes/
    └── default-dark.json
```

Import/export = copy the entire `~/.config/nyx/` directory.

### config.json

```json
{
  "editor": {
    "font_family": "JetBrains Mono",
    "font_size": 14,
    "line_numbers": true,
    "relative_line_numbers": true,
    "cursor_blink": false,
    "word_wrap": false,
    "tab_size": 4
  },
  "theme": "default-dark",
  "modules": {
    "filetree": { "enabled": true, "panel": "left" },
    "terminal": { "enabled": false, "panel": "bottom" },
    "git": { "enabled": false, "panel": "right" },
    "search": { "enabled": false }
  },
  "lsp": {
    "rust-analyzer": {
      "enabled": true,
      "path": null,
      "settings": {}
    }
  },
  "panels": {
    "left": { "width": 250 },
    "bottom": { "height": 200 },
    "right": { "width": 300 }
  }
}
```

- `path: null` on an LSP = Nyx searches the system PATH automatically
- Config loaded at startup, live-reloadable via `config:reload` action
- Editable via Settings GUI or directly in file
- Missing config file generates defaults (only filetree enabled)

### keybindings.json

```json
{
  "Cmd+1": "panel:toggle left",
  "Cmd+2": "panel:toggle bottom",
  "Cmd+3": "panel:toggle right",
  "Cmd+K": "palette:open",
  "Cmd+P": "search:fuzzy_files",
  "Cmd+Shift+F": "search:fuzzy_content",
  "Cmd+R": "terminal:run",
  "Cmd+,": "settings:open",
  "Ctrl+h": "panel:focus left",
  "Ctrl+j": "panel:focus bottom",
  "Ctrl+k": "panel:focus up",
  "Ctrl+l": "panel:focus right"
}
```

### Theme Files

Stored in `~/.config/nyx/themes/` as JSON:

```json
{
  "name": "default-dark",
  "background": "#1e1e2e",
  "foreground": "#cdd6f4",
  "cursor": "#f5e0dc",
  "selection": "#45475a",
  "syntax": {
    "keyword": "#cba6f7",
    "string": "#a6e3a1",
    "comment": "#6c7086",
    "function": "#89b4fa",
    "type": "#f9e2af",
    "number": "#fab387"
  }
}
```

Simple to create custom themes: copy, edit colors, set `"theme": "my-theme"` in config.

## LSP Handling

### Auto-detect + Manual Installation

- On startup, Nyx scans system PATH for known LSP servers
- Detected servers shown as available in Settings → Language Servers
- User explicitly enables the ones they want — no LSP runs without opt-in
- If an LSP is not installed, Nyx can offer to download it
- Manual path override in `config.json` for edge cases

### Lifecycle

- LSP process starts only when a file with a relevant filetype is opened AND the server is enabled
- Shuts down when no relevant files are open (lazy — no unnecessary processes)
- LSP crash shows a discreet message, editor is unaffected

### Capabilities

- Autocomplete
- Go-to-definition / references
- Hover info
- Inline diagnostics (errors, warnings)
- Rename / code actions

### Config

```json
{
  "lsp": {
    "rust-analyzer": {
      "enabled": true,
      "path": null,
      "settings": {}
    },
    "gopls": {
      "enabled": true,
      "path": "/usr/local/bin/gopls",
      "settings": {
        "buildFlags": ["-tags=integration"]
      }
    }
  }
}
```

The `settings` object is passed directly to the LSP server as initialization options.

## Built-in Modules

All compiled into the binary but only initialized when enabled. All opt-in except filetree which is enabled by default.

### Filetree (default: enabled)

- Tree view with vim navigation (`j/k` to move, `Enter` to open, `h/l` to collapse/expand)
- Fuzzy filter by typing when panel has focus
- Respects `.gitignore` by default (configurable)
- File operations via keybindings: `a` (new file), `d` (delete), `r` (rename), `m` (move)
- Default panel: left

### Terminal (default: disabled)

- Embedded terminal emulator (PTY-based)
- Supports system shell config (fish, zsh, bash)
- Vim-like scrollback: `Ctrl+[` for normal mode in terminal, navigate output with vim motions
- Run commands via actions (`terminal:run <cmd>`)
- Default panel: bottom

### Git (default: disabled)

- Status view: changed, staged, untracked files
- Inline diff in editor (gutter markers for changes)
- Operations via keybindings: stage (`s`), unstage (`u`), commit (`c`), diff (`d`)
- Commit message written in a temporary editor buffer — same vim experience
- Branch info visible in status bar (if enabled)
- Default panel: right

### Search (default: disabled)

- `Cmd+P` — fuzzy filename search
- `Cmd+Shift+F` — fuzzy content search (ripgrep-based)
- Results in an overlay with vim navigation (`j/k`, `Enter` to jump)

### Settings (always available)

- Opened via `Cmd+,`
- GUI representation of config files
- "Functionality" tab: checkboxes for all modules
- "Language Servers" tab: detected and installed LSPs
- "Keybindings" tab: visual keybinding editor
- "Editor" tab: font, line numbers, tab size, etc.
- "Theme" tab: select active theme

## Package Manager (built-in)

Manages LSP servers initially. Designed to extend to external plugins in the future.

### LSP Management

- Lists auto-detected LSP servers from system PATH
- Shows available servers that can be installed
- Downloads to `~/.config/nyx/lsp/`
- Installed servers appear in Settings → Language Servers

## Future Features (not v1)

- **External package registry** — community plugins and LSP servers installable via the package manager. Requires a registry spec, versioning, and potentially signing
- **Smart indexing** — per-project indexing (a la Fleet), explicitly activated. Faster search, go-to-symbol, etc.
- **Windows support** — architecture should not block this, but focus is macOS + Linux for now
- **Themes via package registry** — install community themes directly in Nyx
- **Collaborative editing** — CRDT-based real-time editing (long-term vision)
- **Workspace profiles** — different config profiles per project/workspace
