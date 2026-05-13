# Nyx — Project Plan

A minimalist, keyboard-first desktop code editor. Rust + egui.

Full design spec: [docs/superpowers/specs/2026-05-04-nyx-editor-design.md](docs/superpowers/specs/2026-05-04-nyx-editor-design.md)

## Status Legend

- [ ] Not started
- [~] In progress
- [x] Completed

---

## Phase 1: Foundation ✓

The bare minimum — a window that opens, renders text, and responds to vim keybindings.

- [x] Project scaffolding (Cargo workspace, egui setup, CI)
- [x] Window management and GPU rendering pipeline
- [x] Rope-based TextBuffer with undo/redo
- [x] Basic rendering (monospace text, cursor, line numbers)
- [x] VimEngine: Normal, Insert, Command modes
- [x] Basic motions (h/j/k/l, w/b/e, 0/$, gg/G)
- [x] Basic operators (d, c, y, p) with motions
- [x] File open/save
- [x] Config system (`~/.config/nyx/config.json` read/write)
- [x] Default dark theme (hardcoded initially)

## Phase 2: Core Vim Editing ✓

Core vim editing features and text objects.

- [x] Register system (unnamed, named, system clipboard)
- [x] Text objects (`iw`, `aw`, `i"`, `a(`, etc.)
- [x] Visual mode and Visual-Line mode
- [x] Dot-repeat
- [x] Basic search (`/`, `n`, `N`)

## Phase 3: Editor Polish ✓

Syntax highlighting, indentation, and editor UX.

- [x] Relative line numbers
- [x] Tree-sitter syntax highlighting (29 languages)
- [x] Simple keyword-based highlighting fallback (SQL, Dockerfile)
- [x] Auto-indent (tree-sitter aware + copy-indent fallback)
- [x] Auto-indent after `:` (Python)
- [x] Smart backspace (dedent full tab-width)
- [x] Tab key support in insert mode

## Phase 4: Config GUI & Keybindings ✓

Settings UI and keybindings cheatsheet.

- [x] AppView enum and view switching system
- [x] Settings GUI module (`Cmd+,`) with auto-save
- [x] Searchable keybindings cheatsheet (`Cmd+K`)

## Phase 5: LSP Integration ✓

Language intelligence.

- [x] LSP client implementation
- [x] Auto-detect LSP servers from system PATH
- [x] LSP lifecycle management (lazy start/stop)
- [x] Autocomplete
- [x] Go-to-definition / references
- [x] Hover info
- [x] Inline diagnostics
- [x] Rename / code actions
- [x] LSP settings passthrough
- [x] LSP download/install support

## Phase 6: Modules & Panels ✓

Module infrastructure and built-in modules.

- [x] Panel system (left, bottom, right) with toggle behavior
- [x] Panel focus navigation and resize via keyboard
- [x] Filetree module
- [x] Command palette (`Cmd+P`)
- [x] Terminal module (PTY-based)
- [x] Search module (fuzzy filename + content search)
- [x] Git module (status, stage, commit, diff)
- [x] Inline git diff (gutter markers)

## Phase 7: AI Integration

AI-powered editing assistance.

- [ ] AI chat panel (conversation UI in a panel module)
- [ ] Inline AI assistant (code generation / editing in buffer)
- [ ] API key configuration (settings UI)
- [ ] Streaming responses
- [ ] Context-aware prompts (current file, selection, diagnostics)

## Phase 8: Polish & Packaging

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

- Phase 1: Foundation (PR #1)
- Phase 2: Core Vim Editing (PR #2)
- Phase 3: Editor Polish (PR #3)
- Phase 4: Config GUI & Keybindings (PR #4)
- Phase 5: LSP Integration (PRs #5–#6)
- Phase 6: Panels, filetree, terminal, command palette, search (PRs #7–#11)
