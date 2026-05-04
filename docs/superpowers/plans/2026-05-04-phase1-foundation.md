# Phase 1: Foundation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a minimal desktop window that opens, renders a text buffer with a cursor, responds to basic vim keybindings, can open/save files, and reads config from disk.

**Architecture:** Single Rust binary using eframe/egui for GPU-rendered UI. Rope-based text buffer (via `ropey` crate) for efficient editing. Vim engine as a state machine that consumes key events and produces buffer commands. All text operations use char-based indexing (never byte-based) to support Unicode. Config loaded from `~/.config/nyx/config.json` via serde.

**Tech Stack:** Rust, eframe/egui, ropey, serde/serde_json, dirs, tracing

---

## File Structure

```
nyx/
├── Cargo.toml
├── src/
│   ├── main.rs                  # Entry point, eframe setup, CLI args
│   ├── app.rs                   # NyxApp struct implementing eframe::App (thin adapter)
│   ├── editor.rs                # Editor struct owning buffer + vim state
│   ├── buffer/
│   │   ├── mod.rs               # Re-exports
│   │   ├── text_buffer.rs       # Rope-backed buffer with cursor tracking
│   │   └── history.rs           # Undo/redo stack
│   ├── vim/
│   │   ├── mod.rs               # Re-exports
│   │   ├── action.rs            # VimAction, MotionKind, OperatorAction enums
│   │   ├── mode.rs              # Mode enum and transitions
│   │   ├── keyparser.rs         # Key sequence parsing (with count prefix)
│   │   ├── motion.rs            # Motion definitions and cursor math
│   │   ├── operator.rs          # Operators (d, c, y, p)
│   │   └── command.rs           # Command-line mode (:w, :q, etc.)
│   ├── renderer/
│   │   ├── mod.rs               # Re-exports
│   │   ├── editor_view.rs       # Text rendering, cursor, scrolling
│   │   ├── status_bar.rs        # Mode indicator, file path, command line
│   │   └── theme.rs             # Theme struct + default dark theme
│   ├── config/
│   │   ├── mod.rs               # Re-exports
│   │   └── schema.rs            # Config structs, load/save, defaults
│   └── file_io/
│       ├── mod.rs               # Re-exports
│       └── file.rs              # Atomic file open/save operations
```

---

## Task 1: Project Scaffolding

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/app.rs`

- [ ] **Step 1: Initialize cargo project**

Run: `cd /Users/johanlundgren/private-projects/nyx && cargo init`

- [ ] **Step 2: Add dependencies to Cargo.toml**

```toml
[package]
name = "nyx"
version = "0.1.0"
edition = "2021"

[dependencies]
eframe = "0.31"
ropey = "1.6"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
dirs = "6"
tracing = "0.1"
tracing-subscriber = "0.3"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Write main.rs with eframe bootstrap and tracing**

```rust
// src/main.rs
mod app;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("Nyx"),
        ..Default::default()
    };
    eframe::run_native("Nyx", options, Box::new(|_cc| Ok(Box::new(app::NyxApp::new()))))
}
```

- [ ] **Step 4: Write minimal NyxApp**

```rust
// src/app.rs
pub struct NyxApp;

impl NyxApp {
    pub fn new() -> Self {
        Self
    }
}

impl eframe::App for NyxApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        eframe::egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Nyx editor");
        });
    }
}
```

- [ ] **Step 5: Verify it compiles and opens a window**

Run: `cd /Users/johanlundgren/private-projects/nyx && cargo run`
Expected: A window opens with "Nyx editor" text visible.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml src/main.rs src/app.rs
git commit -m "feat: project scaffolding with eframe window"
```

---

## Task 2: Rope-Based TextBuffer

**Files:**
- Create: `src/buffer/mod.rs`
- Create: `src/buffer/text_buffer.rs`

Key design decisions from review:
- `cursor_line` and `cursor_col` are **private** with accessor methods that validate/clamp
- All lengths are **char-based** (never byte-based)
- `slice()` method delegates to `rope.slice()` for O(log n) range extraction
- `line_slice()` returns `RopeSlice` for zero-allocation line access in render loop
- **Mode-aware cursor clamping:** In Normal mode, cursor clamps to `line_content_len - 1` (on last char). In Insert mode, cursor clamps to `line_content_len` (after last char). `set_cursor` takes an `allow_past_end: bool` parameter.

- [ ] **Step 1: Write failing tests for TextBuffer**

```rust
// src/buffer/text_buffer.rs
use ropey::{Rope, RopeSlice};

pub struct TextBuffer {
    rope: Rope,
    cursor_line: usize,
    cursor_col: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_empty_buffer() {
        let buf = TextBuffer::new();
        assert_eq!(buf.text(), "");
        assert_eq!(buf.cursor_line(), 0);
        assert_eq!(buf.cursor_col(), 0);
    }

    #[test]
    fn new_from_string() {
        let buf = TextBuffer::from_text("hello\nworld");
        assert_eq!(buf.text(), "hello\nworld");
        assert_eq!(buf.line_count(), 2);
    }

    #[test]
    fn insert_char() {
        let mut buf = TextBuffer::new();
        buf.insert_char('a');
        assert_eq!(buf.text(), "a");
        assert_eq!(buf.cursor_col(), 1);
    }

    #[test]
    fn insert_unicode_char() {
        let mut buf = TextBuffer::new();
        buf.insert_char('å');
        buf.insert_char('ä');
        buf.insert_char('ö');
        assert_eq!(buf.text(), "åäö");
        assert_eq!(buf.cursor_col(), 3); // char count, not byte count
    }

    #[test]
    fn insert_newline() {
        let mut buf = TextBuffer::from_text("hello");
        buf.set_cursor(0, 5);
        buf.insert_char('\n');
        assert_eq!(buf.text(), "hello\n");
        assert_eq!(buf.cursor_line(), 1);
        assert_eq!(buf.cursor_col(), 0);
    }

    #[test]
    fn delete_char_before_cursor() {
        let mut buf = TextBuffer::from_text("hello");
        buf.set_cursor(0, 5);
        buf.delete_char_before_cursor();
        assert_eq!(buf.text(), "hell");
        assert_eq!(buf.cursor_col(), 4);
    }

    #[test]
    fn delete_char_at_cursor() {
        let mut buf = TextBuffer::from_text("hello");
        buf.set_cursor(0, 0);
        buf.delete_char_at_cursor();
        assert_eq!(buf.text(), "ello");
        assert_eq!(buf.cursor_col(), 0);
    }

    #[test]
    fn get_line() {
        let buf = TextBuffer::from_text("hello\nworld\nfoo");
        assert_eq!(buf.line_slice(0).to_string(), "hello\n");
        assert_eq!(buf.line_slice(1).to_string(), "world\n");
        assert_eq!(buf.line_slice(2).to_string(), "foo");
    }

    #[test]
    fn delete_range() {
        let mut buf = TextBuffer::from_text("hello world");
        buf.delete_range(5, 11); // delete " world"
        assert_eq!(buf.text(), "hello");
    }

    #[test]
    fn insert_text_at() {
        let mut buf = TextBuffer::from_text("helo");
        buf.insert_text_at(3, "l");
        assert_eq!(buf.text(), "hello");
    }

    #[test]
    fn slice_range() {
        let buf = TextBuffer::from_text("hello world");
        assert_eq!(buf.slice(0, 5), "hello");
        assert_eq!(buf.slice(6, 11), "world");
    }

    #[test]
    fn slice_unicode() {
        let buf = TextBuffer::from_text("hej på dig");
        assert_eq!(buf.slice(4, 6), "på");
    }

    #[test]
    fn line_content_len_excludes_newline() {
        let buf = TextBuffer::from_text("hello\nworld");
        assert_eq!(buf.line_content_len(0), 5); // "hello", not "hello\n"
        assert_eq!(buf.line_content_len(1), 5); // "world" (no trailing \n)
    }

    #[test]
    fn set_cursor_clamps_normal_mode() {
        let mut buf = TextBuffer::from_text("hi\nworld");
        buf.set_cursor(0, 999);
        assert_eq!(buf.cursor_col(), 1); // Normal mode: clamped to last char (index 1, not 2)
        buf.set_cursor(999, 0);
        assert_eq!(buf.cursor_line(), 1); // clamped to last line
    }

    #[test]
    fn set_cursor_insert_mode_allows_past_end() {
        let mut buf = TextBuffer::from_text("hi\nworld");
        buf.set_cursor_with_mode(0, 999, true);
        assert_eq!(buf.cursor_col(), 2); // Insert mode: can be past last char
    }

    #[test]
    fn clamp_cursor_normal_moves_back() {
        let mut buf = TextBuffer::from_text("hi");
        buf.set_cursor_with_mode(0, 2, true); // Insert position past 'i'
        assert_eq!(buf.cursor_col(), 2);
        buf.clamp_cursor_normal(); // Switch to Normal: back to last char
        assert_eq!(buf.cursor_col(), 1);
    }

    #[test]
    fn empty_line_cursor_stays_at_zero() {
        let mut buf = TextBuffer::from_text("");
        buf.set_cursor(0, 999);
        assert_eq!(buf.cursor_col(), 0); // empty line, col 0 is only option
    }

    #[test]
    fn empty_buffer_operations() {
        let mut buf = TextBuffer::new();
        assert_eq!(buf.line_count(), 1); // ropey reports 1 line for empty
        buf.delete_char_before_cursor(); // should not panic
        buf.delete_char_at_cursor(); // should not panic
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib buffer::text_buffer`
Expected: FAIL — methods not implemented.

- [ ] **Step 3: Implement TextBuffer**

```rust
// src/buffer/text_buffer.rs
use ropey::{Rope, RopeSlice};

pub struct TextBuffer {
    rope: Rope,
    cursor_line: usize,
    cursor_col: usize,
}

impl TextBuffer {
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            cursor_line: 0,
            cursor_col: 0,
        }
    }

    pub fn from_text(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            cursor_line: 0,
            cursor_col: 0,
        }
    }

    // --- Cursor accessors (private fields, public getters/setters) ---

    pub fn cursor_line(&self) -> usize {
        self.cursor_line
    }

    pub fn cursor_col(&self) -> usize {
        self.cursor_col
    }

    /// Set cursor position with clamping.
    /// `allow_past_end` = true in Insert mode (cursor can be after last char).
    /// `allow_past_end` = false in Normal mode (cursor stays on last char).
    pub fn set_cursor(&mut self, line: usize, col: usize) {
        self.set_cursor_with_mode(line, col, false);
    }

    pub fn set_cursor_with_mode(&mut self, line: usize, col: usize, allow_past_end: bool) {
        self.cursor_line = line.min(self.line_count().saturating_sub(1));
        let content_len = self.line_content_len(self.cursor_line);
        let max_col = if allow_past_end {
            content_len
        } else {
            content_len.saturating_sub(1) // Normal mode: on last char, not past it
        };
        self.cursor_col = col.min(max_col);
    }

    /// Clamp cursor to Normal mode bounds (on last char, not past).
    /// Call after switching from Insert to Normal mode.
    pub fn clamp_cursor_normal(&mut self) {
        let content_len = self.line_content_len(self.cursor_line);
        let max_col = content_len.saturating_sub(1);
        self.cursor_col = self.cursor_col.min(max_col);
    }

    pub fn cursor_offset(&self) -> usize {
        let line_start = self.rope.line_to_char(self.cursor_line);
        line_start + self.cursor_col
    }

    pub fn update_cursor_from_offset(&mut self, offset: usize) {
        let offset = offset.min(self.rope.len_chars());
        self.cursor_line = self.rope.char_to_line(offset);
        let line_start = self.rope.line_to_char(self.cursor_line);
        self.cursor_col = offset - line_start;
    }

    // --- Text queries ---

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    pub fn slice(&self, start: usize, end: usize) -> String {
        self.rope.slice(start..end).to_string()
    }

    pub fn line_count(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn line_slice(&self, idx: usize) -> RopeSlice<'_> {
        self.rope.line(idx)
    }

    /// Length of line content in chars, excluding trailing newline
    pub fn line_content_len(&self, line_idx: usize) -> usize {
        let line = self.rope.line(line_idx);
        let len = line.len_chars();
        if len > 0 && line.char(len - 1) == '\n' {
            len - 1
        } else {
            len
        }
    }

    pub fn line_len_chars(&self, line_idx: usize) -> usize {
        self.rope.line(line_idx).len_chars()
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// Char offset where a given line starts (delegates to rope.line_to_char)
    pub fn line_to_char(&self, line_idx: usize) -> usize {
        self.rope.line_to_char(line_idx)
    }

    // --- Mutations ---

    pub fn insert_char(&mut self, ch: char) {
        let offset = self.cursor_offset();
        self.rope.insert_char(offset, ch);
        if ch == '\n' {
            self.cursor_line += 1;
            self.cursor_col = 0;
        } else {
            self.cursor_col += 1;
        }
    }

    pub fn delete_char_before_cursor(&mut self) {
        let offset = self.cursor_offset();
        if offset == 0 {
            return;
        }
        let ch = self.rope.char(offset - 1);
        self.rope.remove(offset - 1..offset);
        if ch == '\n' {
            self.cursor_line -= 1;
            self.cursor_col = self.line_content_len(self.cursor_line);
        } else {
            self.cursor_col -= 1;
        }
    }

    pub fn delete_char_at_cursor(&mut self) {
        let offset = self.cursor_offset();
        if offset >= self.rope.len_chars() {
            return;
        }
        self.rope.remove(offset..offset + 1);
    }

    pub fn delete_range(&mut self, start: usize, end: usize) {
        self.rope.remove(start..end);
    }

    pub fn insert_text_at(&mut self, offset: usize, text: &str) {
        self.rope.insert(offset, text);
    }
}
```

- [ ] **Step 4: Create buffer module**

```rust
// src/buffer/mod.rs
mod text_buffer;

pub use text_buffer::TextBuffer;
```

- [ ] **Step 5: Add buffer module to main**

Add `mod buffer;` to `src/main.rs`.

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --lib buffer::text_buffer`
Expected: All 15 tests PASS.

- [ ] **Step 7: Commit**

```bash
git add src/buffer/ src/main.rs
git commit -m "feat: rope-based TextBuffer with private cursor and Unicode support"
```

---

## Task 3: Undo/Redo History

**Files:**
- Create: `src/buffer/history.rs`
- Modify: `src/buffer/text_buffer.rs`
- Modify: `src/buffer/mod.rs`

Key design decisions from review:
- **All TextBuffer mutations automatically record to history.** No separate `_recorded` API — this ensures operators and all other code paths get undo for free.
- **Undo grouping:** History supports `begin_group()`/`end_group()` for atomic undo units. An entire Insert mode session (from `i`/`a`/`o` to `Escape`) becomes one `UndoEntry::Group`, so `u` undoes the whole session — matching vim behavior. Operators outside groups create `UndoEntry::Single` entries.

- [ ] **Step 1: Write failing tests for History**

```rust
// src/buffer/history.rs

#[derive(Clone, Debug)]
pub enum EditAction {
    Insert { offset: usize, text: String },
    Delete { offset: usize, text: String },
}

/// An undo entry is either a single edit or a group of edits (e.g., an entire Insert session).
/// `undo()` always pops one UndoEntry, so a group is undone atomically.
#[derive(Clone, Debug)]
pub enum UndoEntry {
    Single(EditAction),
    Group(Vec<EditAction>),
}

pub struct History {
    undo_stack: std::collections::VecDeque<UndoEntry>,
    redo_stack: Vec<UndoEntry>,
    current_group: Option<Vec<EditAction>>,
    max_entries: usize,
    recording: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_undo() {
        let mut hist = History::new();
        hist.push(EditAction::Insert {
            offset: 0,
            text: "hello".into(),
        });
        let entry = hist.undo();
        assert!(entry.is_some());
        match entry.unwrap() {
            UndoEntry::Single(EditAction::Insert { offset, text }) => {
                assert_eq!(offset, 0);
                assert_eq!(text, "hello");
            }
            _ => panic!("expected Single(Insert)"),
        }
    }

    #[test]
    fn undo_returns_none_when_empty() {
        let mut hist = History::new();
        assert!(hist.undo().is_none());
    }

    #[test]
    fn redo_after_undo() {
        let mut hist = History::new();
        hist.push(EditAction::Insert {
            offset: 0,
            text: "hello".into(),
        });
        let _ = hist.undo();
        let entry = hist.redo();
        assert!(entry.is_some());
    }

    #[test]
    fn new_edit_clears_redo_stack() {
        let mut hist = History::new();
        hist.push(EditAction::Insert {
            offset: 0,
            text: "a".into(),
        });
        let _ = hist.undo();
        hist.push(EditAction::Insert {
            offset: 0,
            text: "b".into(),
        });
        assert!(hist.redo().is_none());
    }

    #[test]
    fn respects_max_entries() {
        let mut hist = History::with_max_entries(3);
        for i in 0..5 {
            hist.push(EditAction::Insert {
                offset: i,
                text: "x".into(),
            });
        }
        // Only 3 most recent should survive
        assert!(hist.undo().is_some());
        assert!(hist.undo().is_some());
        assert!(hist.undo().is_some());
        assert!(hist.undo().is_none());
    }

    #[test]
    fn pause_and_resume_recording() {
        let mut hist = History::new();
        hist.set_recording(false);
        hist.push(EditAction::Insert {
            offset: 0,
            text: "ignored".into(),
        });
        assert!(hist.undo().is_none());
        hist.set_recording(true);
        hist.push(EditAction::Insert {
            offset: 0,
            text: "recorded".into(),
        });
        assert!(hist.undo().is_some());
    }

    #[test]
    fn group_undo_is_atomic() {
        let mut hist = History::new();
        hist.begin_group();
        hist.push(EditAction::Insert { offset: 0, text: "a".into() });
        hist.push(EditAction::Insert { offset: 1, text: "b".into() });
        hist.push(EditAction::Insert { offset: 2, text: "c".into() });
        hist.end_group();

        // Single undo should remove the entire group
        let entry = hist.undo();
        assert!(entry.is_some());
        match entry.unwrap() {
            UndoEntry::Group(actions) => assert_eq!(actions.len(), 3),
            _ => panic!("expected Group"),
        }
        // No more entries
        assert!(hist.undo().is_none());
    }

    #[test]
    fn empty_group_produces_no_entry() {
        let mut hist = History::new();
        hist.begin_group();
        hist.end_group();
        assert!(hist.undo().is_none());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib buffer::history`
Expected: FAIL — methods not implemented.

- [ ] **Step 3: Implement History**

```rust
// src/buffer/history.rs
#[derive(Clone, Debug)]
pub enum EditAction {
    Insert { offset: usize, text: String },
    Delete { offset: usize, text: String },
}

pub struct History {
    undo_stack: std::collections::VecDeque<UndoEntry>,
    redo_stack: Vec<UndoEntry>,
    /// Accumulates actions while an undo group is open (e.g., during Insert mode).
    /// When the group is closed, this becomes a single UndoEntry::Group.
    current_group: Option<Vec<EditAction>>,
    max_entries: usize,
    recording: bool,
}

/// An undo entry is either a single edit or a group of edits (e.g., an entire Insert session).
/// `undo()` always pops one UndoEntry, so a group is undone atomically.
#[derive(Clone, Debug)]
pub enum UndoEntry {
    Single(EditAction),
    Group(Vec<EditAction>),
}

impl History {
    pub fn new() -> Self {
        Self::with_max_entries(10_000)
    }

    pub fn with_max_entries(max: usize) -> Self {
        Self {
            undo_stack: std::collections::VecDeque::new(),
            redo_stack: Vec::new(),
            current_group: None,
            max_entries: max,
            recording: true,
        }
    }

    pub fn set_recording(&mut self, recording: bool) {
        self.recording = recording;
    }

    /// Begin an undo group. All subsequent push() calls accumulate into this group
    /// until end_group() is called. Used when entering Insert mode.
    pub fn begin_group(&mut self) {
        if self.current_group.is_none() {
            self.current_group = Some(Vec::new());
        }
    }

    /// Close the current undo group and push it as a single UndoEntry.
    /// Used when leaving Insert mode (Escape).
    pub fn end_group(&mut self) {
        if let Some(actions) = self.current_group.take() {
            if !actions.is_empty() {
                self.push_entry(UndoEntry::Group(actions));
            }
        }
    }

    pub fn push(&mut self, action: EditAction) {
        if !self.recording {
            return;
        }
        if let Some(ref mut group) = self.current_group {
            group.push(action);
            self.redo_stack.clear();
        } else {
            self.push_entry(UndoEntry::Single(action));
        }
    }

    fn push_entry(&mut self, entry: UndoEntry) {
        self.undo_stack.push_back(entry);
        self.redo_stack.clear();
        if self.undo_stack.len() > self.max_entries {
            self.undo_stack.pop_front(); // O(1) eviction
        }
    }

    pub fn undo(&mut self) -> Option<UndoEntry> {
        let entry = self.undo_stack.pop_back()?;
        self.redo_stack.push(entry.clone());
        Some(entry)
    }

    pub fn redo(&mut self) -> Option<UndoEntry> {
        let entry = self.redo_stack.pop()?;
        self.undo_stack.push_back(entry.clone());
        Some(entry)
    }
}

#[cfg(test)]
mod tests {
    // ... (tests from Step 1)
}
```

- [ ] **Step 4: Integrate History into TextBuffer — auto-recording**

All mutation methods in TextBuffer now automatically record to history. The `recording` flag allows undo/redo to replay without creating recursive history entries.

Add to `src/buffer/text_buffer.rs`:

```rust
use crate::buffer::history::{EditAction, UndoEntry, History};

pub struct TextBuffer {
    rope: Rope,
    cursor_line: usize,
    cursor_col: usize,
    history: History,
}
```

Update `new()` and `from_text()` to initialize `history: History::new()`.

Update mutation methods to record:

```rust
impl TextBuffer {
    pub fn insert_char(&mut self, ch: char) {
        let offset = self.cursor_offset();
        self.history.push(EditAction::Insert {
            offset,
            text: ch.to_string(),
        });
        self.rope.insert_char(offset, ch);
        if ch == '\n' {
            self.cursor_line += 1;
            self.cursor_col = 0;
        } else {
            self.cursor_col += 1;
        }
    }

    pub fn delete_char_before_cursor(&mut self) {
        let offset = self.cursor_offset();
        if offset == 0 {
            return;
        }
        let ch = self.rope.char(offset - 1);
        self.history.push(EditAction::Delete {
            offset: offset - 1,
            text: ch.to_string(),
        });
        self.rope.remove(offset - 1..offset);
        if ch == '\n' {
            self.cursor_line -= 1;
            self.cursor_col = self.line_content_len(self.cursor_line);
        } else {
            self.cursor_col -= 1;
        }
    }

    pub fn delete_char_at_cursor(&mut self) {
        let offset = self.cursor_offset();
        if offset >= self.rope.len_chars() {
            return;
        }
        let ch = self.rope.char(offset);
        self.history.push(EditAction::Delete {
            offset,
            text: ch.to_string(),
        });
        self.rope.remove(offset..offset + 1);
    }

    pub fn delete_range(&mut self, start: usize, end: usize) {
        let text = self.rope.slice(start..end).to_string();
        self.history.push(EditAction::Delete {
            offset: start,
            text,
        });
        self.rope.remove(start..end);
    }

    pub fn insert_text_at(&mut self, offset: usize, text: &str) {
        self.history.push(EditAction::Insert {
            offset,
            text: text.to_string(),
        });
        self.rope.insert(offset, text);
    }

    /// Begin an undo group. All edits until end_undo_group() will be a single undo unit.
    /// Call when entering Insert mode.
    pub fn begin_undo_group(&mut self) {
        self.history.begin_group();
    }

    /// End the current undo group. Call when leaving Insert mode (Escape).
    pub fn end_undo_group(&mut self) {
        self.history.end_group();
    }

    fn replay_action_undo(&mut self, action: &EditAction) {
        match action {
            EditAction::Insert { offset, text } => {
                self.rope.remove(*offset..*offset + text.chars().count());
                self.update_cursor_from_offset(*offset);
            }
            EditAction::Delete { offset, text } => {
                self.rope.insert(*offset, text);
                self.update_cursor_from_offset(*offset + text.chars().count());
            }
        }
    }

    fn replay_action_redo(&mut self, action: &EditAction) {
        match action {
            EditAction::Insert { offset, text } => {
                self.rope.insert(*offset, text);
                self.update_cursor_from_offset(*offset + text.chars().count());
            }
            EditAction::Delete { offset, text } => {
                self.rope.remove(*offset..*offset + text.chars().count());
                self.update_cursor_from_offset(*offset);
            }
        }
    }

    pub fn undo(&mut self) {
        self.history.set_recording(false);
        if let Some(entry) = self.history.undo() {
            match entry {
                UndoEntry::Single(action) => {
                    self.replay_action_undo(&action);
                }
                UndoEntry::Group(actions) => {
                    // Replay in reverse order to undo a group
                    for action in actions.iter().rev() {
                        self.replay_action_undo(action);
                    }
                }
            }
        }
        self.history.set_recording(true);
    }

    pub fn redo(&mut self) {
        self.history.set_recording(false);
        if let Some(entry) = self.history.redo() {
            match entry {
                UndoEntry::Single(action) => {
                    self.replay_action_redo(&action);
                }
                UndoEntry::Group(actions) => {
                    // Replay in forward order to redo a group
                    for action in actions.iter() {
                        self.replay_action_redo(action);
                    }
                }
            }
        }
        self.history.set_recording(true);
    }
}
```

Note: `text.chars().count()` is used instead of `text.len()` because offsets are char-based, not byte-based.

- [ ] **Step 5: Add undo/redo tests to TextBuffer**

Add to `src/buffer/text_buffer.rs` tests module:

```rust
#[test]
fn undo_insert() {
    let mut buf = TextBuffer::new();
    buf.insert_char('a');
    buf.insert_char('b');
    assert_eq!(buf.text(), "ab");
    buf.undo();
    assert_eq!(buf.text(), "a");
    buf.undo();
    assert_eq!(buf.text(), "");
}

#[test]
fn redo_after_undo() {
    let mut buf = TextBuffer::new();
    buf.insert_char('a');
    buf.undo();
    assert_eq!(buf.text(), "");
    buf.redo();
    assert_eq!(buf.text(), "a");
}

#[test]
fn undo_delete_range() {
    let mut buf = TextBuffer::from_text("hello world");
    buf.delete_range(5, 11);
    assert_eq!(buf.text(), "hello");
    buf.undo();
    assert_eq!(buf.text(), "hello world");
}

#[test]
fn undo_unicode() {
    let mut buf = TextBuffer::new();
    buf.insert_char('å');
    buf.insert_char('ä');
    assert_eq!(buf.text(), "åä");
    buf.undo();
    assert_eq!(buf.text(), "å");
}

#[test]
fn undo_group_undoes_entire_insert_session() {
    let mut buf = TextBuffer::new();
    // Simulate: enter insert mode, type "abc", escape
    buf.begin_undo_group();
    buf.insert_char('a');
    buf.insert_char('b');
    buf.insert_char('c');
    buf.end_undo_group();

    assert_eq!(buf.text(), "abc");
    // Single undo should remove all three characters
    buf.undo();
    assert_eq!(buf.text(), "");
    // Redo restores them all
    buf.redo();
    assert_eq!(buf.text(), "abc");
}

#[test]
fn undo_group_then_single() {
    let mut buf = TextBuffer::new();
    // First: a grouped insert session
    buf.begin_undo_group();
    buf.insert_char('a');
    buf.insert_char('b');
    buf.end_undo_group();
    // Second: a single operator (e.g., dd)
    buf.delete_range(0, 2);

    assert_eq!(buf.text(), "");
    buf.undo(); // undoes delete_range
    assert_eq!(buf.text(), "ab");
    buf.undo(); // undoes the entire insert group
    assert_eq!(buf.text(), "");
}
```

- [ ] **Step 6: Update buffer/mod.rs**

```rust
// src/buffer/mod.rs
mod text_buffer;
pub mod history;

pub use text_buffer::TextBuffer;
```

- [ ] **Step 7: Run all tests**

Run: `cargo test --lib buffer`
Expected: All tests PASS.

- [ ] **Step 8: Commit**

```bash
git add src/buffer/
git commit -m "feat: auto-recording undo/redo history for TextBuffer"
```

---

## Task 4: Editor Rendering

**Files:**
- Create: `src/renderer/mod.rs`
- Create: `src/renderer/editor_view.rs`
- Create: `src/renderer/status_bar.rs`
- Create: `src/renderer/theme.rs`
- Modify: `src/app.rs`
- Modify: `src/main.rs`

Key fixes from review:
- All cursor rendering uses **char-based indexing** via `char_indices()`
- `font_id` and `char_width` cached outside the render loop
- `line_slice()` used instead of `line()` (zero-allocation)
- **Status bar** rendered at bottom showing current mode + file path
- **Command line** rendered when in Command mode
- **Cursor shape changes by mode**: block (Normal), line (Insert)

- [ ] **Step 1: Create the Theme struct with default dark theme**

```rust
// src/renderer/theme.rs
use eframe::egui::Color32;

pub struct SyntaxColors {
    pub keyword: Color32,
    pub string: Color32,
    pub comment: Color32,
    pub function: Color32,
    pub r#type: Color32,
    pub number: Color32,
}

pub struct Theme {
    pub name: String,
    pub background: Color32,
    pub foreground: Color32,
    pub cursor: Color32,
    pub cursor_insert: Color32,
    pub selection: Color32,
    pub line_number: Color32,
    pub line_number_active: Color32,
    pub gutter_background: Color32,
    pub status_bar_bg: Color32,
    pub status_bar_fg: Color32,
    pub syntax: SyntaxColors,
}

impl Theme {
    pub fn default_dark() -> Self {
        Self {
            name: "default-dark".into(),
            background: Color32::from_rgb(0x1e, 0x1e, 0x2e),
            foreground: Color32::from_rgb(0xcd, 0xd6, 0xf4),
            cursor: Color32::from_rgb(0xf5, 0xe0, 0xdc),
            cursor_insert: Color32::from_rgb(0xf5, 0xe0, 0xdc),
            selection: Color32::from_rgb(0x45, 0x47, 0x5a),
            line_number: Color32::from_rgb(0x6c, 0x70, 0x86),
            line_number_active: Color32::from_rgb(0xcd, 0xd6, 0xf4),
            gutter_background: Color32::from_rgb(0x1e, 0x1e, 0x2e),
            status_bar_bg: Color32::from_rgb(0x31, 0x32, 0x44),
            status_bar_fg: Color32::from_rgb(0xcd, 0xd6, 0xf4),
            syntax: SyntaxColors {
                keyword: Color32::from_rgb(0xcb, 0xa6, 0xf7),
                string: Color32::from_rgb(0xa6, 0xe3, 0xa1),
                comment: Color32::from_rgb(0x6c, 0x70, 0x86),
                function: Color32::from_rgb(0x89, 0xb4, 0xfa),
                r#type: Color32::from_rgb(0xf9, 0xe2, 0xaf),
                number: Color32::from_rgb(0xfa, 0xb3, 0x87),
            },
        }
    }
}
```

- [ ] **Step 2: Create StatusBar**

```rust
// src/renderer/status_bar.rs
use eframe::egui;
use crate::renderer::theme::Theme;
use crate::vim::mode::Mode;

pub struct StatusBar;

impl StatusBar {
    /// Renders the status bar at the bottom. Returns the height consumed.
    pub fn render(
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        rect: egui::Rect,
        theme: &Theme,
        font_id: &egui::FontId,
        line_height: f32,
        mode: Mode,
        file_path: Option<&str>,
        command_input: Option<&str>,
        status_message: Option<&str>,
    ) -> f32 {
        let bar_height = line_height + 4.0;
        let bar_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x, rect.max.y - bar_height),
            egui::vec2(rect.width(), bar_height),
        );
        painter.rect_filled(bar_rect, 0.0, theme.status_bar_bg);

        let text_y = bar_rect.min.y + 2.0;

        // Mode label
        let mode_text = mode.status_text();
        painter.text(
            egui::pos2(bar_rect.min.x + 10.0, text_y),
            egui::Align2::LEFT_TOP,
            mode_text,
            font_id.clone(),
            theme.status_bar_fg,
        );

        // Status message (temporary) or file path
        if let Some(msg) = status_message {
            painter.text(
                egui::pos2(bar_rect.min.x + 100.0, text_y),
                egui::Align2::LEFT_TOP,
                msg,
                font_id.clone(),
                theme.foreground,
            );
        } else if let Some(path) = file_path {
            painter.text(
                egui::pos2(bar_rect.min.x + 100.0, text_y),
                egui::Align2::LEFT_TOP,
                path,
                font_id.clone(),
                theme.line_number,
            );
        }

        // Command line input (when in command mode)
        if let Some(input) = command_input {
            let cmd_text = format!(":{}", input);
            let cmd_rect = egui::Rect::from_min_size(
                egui::pos2(rect.min.x, bar_rect.min.y - line_height - 2.0),
                egui::vec2(rect.width(), line_height + 2.0),
            );
            painter.rect_filled(cmd_rect, 0.0, theme.background);
            painter.text(
                egui::pos2(cmd_rect.min.x + 10.0, cmd_rect.min.y),
                egui::Align2::LEFT_TOP,
                &cmd_text,
                font_id.clone(),
                theme.foreground,
            );
        }

        bar_height
    }
}
```

- [ ] **Step 3: Create EditorView with char-safe rendering**

```rust
// src/renderer/editor_view.rs
use eframe::egui::{self, Rect, Sense, Vec2};
use crate::buffer::TextBuffer;
use crate::renderer::theme::Theme;
use crate::renderer::status_bar::StatusBar;
use crate::vim::mode::Mode;

pub struct EditorView {
    pub scroll_offset: usize,
}

impl EditorView {
    pub fn new() -> Self {
        Self { scroll_offset: 0 }
    }

    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        buffer: &TextBuffer,
        theme: &Theme,
        mode: Mode,
        font_size: f32,
        file_path: Option<&str>,
        command_input: Option<&str>,
        status_message: Option<&str>,
    ) {
        let font_id = egui::FontId::monospace(font_size);
        let line_height = ui.fonts(|f| f.row_height(&font_id));
        let char_width = ui.fonts(|f| {
            f.layout_no_wrap("m".to_string(), font_id.clone(), theme.foreground)
                .rect
                .width()
        });
        let available = ui.available_size();

        // Allocate painter
        let (response, painter) = ui.allocate_painter(available, Sense::click_and_drag());
        let rect = response.rect;
        painter.rect_filled(rect, 0.0, theme.background);

        // Status bar at bottom
        let status_height = StatusBar::render(
            ui, &painter, rect, theme, &font_id, line_height,
            mode, file_path, command_input, status_message,
        );

        let editor_height = available.y - status_height;
        let visible_lines = (editor_height / line_height) as usize;
        let gutter_width = 50.0;
        let text_x = rect.min.x + gutter_width + 10.0;

        let end_line = (self.scroll_offset + visible_lines).min(buffer.line_count());

        for i in self.scroll_offset..end_line {
            let y = rect.min.y + ((i - self.scroll_offset) as f32) * line_height;
            let line_slice = buffer.line_slice(i);
            let line_str = line_slice.to_string();
            let display = line_str.trim_end_matches('\n');

            // Line number
            let line_num = format!("{:>4}", i + 1);
            let num_color = if i == buffer.cursor_line() {
                theme.line_number_active
            } else {
                theme.line_number
            };
            painter.text(
                egui::pos2(rect.min.x + 5.0, y),
                egui::Align2::LEFT_TOP,
                &line_num,
                font_id.clone(),
                num_color,
            );

            // Text content
            painter.text(
                egui::pos2(text_x, y),
                egui::Align2::LEFT_TOP,
                display,
                font_id.clone(),
                theme.foreground,
            );

            // Cursor (only on cursor line)
            if i == buffer.cursor_line() {
                let cursor_col = buffer.cursor_col();

                // Calculate cursor x position using char_indices (char-safe)
                let cursor_x: f32 = if cursor_col == 0 {
                    0.0
                } else {
                    let prefix: String = display.chars().take(cursor_col).collect();
                    painter
                        .layout_no_wrap(prefix, font_id.clone(), theme.foreground)
                        .rect
                        .width()
                };

                match mode {
                    Mode::Normal | Mode::Command => {
                        // Block cursor
                        let cursor_rect = Rect::from_min_size(
                            egui::pos2(text_x + cursor_x, y),
                            Vec2::new(char_width, line_height),
                        );
                        painter.rect_filled(cursor_rect, 0.0, theme.cursor);

                        // Draw character under cursor with inverted color
                        if let Some(ch) = display.chars().nth(cursor_col) {
                            painter.text(
                                egui::pos2(text_x + cursor_x, y),
                                egui::Align2::LEFT_TOP,
                                &ch.to_string(),
                                font_id.clone(),
                                theme.background,
                            );
                        }
                    }
                    Mode::Insert => {
                        // Thin line cursor (2px wide)
                        let cursor_rect = Rect::from_min_size(
                            egui::pos2(text_x + cursor_x, y),
                            Vec2::new(2.0, line_height),
                        );
                        painter.rect_filled(cursor_rect, 0.0, theme.cursor_insert);
                    }
                }
            }
        }

        // Scroll follow
        if buffer.cursor_line() < self.scroll_offset {
            self.scroll_offset = buffer.cursor_line();
        } else if buffer.cursor_line() >= self.scroll_offset + visible_lines {
            self.scroll_offset = buffer.cursor_line() - visible_lines + 1;
        }
    }
}
```

- [ ] **Step 4: Create renderer module**

```rust
// src/renderer/mod.rs
mod editor_view;
mod status_bar;
pub mod theme;

pub use editor_view::EditorView;
pub use theme::Theme;
```

- [ ] **Step 5: Update NyxApp to use EditorView**

```rust
// src/app.rs
use crate::buffer::TextBuffer;
use crate::renderer::{EditorView, Theme};
use crate::vim::mode::Mode;

pub struct NyxApp {
    buffer: TextBuffer,
    editor_view: EditorView,
    theme: Theme,
    mode: Mode,
}

impl NyxApp {
    pub fn new() -> Self {
        Self {
            buffer: TextBuffer::from_text(
                "Welcome to Nyx!\n\nPress i to enter insert mode.\nPress : for commands.\nPress :q to quit.\n"
            ),
            editor_view: EditorView::new(),
            theme: Theme::default_dark(),
            mode: Mode::Normal,
        }
    }
}

impl eframe::App for NyxApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        eframe::egui::CentralPanel::default()
            .frame(eframe::egui::Frame::NONE)
            .show(ctx, |ui| {
                self.editor_view.render(
                    ui, &self.buffer, &self.theme, self.mode, 14.0, None, None, None,
                );
            });
    }
}
```

- [ ] **Step 6: Update main.rs**

```rust
// src/main.rs
mod app;
mod buffer;
mod renderer;
mod vim;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("Nyx"),
        ..Default::default()
    };
    eframe::run_native("Nyx", options, Box::new(|_cc| Ok(Box::new(app::NyxApp::new()))))
}
```

- [ ] **Step 7: Verify rendering**

Run: `cargo run`
Expected: Window opens with dark background, welcome text, line numbers, block cursor on line 1, status bar at bottom showing "NORMAL".

- [ ] **Step 8: Commit**

```bash
git add src/renderer/ src/app.rs src/main.rs
git commit -m "feat: editor rendering with status bar, mode-aware cursor, and char-safe indexing"
```

---

## Task 5: Vim Types and Mode System

**Files:**
- Create: `src/vim/mod.rs`
- Create: `src/vim/action.rs`
- Create: `src/vim/mode.rs`
- Create: `src/vim/keyparser.rs`

Key fixes from review:
- Types (`VimAction`, `MotionKind`, `OperatorAction`) live in their own file `action.rs`
- `KeyParser` supports **count prefix** (`5j`, `3dw`)
- Includes `a`, `A`, `o`, `O`, `I` insert entry points
- `Ctrl+[` handled as Escape alias

- [ ] **Step 1: Create action types**

```rust
// src/vim/action.rs
use crate::vim::mode::Mode;

#[derive(Debug, Clone, PartialEq)]
pub enum VimAction {
    SwitchMode(Mode),
    Motion(MotionKind),
    InsertChar(char),
    DeleteCharBefore,
    Operator(OperatorAction),
    Yank(MotionKind),
    Paste,
    Undo,
    Redo,
    /// Enter insert mode, optionally with a preparatory motion/action
    EnterInsert(InsertEntry),
    Noop,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InsertEntry {
    AtCursor,         // i
    AfterCursor,      // a
    EndOfLine,        // A
    FirstNonBlank,    // I
    NewLineBelow,     // o
    NewLineAbove,     // O
}

#[derive(Debug, Clone, PartialEq)]
pub enum MotionKind {
    Left,
    Down,
    Up,
    Right,
    LineStart,
    FirstNonBlank,
    LineEnd,
    WordForward,
    WordBackward,
    WordEnd,
    FileTop,
    FileBottom,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OperatorAction {
    Delete(MotionKind),
    Change(MotionKind),
    DeleteLine,
    ChangeLine,
    YankLine,
}
```

- [ ] **Step 2: Create Mode**

```rust
// src/vim/mode.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Command,
}

impl Mode {
    pub fn status_text(&self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Command => "COMMAND",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_status_text() {
        assert_eq!(Mode::Normal.status_text(), "NORMAL");
        assert_eq!(Mode::Insert.status_text(), "INSERT");
        assert_eq!(Mode::Command.status_text(), "COMMAND");
    }
}
```

- [ ] **Step 3: Write failing tests for KeyParser with count prefix**

```rust
// src/vim/keyparser.rs
use crate::vim::action::*;
use crate::vim::mode::Mode;

pub struct KeyParser {
    mode: Mode,
    pending: String,
    count: Option<usize>,
}

// Note: Visual, Visual-Line, and Visual-Block modes are planned for Phase 2.
// Dot-repeat (`.`) is planned for Phase 2 — will require storing last action in Editor.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_mode_i_enters_insert() {
        let mut parser = KeyParser::new();
        let action = parser.handle_key('i');
        assert_eq!(action, VimAction::EnterInsert(InsertEntry::AtCursor));
        assert_eq!(parser.mode(), Mode::Insert);
    }

    #[test]
    fn normal_mode_a_enters_insert_after() {
        let mut parser = KeyParser::new();
        let action = parser.handle_key('a');
        assert_eq!(action, VimAction::EnterInsert(InsertEntry::AfterCursor));
        assert_eq!(parser.mode(), Mode::Insert);
    }

    #[test]
    fn normal_mode_shift_a_enters_insert_eol() {
        let mut parser = KeyParser::new();
        let action = parser.handle_key('A');
        assert_eq!(action, VimAction::EnterInsert(InsertEntry::EndOfLine));
        assert_eq!(parser.mode(), Mode::Insert);
    }

    #[test]
    fn normal_mode_shift_i_enters_insert_first_nonblank() {
        let mut parser = KeyParser::new();
        let action = parser.handle_key('I');
        assert_eq!(action, VimAction::EnterInsert(InsertEntry::FirstNonBlank));
        assert_eq!(parser.mode(), Mode::Insert);
    }

    #[test]
    fn normal_mode_o_opens_line_below() {
        let mut parser = KeyParser::new();
        let action = parser.handle_key('o');
        assert_eq!(action, VimAction::EnterInsert(InsertEntry::NewLineBelow));
        assert_eq!(parser.mode(), Mode::Insert);
    }

    #[test]
    fn normal_mode_shift_o_opens_line_above() {
        let mut parser = KeyParser::new();
        let action = parser.handle_key('O');
        assert_eq!(action, VimAction::EnterInsert(InsertEntry::NewLineAbove));
        assert_eq!(parser.mode(), Mode::Insert);
    }

    #[test]
    fn escape_returns_to_normal() {
        let mut parser = KeyParser::new();
        parser.set_mode(Mode::Insert);
        let action = parser.handle_escape();
        assert_eq!(action, VimAction::SwitchMode(Mode::Normal));
    }

    #[test]
    fn normal_mode_hjkl_motions() {
        let mut parser = KeyParser::new();
        assert_eq!(parser.handle_key('h'), VimAction::Motion(MotionKind::Left));
        assert_eq!(parser.handle_key('j'), VimAction::Motion(MotionKind::Down));
        assert_eq!(parser.handle_key('k'), VimAction::Motion(MotionKind::Up));
        assert_eq!(parser.handle_key('l'), VimAction::Motion(MotionKind::Right));
    }

    #[test]
    fn normal_mode_word_motions() {
        let mut parser = KeyParser::new();
        assert_eq!(parser.handle_key('w'), VimAction::Motion(MotionKind::WordForward));
        assert_eq!(parser.handle_key('b'), VimAction::Motion(MotionKind::WordBackward));
        assert_eq!(parser.handle_key('e'), VimAction::Motion(MotionKind::WordEnd));
    }

    #[test]
    fn normal_mode_line_motions() {
        let mut parser = KeyParser::new();
        assert_eq!(parser.handle_key('0'), VimAction::Motion(MotionKind::LineStart));
        assert_eq!(parser.handle_key('$'), VimAction::Motion(MotionKind::LineEnd));
        assert_eq!(parser.handle_key('^'), VimAction::Motion(MotionKind::FirstNonBlank));
    }

    #[test]
    fn normal_mode_gg_and_G() {
        let mut parser = KeyParser::new();
        assert_eq!(parser.handle_key('g'), VimAction::Noop);
        assert_eq!(parser.handle_key('g'), VimAction::Motion(MotionKind::FileTop));
        assert_eq!(parser.handle_key('G'), VimAction::Motion(MotionKind::FileBottom));
    }

    #[test]
    fn normal_mode_undo_redo() {
        let mut parser = KeyParser::new();
        assert_eq!(parser.handle_key('u'), VimAction::Undo);
    }

    #[test]
    fn insert_mode_regular_char() {
        let mut parser = KeyParser::new();
        parser.set_mode(Mode::Insert);
        assert_eq!(parser.handle_key('a'), VimAction::InsertChar('a'));
    }

    #[test]
    fn colon_enters_command() {
        let mut parser = KeyParser::new();
        assert_eq!(parser.handle_key(':'), VimAction::SwitchMode(Mode::Command));
    }

    #[test]
    fn count_prefix_with_motion() {
        let mut parser = KeyParser::new();
        assert_eq!(parser.handle_key('5'), VimAction::Noop);
        let action = parser.handle_key('j');
        assert_eq!(action, VimAction::Motion(MotionKind::Down));
        // count should have been 5, consumed by handle_key
        // The count is read via take_count() after producing the action
    }

    #[test]
    fn count_prefix_parsing() {
        let mut parser = KeyParser::new();
        parser.handle_key('1');
        parser.handle_key('2');
        assert_eq!(parser.take_count(), 12);
    }

    #[test]
    fn no_count_returns_1() {
        let mut parser = KeyParser::new();
        assert_eq!(parser.take_count(), 1);
    }
}
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test --lib vim`
Expected: FAIL.

- [ ] **Step 5: Implement KeyParser**

```rust
// src/vim/keyparser.rs
use crate::vim::action::*;
use crate::vim::mode::Mode;

pub struct KeyParser {
    mode: Mode,
    pending: String,
    count: Option<usize>,
}

// Note: Visual, Visual-Line, and Visual-Block modes are planned for Phase 2.
// Dot-repeat (`.`) is planned for Phase 2 — will require storing last action in Editor.

impl KeyParser {
    pub fn new() -> Self {
        Self {
            mode: Mode::Normal,
            pending: String::new(),
            count: None,
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Returns the accumulated count, consuming it. Returns 1 if no count was set.
    pub fn take_count(&mut self) -> usize {
        self.count.take().unwrap_or(1)
    }

    #[cfg(test)]
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    pub fn handle_key(&mut self, ch: char) -> VimAction {
        match self.mode {
            Mode::Normal => self.handle_normal(ch),
            Mode::Insert => self.handle_insert(ch),
            Mode::Command => VimAction::Noop, // command input handled separately
        }
    }

    pub fn handle_escape(&mut self) -> VimAction {
        self.pending.clear();
        self.count = None;
        self.mode = Mode::Normal;
        VimAction::SwitchMode(Mode::Normal)
    }

    pub fn handle_backspace(&mut self) -> VimAction {
        match self.mode {
            Mode::Insert => VimAction::DeleteCharBefore,
            _ => VimAction::Noop,
        }
    }

    pub fn handle_ctrl_r(&mut self) -> VimAction {
        match self.mode {
            Mode::Normal => VimAction::Redo,
            _ => VimAction::Noop,
        }
    }

    fn handle_normal(&mut self, ch: char) -> VimAction {
        // Count prefix: digits 1-9 start a count, 0 after digits continues count
        // Uses saturating arithmetic and caps at 99999 to prevent overflow
        if ch.is_ascii_digit() && (ch != '0' || self.count.is_some()) {
            let digit = ch.to_digit(10).unwrap() as usize;
            let current = self.count.unwrap_or(0);
            self.count = Some(current.saturating_mul(10).saturating_add(digit).min(99_999));
            return VimAction::Noop;
        }

        // Pending multi-char sequences
        if !self.pending.is_empty() {
            let combined = format!("{}{}", self.pending, ch);
            self.pending.clear();
            return match combined.as_str() {
                "gg" => VimAction::Motion(MotionKind::FileTop),
                "dd" => VimAction::Operator(OperatorAction::DeleteLine),
                "cc" => {
                    self.mode = Mode::Insert;
                    VimAction::Operator(OperatorAction::ChangeLine)
                }
                "yy" => VimAction::Operator(OperatorAction::YankLine),
                s if s.starts_with('d') => {
                    if let Some(motion) = Self::char_to_motion(ch) {
                        VimAction::Operator(OperatorAction::Delete(motion))
                    } else {
                        VimAction::Noop
                    }
                }
                s if s.starts_with('c') => {
                    if let Some(motion) = Self::char_to_motion(ch) {
                        self.mode = Mode::Insert;
                        VimAction::Operator(OperatorAction::Change(motion))
                    } else {
                        VimAction::Noop
                    }
                }
                s if s.starts_with('y') => {
                    if let Some(motion) = Self::char_to_motion(ch) {
                        VimAction::Yank(motion)
                    } else {
                        VimAction::Noop
                    }
                }
                _ => VimAction::Noop,
            };
        }

        match ch {
            // Motions
            'h' => VimAction::Motion(MotionKind::Left),
            'j' => VimAction::Motion(MotionKind::Down),
            'k' => VimAction::Motion(MotionKind::Up),
            'l' => VimAction::Motion(MotionKind::Right),
            '0' => VimAction::Motion(MotionKind::LineStart),
            '^' => VimAction::Motion(MotionKind::FirstNonBlank),
            '$' => VimAction::Motion(MotionKind::LineEnd),
            'w' => VimAction::Motion(MotionKind::WordForward),
            'b' => VimAction::Motion(MotionKind::WordBackward),
            'e' => VimAction::Motion(MotionKind::WordEnd),
            'G' => VimAction::Motion(MotionKind::FileBottom),

            // Pending sequences
            'g' | 'd' | 'c' | 'y' => {
                self.pending.push(ch);
                VimAction::Noop
            }

            // Operators
            'p' => VimAction::Paste,
            'x' => VimAction::Operator(OperatorAction::Delete(MotionKind::Right)),
            'u' => VimAction::Undo,

            // Insert entry points
            'i' => {
                self.mode = Mode::Insert;
                VimAction::EnterInsert(InsertEntry::AtCursor)
            }
            'a' => {
                self.mode = Mode::Insert;
                VimAction::EnterInsert(InsertEntry::AfterCursor)
            }
            'A' => {
                self.mode = Mode::Insert;
                VimAction::EnterInsert(InsertEntry::EndOfLine)
            }
            'I' => {
                self.mode = Mode::Insert;
                VimAction::EnterInsert(InsertEntry::FirstNonBlank)
            }
            'o' => {
                self.mode = Mode::Insert;
                VimAction::EnterInsert(InsertEntry::NewLineBelow)
            }
            'O' => {
                self.mode = Mode::Insert;
                VimAction::EnterInsert(InsertEntry::NewLineAbove)
            }

            // Command mode
            ':' => {
                self.mode = Mode::Command;
                VimAction::SwitchMode(Mode::Command)
            }

            _ => VimAction::Noop,
        }
    }

    fn handle_insert(&mut self, ch: char) -> VimAction {
        VimAction::InsertChar(ch)
    }

    fn char_to_motion(ch: char) -> Option<MotionKind> {
        match ch {
            'h' => Some(MotionKind::Left),
            'j' => Some(MotionKind::Down),
            'k' => Some(MotionKind::Up),
            'l' => Some(MotionKind::Right),
            'w' => Some(MotionKind::WordForward),
            'b' => Some(MotionKind::WordBackward),
            'e' => Some(MotionKind::WordEnd),
            '0' => Some(MotionKind::LineStart),
            '^' => Some(MotionKind::FirstNonBlank),
            '$' => Some(MotionKind::LineEnd),
            'G' => Some(MotionKind::FileBottom),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    // ... (tests from Step 3)
}
```

- [ ] **Step 6: Create vim module**

```rust
// src/vim/mod.rs
pub mod action;
pub mod mode;
mod keyparser;
mod motion;
mod operator;
pub mod command;

pub use action::*;
pub use mode::Mode;
pub use keyparser::KeyParser;
```

Note: `motion` and `operator` modules are created in Tasks 6 and 8. For now, create empty placeholder files:

```rust
// src/vim/motion.rs
// Implemented in Task 6

// src/vim/operator.rs
// Implemented in Task 8

// src/vim/command.rs
// Implemented in Task 9
```

- [ ] **Step 7: Add vim module to main.rs**

Add `mod vim;` to `src/main.rs`.

- [ ] **Step 8: Run tests**

Run: `cargo test --lib vim`
Expected: All tests PASS.

- [ ] **Step 9: Commit**

```bash
git add src/vim/ src/main.rs
git commit -m "feat: vim types, mode system, and keyparser with count prefix"
```

---

## Task 6: Motion Execution

**Files:**
- Create: `src/vim/motion.rs`

Key fixes from review:
- All word motions use **char-based iteration** via `chars()` / `char_indices()` (never `as_bytes()`)
- Empty buffer guards using `saturating_sub` everywhere
- `FirstNonBlank` motion (`^`) added

- [ ] **Step 1: Write failing tests for motion execution**

```rust
// src/vim/motion.rs
use crate::buffer::TextBuffer;
use crate::vim::action::MotionKind;

pub fn execute_motion(buffer: &mut TextBuffer, motion: &MotionKind) {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_left() {
        let mut buf = TextBuffer::from_text("hello");
        buf.set_cursor(0, 3);
        execute_motion(&mut buf, &MotionKind::Left);
        assert_eq!(buf.cursor_col(), 2);
    }

    #[test]
    fn motion_left_at_start_stays() {
        let mut buf = TextBuffer::from_text("hello");
        execute_motion(&mut buf, &MotionKind::Left);
        assert_eq!(buf.cursor_col(), 0);
    }

    #[test]
    fn motion_right() {
        let mut buf = TextBuffer::from_text("hello");
        execute_motion(&mut buf, &MotionKind::Right);
        assert_eq!(buf.cursor_col(), 1);
    }

    #[test]
    fn motion_right_stops_before_newline() {
        let mut buf = TextBuffer::from_text("hi\nworld");
        buf.set_cursor(0, 1);
        execute_motion(&mut buf, &MotionKind::Right);
        assert_eq!(buf.cursor_col(), 1); // can't go past 'i' in normal mode
    }

    #[test]
    fn motion_down() {
        let mut buf = TextBuffer::from_text("hello\nworld");
        execute_motion(&mut buf, &MotionKind::Down);
        assert_eq!(buf.cursor_line(), 1);
    }

    #[test]
    fn motion_down_clamps_col() {
        let mut buf = TextBuffer::from_text("hello\nhi");
        buf.set_cursor(0, 4);
        execute_motion(&mut buf, &MotionKind::Down);
        assert_eq!(buf.cursor_line(), 1);
        assert_eq!(buf.cursor_col(), 1); // "hi" max col 1 in normal mode
    }

    #[test]
    fn motion_up() {
        let mut buf = TextBuffer::from_text("hello\nworld");
        buf.set_cursor(1, 0);
        execute_motion(&mut buf, &MotionKind::Up);
        assert_eq!(buf.cursor_line(), 0);
    }

    #[test]
    fn motion_line_start() {
        let mut buf = TextBuffer::from_text("hello");
        buf.set_cursor(0, 3);
        execute_motion(&mut buf, &MotionKind::LineStart);
        assert_eq!(buf.cursor_col(), 0);
    }

    #[test]
    fn motion_first_non_blank() {
        let mut buf = TextBuffer::from_text("   hello");
        buf.set_cursor(0, 6);
        execute_motion(&mut buf, &MotionKind::FirstNonBlank);
        assert_eq!(buf.cursor_col(), 3); // first non-space char
    }

    #[test]
    fn motion_line_end() {
        let mut buf = TextBuffer::from_text("hello\nworld");
        execute_motion(&mut buf, &MotionKind::LineEnd);
        assert_eq!(buf.cursor_col(), 4); // last char 'o', 0-indexed
    }

    #[test]
    fn motion_file_top() {
        let mut buf = TextBuffer::from_text("a\nb\nc");
        buf.set_cursor(2, 0);
        execute_motion(&mut buf, &MotionKind::FileTop);
        assert_eq!(buf.cursor_line(), 0);
        assert_eq!(buf.cursor_col(), 0);
    }

    #[test]
    fn motion_file_bottom() {
        let mut buf = TextBuffer::from_text("a\nb\nc");
        execute_motion(&mut buf, &MotionKind::FileBottom);
        assert_eq!(buf.cursor_line(), 2);
    }

    #[test]
    fn motion_word_forward() {
        let mut buf = TextBuffer::from_text("hello world foo");
        execute_motion(&mut buf, &MotionKind::WordForward);
        assert_eq!(buf.cursor_col(), 6);
    }

    #[test]
    fn motion_word_backward() {
        let mut buf = TextBuffer::from_text("hello world");
        buf.set_cursor(0, 8);
        execute_motion(&mut buf, &MotionKind::WordBackward);
        assert_eq!(buf.cursor_col(), 6);
    }

    #[test]
    fn motion_word_end() {
        let mut buf = TextBuffer::from_text("hello world");
        execute_motion(&mut buf, &MotionKind::WordEnd);
        assert_eq!(buf.cursor_col(), 4); // 'o' of 'hello'
    }

    #[test]
    fn motion_word_forward_unicode() {
        let mut buf = TextBuffer::from_text("hej på dig");
        execute_motion(&mut buf, &MotionKind::WordForward);
        assert_eq!(buf.cursor_col(), 4); // start of 'på'
    }

    #[test]
    fn motion_on_empty_buffer() {
        let mut buf = TextBuffer::new();
        // None of these should panic
        execute_motion(&mut buf, &MotionKind::Left);
        execute_motion(&mut buf, &MotionKind::Right);
        execute_motion(&mut buf, &MotionKind::Up);
        execute_motion(&mut buf, &MotionKind::Down);
        execute_motion(&mut buf, &MotionKind::FileBottom);
        execute_motion(&mut buf, &MotionKind::LineEnd);
        assert_eq!(buf.cursor_line(), 0);
        assert_eq!(buf.cursor_col(), 0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib vim::motion`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 3: Implement motion execution (char-safe)**

```rust
// src/vim/motion.rs
use crate::buffer::TextBuffer;
use crate::vim::action::MotionKind;

pub fn execute_motion(buffer: &mut TextBuffer, motion: &MotionKind) {
    match motion {
        MotionKind::Left => {
            let col = buffer.cursor_col().saturating_sub(1);
            buffer.set_cursor(buffer.cursor_line(), col);
        }
        MotionKind::Right => {
            let content_len = buffer.line_content_len(buffer.cursor_line());
            let max_col = content_len.saturating_sub(1);
            let new_col = (buffer.cursor_col() + 1).min(max_col);
            buffer.set_cursor(buffer.cursor_line(), new_col);
        }
        MotionKind::Down => {
            if buffer.cursor_line() < buffer.line_count().saturating_sub(1) {
                buffer.set_cursor(buffer.cursor_line() + 1, buffer.cursor_col());
            }
        }
        MotionKind::Up => {
            buffer.set_cursor(buffer.cursor_line().saturating_sub(1), buffer.cursor_col());
        }
        MotionKind::LineStart => {
            buffer.set_cursor(buffer.cursor_line(), 0);
        }
        MotionKind::FirstNonBlank => {
            let line = buffer.line_slice(buffer.cursor_line()).to_string();
            let col = line.chars()
                .take_while(|c| c.is_whitespace() && *c != '\n')
                .count();
            buffer.set_cursor(buffer.cursor_line(), col);
        }
        MotionKind::LineEnd => {
            let content_len = buffer.line_content_len(buffer.cursor_line());
            buffer.set_cursor(buffer.cursor_line(), content_len.saturating_sub(1));
        }
        MotionKind::FileTop => {
            buffer.set_cursor(0, 0);
        }
        MotionKind::FileBottom => {
            let last_line = buffer.line_count().saturating_sub(1);
            buffer.set_cursor(last_line, buffer.cursor_col());
        }
        MotionKind::WordForward => {
            word_forward(buffer);
        }
        MotionKind::WordBackward => {
            word_backward(buffer);
        }
        MotionKind::WordEnd => {
            word_end(buffer);
        }
    }
}

fn word_forward(buffer: &mut TextBuffer) {
    let line = buffer.line_slice(buffer.cursor_line()).to_string();
    let content: Vec<char> = line.trim_end_matches('\n').chars().collect();
    let mut col = buffer.cursor_col();

    // Skip current word
    while col < content.len() && !content[col].is_whitespace() {
        col += 1;
    }
    // Skip whitespace
    while col < content.len() && content[col].is_whitespace() {
        col += 1;
    }

    if col >= content.len() && buffer.cursor_line() < buffer.line_count().saturating_sub(1) {
        buffer.set_cursor(buffer.cursor_line() + 1, 0);
    } else {
        buffer.set_cursor(buffer.cursor_line(), col.min(content.len().saturating_sub(1)));
    }
}

fn word_backward(buffer: &mut TextBuffer) {
    let col = buffer.cursor_col();

    if col == 0 {
        if buffer.cursor_line() > 0 {
            let prev_line = buffer.cursor_line() - 1;
            let prev_content_len = buffer.line_content_len(prev_line);
            buffer.set_cursor(prev_line, prev_content_len.saturating_sub(1));
        }
        return;
    }

    let line = buffer.line_slice(buffer.cursor_line()).to_string();
    let content: Vec<char> = line.trim_end_matches('\n').chars().collect();
    let mut c = col - 1;

    // Skip whitespace
    while c > 0 && content[c].is_whitespace() {
        c -= 1;
    }
    // Skip word
    while c > 0 && !content[c - 1].is_whitespace() {
        c -= 1;
    }

    buffer.set_cursor(buffer.cursor_line(), c);
}

fn word_end(buffer: &mut TextBuffer) {
    let line = buffer.line_slice(buffer.cursor_line()).to_string();
    let content: Vec<char> = line.trim_end_matches('\n').chars().collect();
    let mut col = buffer.cursor_col();

    if col >= content.len().saturating_sub(1) {
        return;
    }

    col += 1;
    // Skip whitespace
    while col < content.len() && content[col].is_whitespace() {
        col += 1;
    }
    // Go to end of word
    while col < content.len().saturating_sub(1) && !content[col + 1].is_whitespace() {
        col += 1;
    }

    buffer.set_cursor(buffer.cursor_line(), col);
}

#[cfg(test)]
mod tests {
    // ... (tests from Step 1)
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib vim::motion`
Expected: All tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/vim/motion.rs
git commit -m "feat: char-safe vim motion execution with Unicode support"
```

---

## Task 7: Wire Vim Input to App (via Editor struct)

**Files:**
- Create: `src/editor.rs`
- Modify: `src/app.rs`
- Modify: `src/main.rs`

Key fix from review: Extract an `Editor` struct that owns buffer + vim state. `NyxApp` becomes a thin eframe adapter.

- [ ] **Step 1: Create Editor struct**

```rust
// src/editor.rs
use crate::buffer::TextBuffer;
use crate::vim::{KeyParser, VimAction, MotionKind, InsertEntry, Mode};
use crate::vim::motion::execute_motion;

pub struct Editor {
    pub buffer: TextBuffer,
    pub key_parser: KeyParser,
    pub file_path: Option<String>,
    pub should_quit: bool,
    pub status_message: Option<String>,
}

impl Editor {
    pub fn new(file_path: Option<String>) -> Self {
        let buffer = if let Some(ref path) = file_path {
            match std::fs::read_to_string(path) {
                Ok(content) => TextBuffer::from_text(&content),
                Err(_) => TextBuffer::new(),
            }
        } else {
            TextBuffer::from_text(
                "Welcome to Nyx!\n\nPress i to enter insert mode.\nPress : for commands.\nPress :q to quit.\n"
            )
        };

        Self {
            buffer,
            key_parser: KeyParser::new(),
            file_path,
            should_quit: false,
            status_message: None,
        }
    }

    pub fn mode(&self) -> Mode {
        self.key_parser.mode()
    }

    pub fn apply_action(&mut self, action: VimAction) {
        // IMPORTANT: Do not consume count on Noop — digits accumulate count
        // and return Noop, so consuming here would lose the count.
        if action == VimAction::Noop {
            return;
        }
        // Clear status message on any non-noop action
        self.status_message = None;

        let count = self.key_parser.take_count();
        match action {
            VimAction::SwitchMode(Mode::Normal) => {
                // Leaving Insert/Command → Normal mode:
                // 1. End undo group (groups the entire insert session)
                // 2. Move cursor one left (vim behavior: Escape in Insert moves back)
                // 3. Clamp cursor to Normal mode bounds
                self.buffer.end_undo_group();
                let col = self.buffer.cursor_col();
                if col > 0 {
                    self.buffer.set_cursor(self.buffer.cursor_line(), col - 1);
                }
                self.buffer.clamp_cursor_normal();
            }
            VimAction::SwitchMode(_) => {
                // Other mode switches (e.g., to Command)
            }
            VimAction::Motion(ref motion) => {
                for _ in 0..count {
                    execute_motion(&mut self.buffer, motion);
                }
            }
            VimAction::InsertChar(ch) => {
                self.buffer.insert_char(ch);
            }
            VimAction::DeleteCharBefore => {
                self.buffer.delete_char_before_cursor();
            }
            VimAction::EnterInsert(entry) => {
                // Begin undo group so the entire insert session is one undo unit
                self.buffer.begin_undo_group();
                self.handle_insert_entry(entry);
            }
            VimAction::Undo => {
                for _ in 0..count {
                    self.buffer.undo();
                }
            }
            VimAction::Redo => {
                for _ in 0..count {
                    self.buffer.redo();
                }
            }
            VimAction::Operator(_) | VimAction::Yank(_) | VimAction::Paste => {
                // Handled in Task 8
            }
            VimAction::Noop => unreachable!(),
        }
    }

    fn handle_insert_entry(&mut self, entry: InsertEntry) {
        match entry {
            InsertEntry::AtCursor => {
                // i — cursor stays
            }
            InsertEntry::AfterCursor => {
                // a — move cursor right by one (Insert mode allows past last char)
                let content_len = self.buffer.line_content_len(self.buffer.cursor_line());
                let new_col = (self.buffer.cursor_col() + 1).min(content_len);
                self.buffer.set_cursor_with_mode(self.buffer.cursor_line(), new_col, true);
            }
            InsertEntry::EndOfLine => {
                // A — move to end of line (Insert mode: past last char)
                let content_len = self.buffer.line_content_len(self.buffer.cursor_line());
                self.buffer.set_cursor_with_mode(self.buffer.cursor_line(), content_len, true);
            }
            InsertEntry::FirstNonBlank => {
                // I — move to first non-blank char
                let line = self.buffer.line_slice(self.buffer.cursor_line()).to_string();
                let col = line.chars().take_while(|c| c.is_whitespace() && *c != '\n').count();
                self.buffer.set_cursor_with_mode(self.buffer.cursor_line(), col, true);
            }
            InsertEntry::NewLineBelow => {
                // o — open line below using line_to_char for reliable offset
                let line = self.buffer.cursor_line();
                let next_line_offset = if line + 1 < self.buffer.line_count() {
                    self.buffer.line_to_char(line + 1)
                } else {
                    self.buffer.len_chars()
                };
                // Insert newline at end of current line (after any trailing \n, or at EOF)
                self.buffer.insert_text_at(next_line_offset, "\n");
                self.buffer.set_cursor(line + 1, 0);
            }
            InsertEntry::NewLineAbove => {
                // O — open line above using line_to_char for reliable offset
                let line = self.buffer.cursor_line();
                let line_start = self.buffer.line_to_char(line);
                self.buffer.insert_text_at(line_start, "\n");
                self.buffer.set_cursor(line, 0);
            }
        }
    }
}
```

- [ ] **Step 2: Update NyxApp to use Editor**

```rust
// src/app.rs
use eframe::egui;
use crate::editor::Editor;
use crate::renderer::{EditorView, Theme};
use crate::vim::{VimAction, Mode};

pub struct NyxApp {
    editor: Editor,
    editor_view: EditorView,
    theme: Theme,
}

impl NyxApp {
    pub fn new(file_path: Option<String>) -> Self {
        Self {
            editor: Editor::new(file_path),
            editor_view: EditorView::new(),
            theme: Theme::default_dark(),
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        ctx.input(|input| {
            // Command mode intercepts all input
            if self.editor.mode() == Mode::Command {
                if input.key_pressed(egui::Key::Enter) {
                    self.editor.execute_command();
                    return;
                }
                if input.key_pressed(egui::Key::Backspace) {
                    self.editor.handle_command_backspace();
                    return;
                }
                if input.key_pressed(egui::Key::Escape)
                    || (input.modifiers.ctrl && input.key_pressed(egui::Key::OpenBracket))
                {
                    self.editor.command_parser.clear();
                    let action = self.editor.key_parser.handle_escape();
                    self.editor.apply_action(action);
                    return;
                }
                for event in &input.events {
                    if let egui::Event::Text(text) = event {
                        for ch in text.chars() {
                            self.editor.handle_command_char(ch);
                        }
                    }
                }
                return;
            }

            // Escape and Ctrl+[ (both exit to Normal mode)
            if input.key_pressed(egui::Key::Escape)
                || (input.modifiers.ctrl && input.key_pressed(egui::Key::OpenBracket))
            {
                let action = self.editor.key_parser.handle_escape();
                self.editor.apply_action(action);
                return;
            }

            // Ctrl+R for redo — only in Normal mode
            if self.editor.mode() == Mode::Normal
                && input.modifiers.ctrl
                && input.key_pressed(egui::Key::R)
            {
                let action = self.editor.key_parser.handle_ctrl_r();
                self.editor.apply_action(action);
                return;
            }

            // Backspace
            if input.key_pressed(egui::Key::Backspace) {
                let action = self.editor.key_parser.handle_backspace();
                self.editor.apply_action(action);
                return;
            }

            // Enter
            if input.key_pressed(egui::Key::Enter) {
                if self.editor.mode() == Mode::Insert {
                    self.editor.buffer.insert_char('\n');
                } else if self.editor.mode() == Mode::Normal {
                    // Enter in normal mode = move down one line (like j)
                    let action = self.editor.key_parser.handle_key('j');
                    self.editor.apply_action(action);
                }
                return;
            }

            // Text input
            for event in &input.events {
                if let egui::Event::Text(text) = event {
                    for ch in text.chars() {
                        let action = self.editor.key_parser.handle_key(ch);
                        self.editor.apply_action(action);
                    }
                }
            }
        });
    }
}

impl eframe::App for NyxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_input(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                self.editor_view.render(
                    ui,
                    &self.editor.buffer,
                    &self.theme,
                    self.editor.mode(),
                    14.0, // font_size — wired from config in Task 11
                    self.editor.file_path.as_deref(),
                    None, // command input — wired in Task 9
                    self.editor.status_message.as_deref(),
                );
            });
    }
}
```

- [ ] **Step 3: Update main.rs**

```rust
// src/main.rs
mod app;
mod buffer;
mod editor;
mod renderer;
mod vim;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    let file_path = std::env::args().nth(1);

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("Nyx"),
        ..Default::default()
    };
    eframe::run_native(
        "Nyx",
        options,
        Box::new(move |_cc| Ok(Box::new(app::NyxApp::new(file_path)))),
    )
}
```

- [ ] **Step 4: Verify interactively**

Run: `cargo run`
Expected:
- Welcome text visible, status bar shows "NORMAL"
- `h/j/k/l` moves cursor (block cursor)
- `i` enters insert mode (cursor becomes thin line, status bar shows "INSERT")
- Typing inserts text
- `Escape` and `Ctrl+[` both return to normal mode (block cursor, cursor moves one left)
- `a` enters insert after cursor, `A` at end of line
- `o` opens new line below, `O` above
- `5j` moves down 5 lines
- `u` undoes entire insert session (not per-character), `Ctrl+R` redoes
- `Enter` in normal mode moves down one line
- Normal mode cursor sits on last character (not past it)

- [ ] **Step 5: Commit**

```bash
git add src/editor.rs src/app.rs src/main.rs
git commit -m "feat: wire vim input to editor with count prefix and insert entries"
```

---

## Task 8: Operators (d, c, y, p)

**Files:**
- Modify: `src/vim/operator.rs`

Key fix from review: Uses `buffer.slice()` instead of `buffer.text()` for O(log n) range extraction. All mutations go through TextBuffer methods which auto-record to history, so undo works for all operators.

- [ ] **Step 1: Write failing tests for operators**

```rust
// src/vim/operator.rs
use crate::buffer::TextBuffer;
use crate::vim::action::{MotionKind, OperatorAction};
use crate::vim::motion::execute_motion;

pub struct OperatorEngine {
    pub clipboard: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delete_line() {
        let mut buf = TextBuffer::from_text("hello\nworld\nfoo");
        let mut engine = OperatorEngine::new();
        engine.execute(&mut buf, &OperatorAction::DeleteLine);
        assert_eq!(buf.text(), "world\nfoo");
        assert_eq!(engine.clipboard, "hello\n");
    }

    #[test]
    fn delete_line_then_undo() {
        let mut buf = TextBuffer::from_text("hello\nworld");
        let mut engine = OperatorEngine::new();
        engine.execute(&mut buf, &OperatorAction::DeleteLine);
        assert_eq!(buf.text(), "world");
        buf.undo();
        assert_eq!(buf.text(), "hello\nworld");
    }

    #[test]
    fn delete_word() {
        let mut buf = TextBuffer::from_text("hello world");
        let mut engine = OperatorEngine::new();
        engine.execute(&mut buf, &OperatorAction::Delete(MotionKind::WordForward));
        assert_eq!(buf.text(), "world");
    }

    #[test]
    fn yank_line() {
        let mut buf = TextBuffer::from_text("hello\nworld");
        let mut engine = OperatorEngine::new();
        engine.execute(&mut buf, &OperatorAction::YankLine);
        assert_eq!(engine.clipboard, "hello\n");
        assert_eq!(buf.text(), "hello\nworld"); // unchanged
    }

    #[test]
    fn paste_after_yank_line() {
        let mut buf = TextBuffer::from_text("hello\nworld");
        let mut engine = OperatorEngine::new();
        engine.execute(&mut buf, &OperatorAction::YankLine);
        engine.paste(&mut buf);
        assert_eq!(buf.text(), "hello\nhello\nworld");
    }

    #[test]
    fn change_line_clears_content() {
        let mut buf = TextBuffer::from_text("hello\nworld");
        let mut engine = OperatorEngine::new();
        engine.execute(&mut buf, &OperatorAction::ChangeLine);
        assert_eq!(buf.text(), "\nworld");
        assert_eq!(buf.cursor_col(), 0);
    }

    #[test]
    fn delete_word_unicode() {
        let mut buf = TextBuffer::from_text("hej världen");
        let mut engine = OperatorEngine::new();
        engine.execute(&mut buf, &OperatorAction::Delete(MotionKind::WordForward));
        assert_eq!(buf.text(), "världen");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib vim::operator`
Expected: FAIL.

- [ ] **Step 3: Implement OperatorEngine**

```rust
// src/vim/operator.rs
use crate::buffer::TextBuffer;
use crate::vim::action::{MotionKind, OperatorAction};
use crate::vim::motion::execute_motion;

pub struct OperatorEngine {
    pub clipboard: String,
}

impl OperatorEngine {
    pub fn new() -> Self {
        Self {
            clipboard: String::new(),
        }
    }

    pub fn execute(&mut self, buffer: &mut TextBuffer, action: &OperatorAction) {
        match action {
            OperatorAction::DeleteLine => {
                let line = buffer.cursor_line();
                let line_start = buffer.cursor_offset() - buffer.cursor_col();
                let line_char_len = buffer.line_len_chars(line);
                self.clipboard = buffer.slice(line_start, line_start + line_char_len);
                buffer.delete_range(line_start, line_start + line_char_len);
                buffer.set_cursor(
                    buffer.cursor_line().min(buffer.line_count().saturating_sub(1)),
                    0,
                );
            }
            OperatorAction::Delete(motion) => {
                let start = buffer.cursor_offset();
                let start_line = buffer.cursor_line();
                let start_col = buffer.cursor_col();
                execute_motion(buffer, motion);
                let end = buffer.cursor_offset();
                let (from, to) = if start < end {
                    (start, end)
                } else {
                    (end, start)
                };
                if from < to {
                    self.clipboard = buffer.slice(from, to);
                    buffer.delete_range(from, to);
                    buffer.update_cursor_from_offset(from);
                }
            }
            OperatorAction::ChangeLine => {
                let line = buffer.cursor_line();
                let line_start = buffer.cursor_offset() - buffer.cursor_col();
                let content_len = buffer.line_content_len(line);
                if content_len > 0 {
                    self.clipboard = buffer.slice(line_start, line_start + content_len);
                    buffer.delete_range(line_start, line_start + content_len);
                }
                buffer.set_cursor(line, 0);
            }
            OperatorAction::Change(motion) => {
                let start = buffer.cursor_offset();
                execute_motion(buffer, motion);
                let end = buffer.cursor_offset();
                let (from, to) = if start < end {
                    (start, end)
                } else {
                    (end, start)
                };
                if from < to {
                    self.clipboard = buffer.slice(from, to);
                    buffer.delete_range(from, to);
                    buffer.update_cursor_from_offset(from);
                }
            }
            OperatorAction::YankLine => {
                let line = buffer.cursor_line();
                let line_start = buffer.cursor_offset() - buffer.cursor_col();
                let line_char_len = buffer.line_len_chars(line);
                self.clipboard = buffer.slice(line_start, line_start + line_char_len);
            }
        }
    }

    pub fn yank_motion(&mut self, buffer: &mut TextBuffer, motion: &MotionKind) {
        let start = buffer.cursor_offset();
        let saved_line = buffer.cursor_line();
        let saved_col = buffer.cursor_col();
        execute_motion(buffer, motion);
        let end = buffer.cursor_offset();
        let (from, to) = if start < end {
            (start, end)
        } else {
            (end, start)
        };
        if from < to {
            self.clipboard = buffer.slice(from, to);
        }
        buffer.set_cursor(saved_line, saved_col);
    }

    pub fn paste(&mut self, buffer: &mut TextBuffer) {
        if self.clipboard.is_empty() {
            return;
        }
        if self.clipboard.ends_with('\n') {
            // Line paste: below current line
            let line = buffer.cursor_line();
            let line_start = buffer.cursor_offset() - buffer.cursor_col();
            let line_char_len = buffer.line_len_chars(line);
            buffer.insert_text_at(line_start + line_char_len, &self.clipboard);
            buffer.set_cursor(line + 1, 0);
        } else {
            // Inline paste: after cursor
            let offset = (buffer.cursor_offset() + 1).min(buffer.len_chars());
            buffer.insert_text_at(offset, &self.clipboard);
        }
    }
}

#[cfg(test)]
mod tests {
    // ... (tests from Step 1)
}
```

- [ ] **Step 4: Wire operators into Editor**

Update `src/editor.rs` `apply_action` to handle operator actions:

```rust
use crate::vim::operator::OperatorEngine;

pub struct Editor {
    pub buffer: TextBuffer,
    pub key_parser: KeyParser,
    pub operator_engine: OperatorEngine,
    pub file_path: Option<String>,
    pub should_quit: bool,
    pub status_message: Option<String>,
}

// In apply_action():
VimAction::Operator(ref op_action) => {
    for _ in 0..count {
        self.operator_engine.execute(&mut self.buffer, op_action);
    }
}
VimAction::Yank(ref motion) => {
    self.operator_engine.yank_motion(&mut self.buffer, motion);
}
VimAction::Paste => {
    for _ in 0..count {
        self.operator_engine.paste(&mut self.buffer);
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib vim::operator`
Expected: All tests PASS.

- [ ] **Step 6: Commit**

```bash
git add src/vim/operator.rs src/editor.rs
git commit -m "feat: vim operators (d, c, y, p) with undo support and char-safe slicing"
```

---

## Task 9: Command Mode (:w, :q)

**Files:**
- Modify: `src/vim/command.rs`
- Modify: `src/editor.rs`
- Modify: `src/app.rs`

Key fix from review: Command line input is **rendered on screen** via `command_input` parameter to EditorView.

- [ ] **Step 1: Write failing tests**

```rust
// src/vim/command.rs

#[derive(Debug, Clone, PartialEq)]
pub enum CommandResult {
    Write,
    Quit,
    WriteQuit,
    ForceQuit,
    Unknown(String),
}

pub struct CommandParser {
    pub input: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_write() {
        let mut parser = CommandParser::new();
        parser.input = "w".into();
        assert_eq!(parser.execute(), CommandResult::Write);
    }

    #[test]
    fn parse_quit() {
        let mut parser = CommandParser::new();
        parser.input = "q".into();
        assert_eq!(parser.execute(), CommandResult::Quit);
    }

    #[test]
    fn parse_write_quit() {
        let mut parser = CommandParser::new();
        parser.input = "wq".into();
        assert_eq!(parser.execute(), CommandResult::WriteQuit);
    }

    #[test]
    fn parse_force_quit() {
        let mut parser = CommandParser::new();
        parser.input = "q!".into();
        assert_eq!(parser.execute(), CommandResult::ForceQuit);
    }

    #[test]
    fn parse_unknown() {
        let mut parser = CommandParser::new();
        parser.input = "foo".into();
        assert_eq!(parser.execute(), CommandResult::Unknown("foo".into()));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib vim::command`
Expected: FAIL.

- [ ] **Step 3: Implement CommandParser**

```rust
// src/vim/command.rs

#[derive(Debug, Clone, PartialEq)]
pub enum CommandResult {
    Write,
    Quit,
    WriteQuit,
    ForceQuit,
    Unknown(String),
}

pub struct CommandParser {
    pub input: String,
}

impl CommandParser {
    pub fn new() -> Self {
        Self {
            input: String::new(),
        }
    }

    pub fn execute(&self) -> CommandResult {
        match self.input.trim() {
            "w" => CommandResult::Write,
            "q" => CommandResult::Quit,
            "wq" | "x" => CommandResult::WriteQuit,
            "q!" => CommandResult::ForceQuit,
            other => CommandResult::Unknown(other.to_string()),
        }
    }

    pub fn clear(&mut self) {
        self.input.clear();
    }

    pub fn push_char(&mut self, ch: char) {
        self.input.push(ch);
    }

    pub fn pop_char(&mut self) {
        self.input.pop();
    }
}

#[cfg(test)]
mod tests {
    // ... (tests from Step 1)
}
```

- [ ] **Step 4: Add CommandParser to Editor and handle command mode input**

Add to `src/editor.rs`:

```rust
use crate::vim::command::{CommandParser, CommandResult};

pub struct Editor {
    // ... existing fields ...
    pub command_parser: CommandParser,
}

impl Editor {
    pub fn command_input(&self) -> Option<&str> {
        if self.key_parser.mode() == Mode::Command {
            Some(&self.command_parser.input)
        } else {
            None
        }
    }

    pub fn handle_command_char(&mut self, ch: char) {
        self.command_parser.push_char(ch);
    }

    pub fn handle_command_backspace(&mut self) {
        self.command_parser.pop_char();
    }

    pub fn execute_command(&mut self) {
        let result = self.command_parser.execute();
        match result {
            CommandResult::Quit | CommandResult::ForceQuit => {
                self.should_quit = true;
            }
            CommandResult::Write => {
                self.save_file();
            }
            CommandResult::WriteQuit => {
                self.save_file();
                self.should_quit = true;
            }
            CommandResult::Unknown(cmd) => {
                self.status_message = Some(format!("Unknown command: {}", cmd));
            }
        }
        self.command_parser.clear();
        self.key_parser.handle_escape();
    }

    fn save_file(&mut self) {
        if let Some(ref path) = self.file_path {
            match crate::file_io::write_file(std::path::Path::new(path), &self.buffer.text()) {
                Ok(_) => {
                    self.status_message = Some(format!("Written: {}", path));
                    tracing::info!("File saved: {}", path);
                }
                Err(e) => {
                    self.status_message = Some(format!("Error writing {}: {}", path, e));
                    tracing::error!("Failed to save {}: {}", path, e);
                }
            }
        } else {
            self.status_message = Some("No file path".to_string());
        }
    }
}
```

- [ ] **Step 5: Verify command mode input handling in app.rs**

Command mode input handling was already added to `handle_input()` in Task 7 (it checks `Mode::Command` first, before all other input). No changes needed here — just verify it compiles.

- [ ] **Step 6: Pass command_input to renderer**

In `src/app.rs` update the render call:

```rust
self.editor_view.render(
    ui,
    &self.editor.buffer,
    &self.theme,
    self.editor.mode(),
    self.config.editor.font_size,
    self.editor.file_path.as_deref(),
    self.editor.command_input(),
    self.editor.status_message.as_deref(),
);
```

- [ ] **Step 7: Run tests**

Run: `cargo test --lib vim::command`
Expected: All tests PASS.

- [ ] **Step 8: Verify interactively**

Run: `cargo run`
Expected: `:` shows ":" at bottom of screen, typing adds characters, `Enter` executes, `:q` quits.

- [ ] **Step 9: Commit**

```bash
git add src/vim/command.rs src/editor.rs src/app.rs
git commit -m "feat: command mode with visible command line and error feedback"
```

---

## Task 10: File I/O (Atomic Writes)

**Files:**
- Create: `src/file_io/mod.rs`
- Create: `src/file_io/file.rs`
- Modify: `src/main.rs`

Key fixes from review: Atomic writes (write-to-temp + rename), proper error propagation (no silent `let _`).

- [ ] **Step 1: Write failing tests**

```rust
// src/file_io/file.rs
use std::path::Path;

pub fn read_file(path: &Path) -> Result<String, std::io::Error> {
    todo!()
}

pub fn write_file(path: &Path, content: &str) -> Result<(), std::io::Error> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn read_existing_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");
        fs::write(&path, "hello\nworld").unwrap();
        let content = read_file(&path).unwrap();
        assert_eq!(content, "hello\nworld");
    }

    #[test]
    fn read_nonexistent_file() {
        let result = read_file(Path::new("/tmp/nyx_nonexistent_test_file_12345"));
        assert!(result.is_err());
    }

    #[test]
    fn write_and_read_back() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");
        write_file(&path, "written content").unwrap();
        let content = read_file(&path).unwrap();
        assert_eq!(content, "written content");
    }

    #[test]
    fn atomic_write_does_not_corrupt_on_content_change() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");
        write_file(&path, "first").unwrap();
        write_file(&path, "second").unwrap();
        let content = read_file(&path).unwrap();
        assert_eq!(content, "second");
    }

    #[test]
    fn write_unicode_content() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");
        write_file(&path, "hej på dig åäö").unwrap();
        let content = read_file(&path).unwrap();
        assert_eq!(content, "hej på dig åäö");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib file_io`
Expected: FAIL.

- [ ] **Step 3: Implement atomic file I/O**

```rust
// src/file_io/file.rs
use std::path::Path;
use std::fs;
use std::io::Write;

pub fn read_file(path: &Path) -> Result<String, std::io::Error> {
    fs::read_to_string(path)
}

/// Atomic write: writes to a temp file in the same directory, then renames into place.
/// This ensures the original file is never left in a corrupted state.
pub fn write_file(path: &Path, content: &str) -> Result<(), std::io::Error> {
    let parent = path.parent().unwrap_or(Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(content.as_bytes())?;
    tmp.flush()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    // ... (tests from Step 1)
}
```

- [ ] **Step 4: Create file_io module**

```rust
// src/file_io/mod.rs
mod file;

pub use file::{read_file, write_file};
```

- [ ] **Step 5: Add file_io module to main.rs and update Editor**

Add `mod file_io;` to `src/main.rs`.

Update `Editor::new()` in `src/editor.rs` to use `crate::file_io::read_file`:

```rust
let buffer = if let Some(ref path) = file_path {
    match crate::file_io::read_file(std::path::Path::new(path)) {
        Ok(content) => TextBuffer::from_text(&content),
        Err(e) => {
            tracing::warn!("Could not read {}: {}", path, e);
            TextBuffer::new() // new file
        }
    }
} else {
    // welcome buffer
};
```

Note: `tempfile` is already in `[dependencies]` for atomic writes at runtime (not just dev-dependencies). Update `Cargo.toml`:

```toml
[dependencies]
# ... existing deps ...
tempfile = "3"
```

- [ ] **Step 6: Run tests**

Run: `cargo test --lib file_io`
Expected: All tests PASS.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml src/file_io/ src/main.rs src/editor.rs
git commit -m "feat: atomic file I/O with error propagation"
```

---

## Task 11: Config System

**Files:**
- Create: `src/config/mod.rs`
- Create: `src/config/schema.rs`
- Modify: `src/app.rs`
- Modify: `src/main.rs`

Key fixes from review: **Never overwrite config on parse failure.** Log warnings instead. Explicit directory permissions.

- [ ] **Step 1: Write failing tests**

```rust
// src/config/schema.rs
use serde::{Deserialize, Serialize};

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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib config`
Expected: FAIL.

- [ ] **Step 3: Implement NyxConfig**

```rust
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
    // ... (tests from Step 1)
}
```

- [ ] **Step 4: Create config module**

```rust
// src/config/mod.rs
mod schema;

pub use schema::NyxConfig;
```

- [ ] **Step 5: Add config module to main.rs and pass to NyxApp**

Add `mod config;` to `src/main.rs`.

```rust
fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    let file_path = std::env::args().nth(1);
    let config = crate::config::NyxConfig::load_or_create(
        &crate::config::NyxConfig::config_path(),
    );

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("Nyx"),
        ..Default::default()
    };
    eframe::run_native(
        "Nyx",
        options,
        Box::new(move |_cc| Ok(Box::new(app::NyxApp::new(file_path, config)))),
    )
}
```

Update `NyxApp` to store config and pass `font_size` to the renderer:

```rust
// src/app.rs — updated struct and constructor
use crate::config::NyxConfig;

pub struct NyxApp {
    editor: Editor,
    editor_view: EditorView,
    theme: Theme,
    config: NyxConfig,
}

impl NyxApp {
    pub fn new(file_path: Option<String>, config: NyxConfig) -> Self {
        Self {
            editor: Editor::new(file_path),
            editor_view: EditorView::new(),
            theme: Theme::default_dark(),
            config,
        }
    }
    // ... handle_input() unchanged
}

impl eframe::App for NyxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_input(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                self.editor_view.render(
                    ui,
                    &self.editor.buffer,
                    &self.theme,
                    self.editor.mode(),
                    self.config.editor.font_size, // from config
                    self.editor.file_path.as_deref(),
                    self.editor.command_input(),
                    self.editor.status_message.as_deref(),
                );
            });
    }
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test --lib config`
Expected: All tests PASS.

- [ ] **Step 7: Commit**

```bash
git add src/config/ src/main.rs src/app.rs
git commit -m "feat: config system with safe fallback on parse errors"
```

---

## Summary

| Task | Description | Depends On |
|------|-------------|------------|
| 1 | Project scaffolding | — |
| 2 | Rope-based TextBuffer (private cursor, Unicode, slice) | 1 |
| 3 | Auto-recording undo/redo history | 2 |
| 4 | Editor rendering (status bar, mode cursor, char-safe) | 2 |
| 5 | Vim types, mode system, keyparser (count prefix, a/A/o/O/I) | 1 |
| 6 | Motion execution (char-safe, empty buffer guards) | 2, 5 |
| 7 | Wire vim input via Editor struct | 3, 4, 5, 6 |
| 8 | Operators (d, c, y, p) with undo + slice() | 7 |
| 9 | Command mode (visible command line, error feedback) | 7 |
| 10 | Atomic file I/O | 9 |
| 11 | Config system (safe fallback) | 1 |

Tasks 2-6 can be developed in parallel after Task 1. Task 7 is the integration point. Tasks 8-10 depend on 7. Task 11 is independent after Task 1.
