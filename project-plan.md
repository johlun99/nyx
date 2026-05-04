# Nyx — Project Plan

A minimalist, keyboard-first desktop code editor. Rust + egui.

Full design spec: [docs/superpowers/specs/2026-05-04-nyx-editor-design.md](docs/superpowers/specs/2026-05-04-nyx-editor-design.md)

## Status Legend

- [ ] Not started
- [~] In progress
- [x] Completed

---

## Phase 1: Foundation

The bare minimum — a window that opens, renders text, and responds to vim keybindings.

- [ ] Project scaffolding (Cargo workspace, egui setup, CI)
- [ ] Window management and GPU rendering pipeline
- [ ] Rope-based TextBuffer with undo/redo
- [ ] Basic rendering (monospace text, cursor, line numbers)
- [ ] VimEngine: Normal, Insert, Command modes
- [ ] Basic motions (h/j/k/l, w/b/e, 0/$, gg/G)
- [ ] Basic operators (d, c, y, p) with motions
- [ ] File open/save
- [ ] Config system (`~/.config/nyx/config.json` read/write)
- [ ] Default dark theme (hardcoded initially)

## Phase 2: Full Vim & Config

Complete vim emulation and the full config system.

- [ ] VimEngine: Visual, Visual-Line, Visual-Block, Replace modes
- [ ] Registers, macros, marks, dot-repeat
- [ ] Ex-commands (`:s`, `:g`, `:w`, `:q`, etc.)
- [ ] Text objects (`iw`, `aw`, `i"`, `a(`, etc.)
- [ ] KeyMapper with configurable keybindings (`keybindings.json`)
- [ ] Theme system (`~/.config/nyx/themes/`)
- [ ] Config live-reload
- [ ] Action system (named actions, `chain` support)

## Phase 3: Module System & Panels

The panel layout and module infrastructure.

- [ ] NyxModule trait and ModuleRegistry
- [ ] Panel system (left, bottom, right) with toggle behavior
- [ ] Panel focus navigation (`Cmd+h/j/k/l`)
- [ ] Panel resize via keyboard
- [ ] Tab support for multiple modules in same panel
- [ ] Filetree module
- [ ] Command palette (`Cmd+K`)

## Phase 4: Core Modules

Built-in modules that make it a usable editor.

- [ ] Terminal module (PTY-based)
- [ ] Git module (status, stage, commit, diff)
- [ ] Search module (fuzzy filename + content search)
- [ ] Settings GUI module (`Cmd+,`)
- [ ] Inline git diff (gutter markers)

## Phase 5: LSP Integration

Language intelligence.

- [ ] LSP client implementation
- [ ] Auto-detect LSP servers from system PATH
- [ ] LSP lifecycle management (lazy start/stop)
- [ ] Autocomplete
- [ ] Go-to-definition / references
- [ ] Hover info
- [ ] Inline diagnostics
- [ ] Rename / code actions
- [ ] LSP settings passthrough
- [ ] LSP download/install support

## Phase 6: Polish & Package Manager

- [ ] Built-in package manager for LSP servers
- [ ] Config import/export
- [ ] Cross-platform keybinding handling (Cmd vs Ctrl)
- [ ] Performance profiling and optimization
- [ ] Error handling and edge cases
- [ ] Linux-specific testing and fixes

## Future (post-v1)

- [ ] External plugin API and package registry
- [ ] Smart project indexing
- [ ] Windows support
- [ ] Community themes via registry
- [ ] Collaborative editing (CRDT)
- [ ] Workspace profiles

---

## Completed Work

_Nothing yet — project just started._
