# Phase 2: Core Vim Editing — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add registers, text objects, visual modes, dot-repeat, and search to make Nyx a usable vim-style editor.

**Architecture:** Bottom-up, register-first. RegisterFile is the shared foundation. Text objects plug into existing operators. Visual modes use registers + text objects. Dot-repeat records and replays actions. Search is independent.

**Tech Stack:** Rust, eframe/egui, ropey, arboard (new — cross-platform clipboard)

**Spec:** `docs/superpowers/specs/2026-05-05-phase2-core-vim-editing-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src/vim/register.rs` | Create | RegisterEntry, RegisterFile (unnamed + named + system clipboard) |
| `src/vim/text_object.rs` | Create | TextObject/TextObjectKind enums, resolve_text_object() |
| `src/vim/search.rs` | Create | SearchState, SearchDirection, match finding/navigation |
| `src/vim/action.rs` | Modify | Add TextObject operator variants, visual actions, search actions, DotRepeat |
| `src/vim/mode.rs` | Modify | Add Visual, VisualLine, VisualBlock variants |
| `src/vim/keyparser.rs` | Modify | Register prefix, text object sequences, visual/search keys, `.` |
| `src/vim/operator.rs` | Modify | Replace clipboard with RegisterFile, add text object execution |
| `src/vim/mod.rs` | Modify | Export new modules |
| `src/editor.rs` | Modify | VisualAnchor, LastAction, SearchState, apply_action expansion |
| `src/app.rs` | Modify | Ctrl+V handling, search input mode, visual mode input routing |
| `src/renderer/editor_view.rs` | Modify | Selection highlighting, search match highlighting |
| `src/renderer/theme.rs` | Modify | Add search_match, search_current colors |
| `src/renderer/status_bar.rs` | Modify | Visual mode labels, search input display, match count |
| `Cargo.toml` | Modify | Add arboard dependency |

---

### Task 1: Register System

Create the register file with unnamed, named (a-z), and system clipboard (+) registers.

**Files:**
- Create: `src/vim/register.rs`
- Modify: `src/vim/mod.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Add arboard dependency**

In `Cargo.toml`, add to `[dependencies]`:
```toml
arboard = "3"
```

Run: `cargo check`
Expected: compiles

- [ ] **Step 2: Write failing tests for RegisterFile**

Create `src/vim/register.rs`:

```rust
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct RegisterEntry {
    pub content: String,
    pub linewise: bool,
}

impl Default for RegisterEntry {
    fn default() -> Self {
        Self {
            content: String::new(),
            linewise: false,
        }
    }
}

pub struct RegisterFile {
    unnamed: RegisterEntry,
    named: HashMap<char, RegisterEntry>,
    system_clipboard: Option<arboard::Clipboard>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_unnamed_default_is_empty() {
        let reg = RegisterFile::new();
        let entry = reg.get(None);
        assert_eq!(entry.content, "");
        assert!(!entry.linewise);
    }

    #[test]
    fn set_and_get_unnamed() {
        let mut reg = RegisterFile::new();
        reg.set(None, "hello".into(), false);
        let entry = reg.get(None);
        assert_eq!(entry.content, "hello");
        assert!(!entry.linewise);
    }

    #[test]
    fn set_and_get_named() {
        let mut reg = RegisterFile::new();
        reg.set(Some('a'), "test".into(), true);
        let entry = reg.get(Some('a'));
        assert_eq!(entry.content, "test");
        assert!(entry.linewise);
    }

    #[test]
    fn named_register_also_sets_unnamed() {
        let mut reg = RegisterFile::new();
        reg.set(Some('b'), "line\n".into(), true);
        let unnamed = reg.get(None);
        assert_eq!(unnamed.content, "line\n");
        assert!(unnamed.linewise);
    }

    #[test]
    fn get_unset_named_returns_empty() {
        let reg = RegisterFile::new();
        let entry = reg.get(Some('z'));
        assert_eq!(entry.content, "");
    }

    #[test]
    fn linewise_flag_preserved() {
        let mut reg = RegisterFile::new();
        reg.set(None, "hello\n".into(), true);
        assert!(reg.get(None).linewise);
        reg.set(None, "world".into(), false);
        assert!(!reg.get(None).linewise);
    }
}
```

Run: `cargo test --lib vim::register`
Expected: FAIL — `RegisterFile::new()`, `get()`, `set()` not yet implemented

- [ ] **Step 3: Implement RegisterFile**

Add to `src/vim/register.rs` (before the `#[cfg(test)]` block):

```rust
impl RegisterFile {
    pub fn new() -> Self {
        // System clipboard: best-effort, silently unavailable on headless systems
        let system_clipboard = arboard::Clipboard::new().ok();
        Self {
            unnamed: RegisterEntry::default(),
            named: HashMap::new(),
            system_clipboard,
        }
    }

    pub fn get(&self, name: Option<char>) -> RegisterEntry {
        match name {
            None => self.unnamed.clone(),
            Some('+') => {
                // Clone self to get mutable access for clipboard read
                // arboard requires &mut self for get_text
                // We'll handle this with interior mutability if needed,
                // but for now return unnamed as fallback
                // (system clipboard read is handled via get_system_clipboard)
                self.unnamed.clone()
            }
            Some(c @ 'a'..='z') => self.named.get(&c).cloned().unwrap_or_default(),
            Some(_) => RegisterEntry::default(),
        }
    }

    /// Get from system clipboard. Needs &mut self because arboard requires it.
    pub fn get_mut(&mut self, name: Option<char>) -> RegisterEntry {
        match name {
            Some('+') => {
                if let Some(ref mut clip) = self.system_clipboard {
                    if let Ok(text) = clip.get_text() {
                        return RegisterEntry {
                            content: text,
                            linewise: false,
                        };
                    }
                }
                self.unnamed.clone()
            }
            other => self.get(other),
        }
    }

    pub fn set(&mut self, name: Option<char>, content: String, linewise: bool) {
        let entry = RegisterEntry { content, linewise };
        match name {
            None => {
                self.unnamed = entry;
            }
            Some('+') => {
                if let Some(ref mut clip) = self.system_clipboard {
                    let _ = clip.set_text(entry.content.clone());
                }
                self.unnamed = entry;
            }
            Some(c @ 'a'..='z') => {
                self.unnamed = entry.clone();
                self.named.insert(c, entry);
            }
            Some(_) => {}
        }
    }
}
```

Run: `cargo test --lib vim::register`
Expected: All 6 tests PASS

- [ ] **Step 4: Export register module**

In `src/vim/mod.rs`, add:
```rust
pub(crate) mod register;
```

Run: `cargo test`
Expected: All tests pass (existing + new)

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/vim/register.rs src/vim/mod.rs
git commit -m "feat: add register system with unnamed, named, and system clipboard"
```

---

### Task 2: Integrate Registers into OperatorEngine

Replace `OperatorEngine.clipboard: String` with `RegisterFile`. Add register parameter to all methods.

**Files:**
- Modify: `src/vim/operator.rs`
- Modify: `src/editor.rs`

- [ ] **Step 1: Write failing test for register-aware operator**

In `src/vim/operator.rs`, add test:

```rust
#[test]
fn delete_line_into_named_register() {
    let mut buf = TextBuffer::from_text("hello\nworld\nfoo");
    let mut engine = OperatorEngine::new();
    engine.execute(&mut buf, &OperatorAction::DeleteLine, Some('a'));
    assert_eq!(engine.registers.get(Some('a')).content, "hello\n");
    assert_eq!(engine.registers.get(None).content, "hello\n"); // also in unnamed
    assert_eq!(buf.text(), "world\nfoo");
}
```

Run: `cargo test --lib vim::operator::tests::delete_line_into_named_register`
Expected: FAIL — `execute` doesn't accept register parameter

- [ ] **Step 2: Refactor OperatorEngine to use RegisterFile**

Replace the `OperatorEngine` struct and all methods in `src/vim/operator.rs`:

```rust
use crate::buffer::TextBuffer;
use crate::vim::action::{MotionKind, OperatorAction};
use crate::vim::motion::execute_motion;
use crate::vim::register::RegisterFile;

#[derive(Default)]
pub struct OperatorEngine {
    pub registers: RegisterFile,
}
```

Note: `RegisterFile` needs `Default`. Add `impl Default for RegisterFile`:

In `src/vim/register.rs`, add:
```rust
impl Default for RegisterFile {
    fn default() -> Self {
        Self::new()
    }
}
```

Update `OperatorEngine::new()`:
```rust
impl OperatorEngine {
    pub fn new() -> Self {
        Self {
            registers: RegisterFile::new(),
        }
    }
```

Update `execute()` to accept `register: Option<char>`:
```rust
    pub fn execute(
        &mut self,
        buffer: &mut TextBuffer,
        action: &OperatorAction,
        register: Option<char>,
    ) {
        match action {
            OperatorAction::DeleteLine => {
                let line = buffer.cursor_line();
                let line_start = buffer.cursor_offset() - buffer.cursor_col();
                let line_char_len = buffer.line_len_chars(line);
                let content = buffer.slice(line_start, line_start + line_char_len);
                self.registers.set(register, content, true);
                buffer.delete_range(line_start, line_start + line_char_len);
                buffer.set_cursor(
                    buffer.cursor_line().min(buffer.line_count().saturating_sub(1)),
                    0,
                );
            }
            OperatorAction::Delete(motion) | OperatorAction::Change(motion) => {
                self.delete_motion(buffer, motion, register);
            }
            OperatorAction::ChangeLine => {
                let line = buffer.cursor_line();
                let line_start = buffer.cursor_offset() - buffer.cursor_col();
                let content_len = buffer.line_content_len(line);
                if content_len > 0 {
                    let content = buffer.slice(line_start, line_start + content_len);
                    self.registers.set(register, content, false);
                    buffer.delete_range(line_start, line_start + content_len);
                }
                buffer.set_cursor(line, 0);
            }
            OperatorAction::YankLine => {
                let line = buffer.cursor_line();
                let line_start = buffer.cursor_offset() - buffer.cursor_col();
                let line_char_len = buffer.line_len_chars(line);
                let content = buffer.slice(line_start, line_start + line_char_len);
                self.registers.set(register, content, true);
            }
        }
    }
```

Update `delete_motion()`:
```rust
    fn delete_motion(
        &mut self,
        buffer: &mut TextBuffer,
        motion: &MotionKind,
        register: Option<char>,
    ) {
        let start = buffer.cursor_offset();
        execute_motion(buffer, motion);
        let end = buffer.cursor_offset();
        let (from, to) = if start < end {
            (start, end)
        } else {
            (end, start)
        };
        if from < to {
            let content = buffer.slice(from, to);
            self.registers.set(register, content, false);
            buffer.delete_range(from, to);
            buffer.update_cursor_from_offset(from);
        }
    }
```

Update `yank_motion()`:
```rust
    pub fn yank_motion(
        &mut self,
        buffer: &mut TextBuffer,
        motion: &MotionKind,
        register: Option<char>,
    ) {
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
            let content = buffer.slice(from, to);
            self.registers.set(register, content, false);
        }
        buffer.set_cursor(saved_line, saved_col);
    }
```

Update `paste()`:
```rust
    pub fn paste(&mut self, buffer: &mut TextBuffer, register: Option<char>) {
        let entry = self.registers.get_mut(register);
        if entry.content.is_empty() {
            return;
        }
        if entry.linewise {
            let line = buffer.cursor_line();
            let line_start = buffer.cursor_offset() - buffer.cursor_col();
            let line_char_len = buffer.line_len_chars(line);
            buffer.insert_text_at(line_start + line_char_len, &entry.content);
            buffer.set_cursor(line + 1, 0);
        } else {
            let offset = (buffer.cursor_offset() + 1).min(buffer.len_chars());
            buffer.insert_text_at(offset, &entry.content);
        }
    }
```

- [ ] **Step 3: Update existing tests in operator.rs**

All existing tests need `None` as the register parameter. Update each test's `execute()`, `yank_motion()`, and `paste()` calls:

```rust
#[test]
fn delete_line() {
    let mut buf = TextBuffer::from_text("hello\nworld\nfoo");
    let mut engine = OperatorEngine::new();
    engine.execute(&mut buf, &OperatorAction::DeleteLine, None);
    assert_eq!(buf.text(), "world\nfoo");
    assert_eq!(engine.registers.get(None).content, "hello\n");
}

#[test]
fn delete_line_then_undo() {
    let mut buf = TextBuffer::from_text("hello\nworld");
    let mut engine = OperatorEngine::new();
    engine.execute(&mut buf, &OperatorAction::DeleteLine, None);
    assert_eq!(buf.text(), "world");
    buf.undo();
    assert_eq!(buf.text(), "hello\nworld");
}

#[test]
fn delete_word() {
    let mut buf = TextBuffer::from_text("hello world");
    let mut engine = OperatorEngine::new();
    engine.execute(&mut buf, &OperatorAction::Delete(MotionKind::WordForward), None);
    assert_eq!(buf.text(), "world");
}

#[test]
fn yank_line() {
    let mut buf = TextBuffer::from_text("hello\nworld");
    let mut engine = OperatorEngine::new();
    engine.execute(&mut buf, &OperatorAction::YankLine, None);
    assert_eq!(engine.registers.get(None).content, "hello\n");
    assert_eq!(buf.text(), "hello\nworld");
}

#[test]
fn paste_after_yank_line() {
    let mut buf = TextBuffer::from_text("hello\nworld");
    let mut engine = OperatorEngine::new();
    engine.execute(&mut buf, &OperatorAction::YankLine, None);
    engine.paste(&mut buf, None);
    assert_eq!(buf.text(), "hello\nhello\nworld");
}

#[test]
fn change_line_clears_content() {
    let mut buf = TextBuffer::from_text("hello\nworld");
    let mut engine = OperatorEngine::new();
    engine.execute(&mut buf, &OperatorAction::ChangeLine, None);
    assert_eq!(buf.text(), "\nworld");
    assert_eq!(buf.cursor_col(), 0);
}

#[test]
fn delete_word_unicode() {
    let mut buf = TextBuffer::from_text("hej världen");
    let mut engine = OperatorEngine::new();
    engine.execute(&mut buf, &OperatorAction::Delete(MotionKind::WordForward), None);
    assert_eq!(buf.text(), "världen");
}

#[test]
fn change_word() {
    let mut buf = TextBuffer::from_text("hello world");
    let mut engine = OperatorEngine::new();
    engine.execute(&mut buf, &OperatorAction::Change(MotionKind::WordForward), None);
    assert_eq!(buf.text(), "world");
    assert_eq!(engine.registers.get(None).content, "hello ");
}

#[test]
fn yank_motion_preserves_cursor() {
    let mut buf = TextBuffer::from_text("hello world");
    buf.set_cursor(0, 0);
    let mut engine = OperatorEngine::new();
    engine.yank_motion(&mut buf, &MotionKind::WordForward, None);
    assert_eq!(engine.registers.get(None).content, "hello ");
    assert_eq!(buf.cursor_col(), 0);
}

#[test]
fn paste_inline() {
    let mut buf = TextBuffer::from_text("hello world");
    let mut engine = OperatorEngine::new();
    engine.execute(&mut buf, &OperatorAction::Delete(MotionKind::WordForward), None);
    assert_eq!(buf.text(), "world");
    engine.paste(&mut buf, None);
    assert_eq!(buf.text(), "whello orld");
}

#[test]
fn delete_line_into_named_register() {
    let mut buf = TextBuffer::from_text("hello\nworld\nfoo");
    let mut engine = OperatorEngine::new();
    engine.execute(&mut buf, &OperatorAction::DeleteLine, Some('a'));
    assert_eq!(engine.registers.get(Some('a')).content, "hello\n");
    assert_eq!(engine.registers.get(None).content, "hello\n");
    assert_eq!(buf.text(), "world\nfoo");
}
```

- [ ] **Step 4: Update Editor.apply_action() to pass register**

In `src/editor.rs`, update `apply_action()`. Add `let register = self.key_parser.take_register();` after the `take_count()` call, and pass `register` to operator engine methods:

```rust
pub fn apply_action(&mut self, action: VimAction) {
    if action == VimAction::Noop {
        return;
    }
    self.status_message = None;

    let count = self.key_parser.take_count();
    let register = self.key_parser.take_register();
    match action {
        // ... existing arms unchanged for SwitchMode, Motion, InsertChar, DeleteCharBefore, EnterInsert, Undo, Redo ...
        VimAction::Operator(ref op_action) => {
            for _ in 0..count {
                self.operator_engine.execute(&mut self.buffer, op_action, register);
            }
        }
        VimAction::Yank(ref motion) => {
            self.operator_engine
                .yank_motion(&mut self.buffer, motion, register);
        }
        VimAction::Paste => {
            for _ in 0..count {
                self.operator_engine.paste(&mut self.buffer, register);
            }
        }
        // ... rest unchanged ...
    }
}
```

This requires `take_register()` on KeyParser. Add a stub to `src/vim/keyparser.rs`:
```rust
pub fn take_register(&mut self) -> Option<char> {
    None // Will be implemented in next steps
}
```

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add src/vim/register.rs src/vim/operator.rs src/vim/mod.rs src/editor.rs src/vim/keyparser.rs
git commit -m "feat: integrate register system into OperatorEngine"
```

---

### Task 3: Register Prefix in KeyParser

Add `"` prefix parsing to KeyParser so users can target specific registers.

**Files:**
- Modify: `src/vim/keyparser.rs`

- [ ] **Step 1: Write failing tests**

Add to `src/vim/keyparser.rs` tests:

```rust
#[test]
fn register_prefix_sets_register() {
    let mut parser = KeyParser::new();
    assert_eq!(parser.handle_key('"'), VimAction::Noop);
    assert_eq!(parser.handle_key('a'), VimAction::Noop);
    // Register is now set to 'a', consumed by take_register
    assert_eq!(parser.take_register(), Some('a'));
}

#[test]
fn register_prefix_plus() {
    let mut parser = KeyParser::new();
    parser.handle_key('"');
    parser.handle_key('+');
    assert_eq!(parser.take_register(), Some('+'));
}

#[test]
fn register_consumed_after_take() {
    let mut parser = KeyParser::new();
    parser.handle_key('"');
    parser.handle_key('a');
    assert_eq!(parser.take_register(), Some('a'));
    assert_eq!(parser.take_register(), None);
}

#[test]
fn register_prefix_then_operator() {
    let mut parser = KeyParser::new();
    parser.handle_key('"');
    parser.handle_key('a');
    let action = parser.handle_key('p');
    assert_eq!(action, VimAction::Paste);
    assert_eq!(parser.take_register(), Some('a'));
}
```

Run: `cargo test --lib vim::keyparser::tests::register_prefix_sets_register`
Expected: FAIL

- [ ] **Step 2: Implement register prefix parsing**

Add fields to `KeyParser`:
```rust
pub struct KeyParser {
    mode: Mode,
    pending: String,
    count: Option<usize>,
    pending_register: Option<char>,
    awaiting_register: bool,
}
```

Update `new()`:
```rust
pub fn new() -> Self {
    Self {
        mode: Mode::Normal,
        pending: String::new(),
        count: None,
        pending_register: None,
        awaiting_register: false,
    }
}
```

Add `take_register()`:
```rust
pub fn take_register(&mut self) -> Option<char> {
    self.pending_register.take()
}
```

Update `handle_escape()` to clear register state:
```rust
pub fn handle_escape(&mut self) -> VimAction {
    self.pending.clear();
    self.count = None;
    self.pending_register = None;
    self.awaiting_register = false;
    self.mode = Mode::Normal;
    VimAction::SwitchMode(Mode::Normal)
}
```

In `handle_normal()`, add register prefix handling at the top (before count and pending checks):
```rust
fn handle_normal(&mut self, ch: char) -> VimAction {
    // Register prefix: " followed by a-z or +
    if self.awaiting_register {
        self.awaiting_register = false;
        match ch {
            'a'..='z' | '+' => {
                self.pending_register = Some(ch);
                return VimAction::Noop;
            }
            _ => {
                // Invalid register name, ignore
                return VimAction::Noop;
            }
        }
    }

    // Count prefix (existing code) ...
    // ...

    // In the single-char match block, add '"' case:
    // Before the `_ => VimAction::Noop` default:
    '"' => {
        self.awaiting_register = true;
        VimAction::Noop
    }
```

Run: `cargo test --lib vim::keyparser`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add src/vim/keyparser.rs
git commit -m "feat: add register prefix parsing to KeyParser"
```

---

### Task 4: Text Object Types + Word/BigWord Resolution

Create the text object module with enums and word/WORD resolution.

**Files:**
- Create: `src/vim/text_object.rs`
- Modify: `src/vim/mod.rs`

- [ ] **Step 1: Write failing tests for word text objects**

Create `src/vim/text_object.rs`:

```rust
use crate::buffer::TextBuffer;

#[derive(Debug, Clone, PartialEq)]
pub enum TextObject {
    Inner(TextObjectKind),
    Around(TextObjectKind),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TextObjectKind {
    Word,
    BigWord,
    DoubleQuote,
    SingleQuote,
    Paren,
    Bracket,
    Brace,
}

/// Returns char range [start, end) for the text object at cursor, or None.
pub fn resolve_text_object(buffer: &TextBuffer, obj: &TextObject) -> Option<(usize, usize)> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Word ---

    #[test]
    fn inner_word_middle() {
        let buf = TextBuffer::from_text("hello world foo");
        // cursor on 'w' (col 6)
        let mut buf = buf;
        buf.set_cursor(0, 6);
        let range = resolve_text_object(&buf, &TextObject::Inner(TextObjectKind::Word));
        assert_eq!(range, Some((6, 11))); // "world"
    }

    #[test]
    fn inner_word_start_of_line() {
        let mut buf = TextBuffer::from_text("hello world");
        buf.set_cursor(0, 0);
        let range = resolve_text_object(&buf, &TextObject::Inner(TextObjectKind::Word));
        assert_eq!(range, Some((0, 5))); // "hello"
    }

    #[test]
    fn around_word_includes_trailing_space() {
        let mut buf = TextBuffer::from_text("hello world foo");
        buf.set_cursor(0, 6);
        let range = resolve_text_object(&buf, &TextObject::Around(TextObjectKind::Word));
        assert_eq!(range, Some((6, 12))); // "world "
    }

    #[test]
    fn around_word_at_end_includes_leading_space() {
        let mut buf = TextBuffer::from_text("hello world");
        buf.set_cursor(0, 6);
        let range = resolve_text_object(&buf, &TextObject::Around(TextObjectKind::Word));
        assert_eq!(range, Some((5, 11))); // " world"
    }

    #[test]
    fn inner_word_punctuation() {
        // In vim, punctuation is its own word class
        let mut buf = TextBuffer::from_text("foo.bar");
        buf.set_cursor(0, 3); // on '.'
        let range = resolve_text_object(&buf, &TextObject::Inner(TextObjectKind::Word));
        assert_eq!(range, Some((3, 4))); // just "."
    }

    #[test]
    fn inner_big_word() {
        let mut buf = TextBuffer::from_text("hello foo.bar world");
        buf.set_cursor(0, 6); // on 'f' of "foo.bar"
        let range = resolve_text_object(&buf, &TextObject::Inner(TextObjectKind::BigWord));
        assert_eq!(range, Some((6, 13))); // "foo.bar"
    }

    #[test]
    fn around_big_word() {
        let mut buf = TextBuffer::from_text("hello foo.bar world");
        buf.set_cursor(0, 6);
        let range = resolve_text_object(&buf, &TextObject::Around(TextObjectKind::BigWord));
        assert_eq!(range, Some((6, 14))); // "foo.bar "
    }

    #[test]
    fn inner_word_unicode() {
        let mut buf = TextBuffer::from_text("hej världen");
        buf.set_cursor(0, 4); // on 'v' of "världen"
        let range = resolve_text_object(&buf, &TextObject::Inner(TextObjectKind::Word));
        assert_eq!(range, Some((4, 11))); // "världen"
    }
}
```

Run: `cargo test --lib vim::text_object`
Expected: FAIL — `resolve_text_object` is `todo!()`

- [ ] **Step 2: Implement word/WORD text object resolution**

Replace the `todo!()` in `resolve_text_object`:

```rust
pub fn resolve_text_object(buffer: &TextBuffer, obj: &TextObject) -> Option<(usize, usize)> {
    let offset = buffer.cursor_offset();
    let text = buffer.text();
    let chars: Vec<char> = text.chars().collect();

    if chars.is_empty() || offset >= chars.len() {
        return None;
    }

    match obj {
        TextObject::Inner(kind) => resolve_inner(buffer, &chars, offset, kind),
        TextObject::Around(kind) => resolve_around(buffer, &chars, offset, kind),
    }
}

fn char_class(ch: char) -> u8 {
    if ch.is_alphanumeric() || ch == '_' {
        0 // word char
    } else if ch.is_whitespace() {
        2 // whitespace
    } else {
        1 // punctuation
    }
}

fn resolve_inner(
    buffer: &TextBuffer,
    chars: &[char],
    offset: usize,
    kind: &TextObjectKind,
) -> Option<(usize, usize)> {
    match kind {
        TextObjectKind::Word => {
            let class = char_class(chars[offset]);
            let mut start = offset;
            while start > 0 && char_class(chars[start - 1]) == class {
                start -= 1;
            }
            let mut end = offset;
            while end < chars.len() && char_class(chars[end]) == class {
                end += 1;
            }
            Some((start, end))
        }
        TextObjectKind::BigWord => {
            if chars[offset].is_whitespace() {
                // On whitespace: select the whitespace run
                let mut start = offset;
                while start > 0 && chars[start - 1].is_whitespace() {
                    start -= 1;
                }
                let mut end = offset;
                while end < chars.len() && chars[end].is_whitespace() {
                    end += 1;
                }
                Some((start, end))
            } else {
                let mut start = offset;
                while start > 0 && !chars[start - 1].is_whitespace() {
                    start -= 1;
                }
                let mut end = offset;
                while end < chars.len() && !chars[end].is_whitespace() {
                    end += 1;
                }
                Some((start, end))
            }
        }
        TextObjectKind::DoubleQuote => resolve_inner_quote(chars, offset, '"'),
        TextObjectKind::SingleQuote => resolve_inner_quote(chars, offset, '\''),
        TextObjectKind::Paren => resolve_inner_bracket(chars, offset, '(', ')'),
        TextObjectKind::Bracket => resolve_inner_bracket(chars, offset, '[', ']'),
        TextObjectKind::Brace => resolve_inner_bracket(chars, offset, '{', '}'),
    }
}

fn resolve_around(
    buffer: &TextBuffer,
    chars: &[char],
    offset: usize,
    kind: &TextObjectKind,
) -> Option<(usize, usize)> {
    match kind {
        TextObjectKind::Word => {
            let class = char_class(chars[offset]);
            let mut start = offset;
            while start > 0 && char_class(chars[start - 1]) == class {
                start -= 1;
            }
            let mut end = offset;
            while end < chars.len() && char_class(chars[end]) == class {
                end += 1;
            }
            // Include trailing whitespace, or leading if no trailing
            let orig_end = end;
            while end < chars.len() && chars[end].is_whitespace() && chars[end] != '\n' {
                end += 1;
            }
            if end == orig_end {
                // No trailing whitespace, try leading
                while start > 0 && chars[start - 1].is_whitespace() && chars[start - 1] != '\n' {
                    start -= 1;
                }
            }
            Some((start, end))
        }
        TextObjectKind::BigWord => {
            if chars[offset].is_whitespace() {
                return resolve_inner(buffer, chars, offset, kind);
            }
            let mut start = offset;
            while start > 0 && !chars[start - 1].is_whitespace() {
                start -= 1;
            }
            let mut end = offset;
            while end < chars.len() && !chars[end].is_whitespace() {
                end += 1;
            }
            let orig_end = end;
            while end < chars.len() && chars[end].is_whitespace() && chars[end] != '\n' {
                end += 1;
            }
            if end == orig_end {
                while start > 0 && chars[start - 1].is_whitespace() && chars[start - 1] != '\n' {
                    start -= 1;
                }
            }
            Some((start, end))
        }
        TextObjectKind::DoubleQuote => resolve_around_quote(chars, offset, '"'),
        TextObjectKind::SingleQuote => resolve_around_quote(chars, offset, '\''),
        TextObjectKind::Paren => resolve_around_bracket(chars, offset, '(', ')'),
        TextObjectKind::Bracket => resolve_around_bracket(chars, offset, '[', ']'),
        TextObjectKind::Brace => resolve_around_bracket(chars, offset, '{', '}'),
    }
}
```

Add stub functions for quotes and brackets (to be implemented in Tasks 5 and 6):

```rust
fn resolve_inner_quote(chars: &[char], offset: usize, quote: char) -> Option<(usize, usize)> {
    None // Implemented in Task 5
}

fn resolve_around_quote(chars: &[char], offset: usize, quote: char) -> Option<(usize, usize)> {
    None // Implemented in Task 5
}

fn resolve_inner_bracket(
    chars: &[char],
    offset: usize,
    open: char,
    close: char,
) -> Option<(usize, usize)> {
    None // Implemented in Task 6
}

fn resolve_around_bracket(
    chars: &[char],
    offset: usize,
    open: char,
    close: char,
) -> Option<(usize, usize)> {
    None // Implemented in Task 6
}
```

Run: `cargo test --lib vim::text_object`
Expected: Word/BigWord tests PASS

- [ ] **Step 3: Export text_object module**

In `src/vim/mod.rs`, add:
```rust
pub(crate) mod text_object;
```

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add src/vim/text_object.rs src/vim/mod.rs
git commit -m "feat: add text object types with word/WORD resolution"
```

---

### Task 5: Text Object Quote Resolution

Implement `i"`, `a"`, `i'`, `a'` text objects.

**Files:**
- Modify: `src/vim/text_object.rs`

- [ ] **Step 1: Write failing tests for quote text objects**

Add to tests in `src/vim/text_object.rs`:

```rust
// --- Quotes ---

#[test]
fn inner_double_quote() {
    let mut buf = TextBuffer::from_text("say \"hello world\" end");
    buf.set_cursor(0, 7); // on 'l' inside quotes
    let range = resolve_text_object(&buf, &TextObject::Inner(TextObjectKind::DoubleQuote));
    assert_eq!(range, Some((5, 16))); // "hello world"
}

#[test]
fn around_double_quote() {
    let mut buf = TextBuffer::from_text("say \"hello world\" end");
    buf.set_cursor(0, 7);
    let range = resolve_text_object(&buf, &TextObject::Around(TextObjectKind::DoubleQuote));
    assert_eq!(range, Some((4, 17))); // "\"hello world\""
}

#[test]
fn inner_single_quote() {
    let mut buf = TextBuffer::from_text("it's 'fine' now");
    buf.set_cursor(0, 7); // on 'i' inside quotes
    let range = resolve_text_object(&buf, &TextObject::Inner(TextObjectKind::SingleQuote));
    assert_eq!(range, Some((6, 10))); // "fine"
}

#[test]
fn around_single_quote() {
    let mut buf = TextBuffer::from_text("it's 'fine' now");
    buf.set_cursor(0, 7);
    let range = resolve_text_object(&buf, &TextObject::Around(TextObjectKind::SingleQuote));
    assert_eq!(range, Some((5, 11))); // "'fine'"
}

#[test]
fn quote_cursor_on_opening_quote() {
    let mut buf = TextBuffer::from_text("say \"hi\" end");
    buf.set_cursor(0, 4); // on opening "
    let range = resolve_text_object(&buf, &TextObject::Inner(TextObjectKind::DoubleQuote));
    assert_eq!(range, Some((5, 7))); // "hi"
}

#[test]
fn quote_no_match_returns_none() {
    let mut buf = TextBuffer::from_text("no quotes here");
    buf.set_cursor(0, 3);
    let range = resolve_text_object(&buf, &TextObject::Inner(TextObjectKind::DoubleQuote));
    assert_eq!(range, None);
}

#[test]
fn quote_empty_inside() {
    let mut buf = TextBuffer::from_text("x = \"\"");
    buf.set_cursor(0, 4); // on first "
    let range = resolve_text_object(&buf, &TextObject::Inner(TextObjectKind::DoubleQuote));
    assert_eq!(range, Some((5, 5))); // empty range
}
```

Run: `cargo test --lib vim::text_object::tests::inner_double_quote`
Expected: FAIL — returns None

- [ ] **Step 2: Implement quote resolution**

Replace the stub functions in `src/vim/text_object.rs`:

```rust
fn resolve_inner_quote(chars: &[char], offset: usize, quote: char) -> Option<(usize, usize)> {
    // Find the line boundaries (quotes are line-local)
    let line_start = chars[..offset]
        .iter()
        .rposition(|&c| c == '\n')
        .map(|p| p + 1)
        .unwrap_or(0);
    let line_end = chars[offset..]
        .iter()
        .position(|&c| c == '\n')
        .map(|p| offset + p)
        .unwrap_or(chars.len());

    let line_chars = &chars[line_start..line_end];
    let cursor_in_line = offset - line_start;

    // Find all quote positions on this line
    let quote_positions: Vec<usize> = line_chars
        .iter()
        .enumerate()
        .filter(|(_, &c)| c == quote)
        .map(|(i, _)| i)
        .collect();

    // Need at least 2 quotes to form a pair
    if quote_positions.len() < 2 {
        return None;
    }

    // Find the pair that contains the cursor
    for pair in quote_positions.chunks(2) {
        if pair.len() < 2 {
            break;
        }
        let open = pair[0];
        let close = pair[1];
        if cursor_in_line >= open && cursor_in_line <= close {
            return Some((line_start + open + 1, line_start + close));
        }
    }

    None
}

fn resolve_around_quote(chars: &[char], offset: usize, quote: char) -> Option<(usize, usize)> {
    let inner = resolve_inner_quote(chars, offset, quote)?;
    // Around includes the quotes themselves (one char before inner start, one after inner end)
    Some((inner.0 - 1, inner.1 + 1))
}
```

Run: `cargo test --lib vim::text_object`
Expected: All quote tests PASS

- [ ] **Step 3: Commit**

```bash
git add src/vim/text_object.rs
git commit -m "feat: add quote text object resolution (i\"/a\", i'/a')"
```

---

### Task 6: Text Object Bracket Resolution

Implement `i(`, `a(`, `i[`, `a[`, `i{`, `a{` with nesting support.

**Files:**
- Modify: `src/vim/text_object.rs`

- [ ] **Step 1: Write failing tests**

Add to tests in `src/vim/text_object.rs`:

```rust
// --- Brackets ---

#[test]
fn inner_paren() {
    let mut buf = TextBuffer::from_text("call(x, y)");
    buf.set_cursor(0, 6); // on ','
    let range = resolve_text_object(&buf, &TextObject::Inner(TextObjectKind::Paren));
    assert_eq!(range, Some((5, 9))); // "x, y"
}

#[test]
fn around_paren() {
    let mut buf = TextBuffer::from_text("call(x, y)");
    buf.set_cursor(0, 6);
    let range = resolve_text_object(&buf, &TextObject::Around(TextObjectKind::Paren));
    assert_eq!(range, Some((4, 10))); // "(x, y)"
}

#[test]
fn inner_bracket() {
    let mut buf = TextBuffer::from_text("arr[1, 2]");
    buf.set_cursor(0, 5);
    let range = resolve_text_object(&buf, &TextObject::Inner(TextObjectKind::Bracket));
    assert_eq!(range, Some((4, 8))); // "1, 2"
}

#[test]
fn inner_brace() {
    let mut buf = TextBuffer::from_text("fn() { body }");
    buf.set_cursor(0, 8); // on 'o' of body
    let range = resolve_text_object(&buf, &TextObject::Inner(TextObjectKind::Brace));
    assert_eq!(range, Some((6, 12))); // " body "
}

#[test]
fn nested_parens() {
    let mut buf = TextBuffer::from_text("a(b(c)d)e");
    buf.set_cursor(0, 4); // on 'c' inside inner parens
    let range = resolve_text_object(&buf, &TextObject::Inner(TextObjectKind::Paren));
    assert_eq!(range, Some((4, 5))); // "c" (innermost)
}

#[test]
fn nested_parens_outer() {
    let mut buf = TextBuffer::from_text("a(b(c)d)e");
    buf.set_cursor(0, 2); // on 'b' — between outer parens but outside inner
    let range = resolve_text_object(&buf, &TextObject::Inner(TextObjectKind::Paren));
    assert_eq!(range, Some((2, 7))); // "b(c)d"
}

#[test]
fn bracket_multiline() {
    let mut buf = TextBuffer::from_text("{\n  hello\n}");
    buf.set_cursor(1, 2); // on 'h' of hello (line 1, col 2)
    let range = resolve_text_object(&buf, &TextObject::Inner(TextObjectKind::Brace));
    assert_eq!(range, Some((1, 10))); // "\n  hello\n"
}

#[test]
fn bracket_no_match() {
    let mut buf = TextBuffer::from_text("no brackets");
    buf.set_cursor(0, 3);
    let range = resolve_text_object(&buf, &TextObject::Inner(TextObjectKind::Paren));
    assert_eq!(range, None);
}

#[test]
fn cursor_on_opening_bracket() {
    let mut buf = TextBuffer::from_text("(hello)");
    buf.set_cursor(0, 0); // on '('
    let range = resolve_text_object(&buf, &TextObject::Inner(TextObjectKind::Paren));
    assert_eq!(range, Some((1, 6))); // "hello"
}

#[test]
fn around_brace() {
    let mut buf = TextBuffer::from_text("fn() { body }");
    buf.set_cursor(0, 8);
    let range = resolve_text_object(&buf, &TextObject::Around(TextObjectKind::Brace));
    assert_eq!(range, Some((5, 13))); // "{ body }"
}
```

Run: `cargo test --lib vim::text_object::tests::inner_paren`
Expected: FAIL — returns None

- [ ] **Step 2: Implement bracket resolution**

Replace the bracket stub functions:

```rust
fn resolve_inner_bracket(
    chars: &[char],
    offset: usize,
    open: char,
    close: char,
) -> Option<(usize, usize)> {
    // Find the opening bracket: scan left from cursor
    let mut depth = 0i32;
    let mut open_pos = None;

    // If cursor is on the open bracket, use it
    if chars[offset] == open {
        open_pos = Some(offset);
    } else if chars[offset] == close {
        // Cursor on close bracket: find matching open
        depth = 1;
        for i in (0..offset).rev() {
            if chars[i] == close {
                depth += 1;
            } else if chars[i] == open {
                depth -= 1;
                if depth == 0 {
                    open_pos = Some(i);
                    break;
                }
            }
        }
    } else {
        // Scan left for the enclosing open bracket
        depth = 0;
        for i in (0..=offset).rev() {
            if chars[i] == close && i != offset {
                depth += 1;
            } else if chars[i] == open {
                if depth == 0 {
                    open_pos = Some(i);
                    break;
                }
                depth -= 1;
            }
        }
    }

    let open_pos = open_pos?;

    // Find matching close bracket
    depth = 1;
    for i in (open_pos + 1)..chars.len() {
        if chars[i] == open {
            depth += 1;
        } else if chars[i] == close {
            depth -= 1;
            if depth == 0 {
                return Some((open_pos + 1, i));
            }
        }
    }

    None
}

fn resolve_around_bracket(
    chars: &[char],
    offset: usize,
    open: char,
    close: char,
) -> Option<(usize, usize)> {
    let inner = resolve_inner_bracket(chars, offset, open, close)?;
    // Around includes the brackets themselves
    Some((inner.0 - 1, inner.1 + 1))
}
```

Run: `cargo test --lib vim::text_object`
Expected: All bracket tests PASS

- [ ] **Step 3: Commit**

```bash
git add src/vim/text_object.rs
git commit -m "feat: add bracket/paren/brace text object resolution with nesting"
```

---

### Task 7: Text Object Integration

Wire text objects into VimAction, KeyParser, and OperatorEngine.

**Files:**
- Modify: `src/vim/action.rs`
- Modify: `src/vim/keyparser.rs`
- Modify: `src/vim/operator.rs`
- Modify: `src/editor.rs`

- [ ] **Step 1: Add text object variants to OperatorAction**

In `src/vim/action.rs`, add the import and new variants:

```rust
use crate::vim::text_object::TextObject;
```

Add to `OperatorAction`:
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum OperatorAction {
    Delete(MotionKind),
    Change(MotionKind),
    DeleteLine,
    ChangeLine,
    YankLine,
    DeleteTextObject(TextObject),
    ChangeTextObject(TextObject),
    YankTextObject(TextObject),
}
```

Run: `cargo check`
Expected: Compile errors in operator.rs (non-exhaustive match) — expected, will fix in step 3

- [ ] **Step 2: Write failing tests for text object operators**

Add to `src/vim/operator.rs` tests:

```rust
use crate::vim::text_object::{TextObject, TextObjectKind};

#[test]
fn delete_inner_word() {
    let mut buf = TextBuffer::from_text("hello world foo");
    buf.set_cursor(0, 6); // on 'w'
    let mut engine = OperatorEngine::new();
    engine.execute(
        &mut buf,
        &OperatorAction::DeleteTextObject(TextObject::Inner(TextObjectKind::Word)),
        None,
    );
    assert_eq!(buf.text(), "hello  foo");
    assert_eq!(engine.registers.get(None).content, "world");
}

#[test]
fn change_inner_quotes() {
    let mut buf = TextBuffer::from_text("say \"hello\" end");
    buf.set_cursor(0, 6); // inside quotes
    let mut engine = OperatorEngine::new();
    engine.execute(
        &mut buf,
        &OperatorAction::ChangeTextObject(TextObject::Inner(TextObjectKind::DoubleQuote)),
        None,
    );
    assert_eq!(buf.text(), "say \"\" end");
    assert_eq!(engine.registers.get(None).content, "hello");
}

#[test]
fn yank_around_parens() {
    let mut buf = TextBuffer::from_text("call(x, y) end");
    buf.set_cursor(0, 6);
    let mut engine = OperatorEngine::new();
    engine.execute(
        &mut buf,
        &OperatorAction::YankTextObject(TextObject::Around(TextObjectKind::Paren)),
        None,
    );
    assert_eq!(buf.text(), "call(x, y) end"); // unchanged
    assert_eq!(engine.registers.get(None).content, "(x, y)");
}
```

- [ ] **Step 3: Implement text object execution in OperatorEngine**

In `src/vim/operator.rs`, add import:
```rust
use crate::vim::text_object::{resolve_text_object, TextObject};
```

Add arms to the `execute()` match:

```rust
OperatorAction::DeleteTextObject(ref obj) => {
    if let Some((start, end)) = resolve_text_object(buffer, obj) {
        let content = buffer.slice(start, end);
        self.registers.set(register, content, false);
        buffer.delete_range(start, end);
        buffer.update_cursor_from_offset(start);
    }
}
OperatorAction::ChangeTextObject(ref obj) => {
    if let Some((start, end)) = resolve_text_object(buffer, obj) {
        let content = buffer.slice(start, end);
        self.registers.set(register, content, false);
        buffer.delete_range(start, end);
        buffer.update_cursor_from_offset(start);
    }
}
OperatorAction::YankTextObject(ref obj) => {
    if let Some((start, end)) = resolve_text_object(buffer, obj) {
        let content = buffer.slice(start, end);
        self.registers.set(register, content, false);
    }
}
```

Run: `cargo test --lib vim::operator`
Expected: All tests PASS

- [ ] **Step 4: Add text object parsing to KeyParser**

In `src/vim/keyparser.rs`, the pending buffer handling needs to support 3-char sequences. Currently `handle_normal` resolves pending after 2 chars. We need: if combined is `"di"`, `"da"`, `"ci"`, `"ca"`, `"yi"`, `"ya"`, keep pending and wait for one more char.

Update the pending block in `handle_normal()`:

```rust
if !self.pending.is_empty() {
    let combined = format!("{}{}", self.pending, ch);

    // 3-char text object sequences: operator + i/a + object
    if combined.len() == 2 {
        let first = combined.chars().next().unwrap();
        if (first == 'd' || first == 'c' || first == 'y') && (ch == 'i' || ch == 'a') {
            // Keep pending for one more character (the text object kind)
            self.pending = combined;
            return VimAction::Noop;
        }
    }

    if combined.len() == 3 {
        // Text object: e.g. "diw", "ci\"", "ya("
        let mut chars = combined.chars();
        let op = chars.next().unwrap();
        let scope = chars.next().unwrap();
        let kind_ch = chars.next().unwrap();

        self.pending.clear();

        if let Some(kind) = Self::char_to_text_object_kind(kind_ch) {
            let text_obj = match scope {
                'i' => TextObject::Inner(kind),
                'a' => TextObject::Around(kind),
                _ => return VimAction::Noop,
            };
            return match op {
                'd' => VimAction::Operator(OperatorAction::DeleteTextObject(text_obj)),
                'c' => {
                    self.mode = Mode::Insert;
                    VimAction::Operator(OperatorAction::ChangeTextObject(text_obj))
                }
                'y' => VimAction::Operator(OperatorAction::YankTextObject(text_obj)),
                _ => VimAction::Noop,
            };
        }
        return VimAction::Noop;
    }

    self.pending.clear();
    // ... existing 2-char match logic (gg, dd, cc, yy, d+motion, c+motion, y+motion) ...
```

Add the helper:
```rust
fn char_to_text_object_kind(ch: char) -> Option<TextObjectKind> {
    match ch {
        'w' => Some(TextObjectKind::Word),
        'W' => Some(TextObjectKind::BigWord),
        '"' => Some(TextObjectKind::DoubleQuote),
        '\'' => Some(TextObjectKind::SingleQuote),
        '(' | ')' => Some(TextObjectKind::Paren),
        '[' | ']' => Some(TextObjectKind::Bracket),
        '{' | '}' => Some(TextObjectKind::Brace),
        _ => None,
    }
}
```

Add import at top:
```rust
use crate::vim::text_object::{TextObject, TextObjectKind};
```

- [ ] **Step 5: Write and run KeyParser text object tests**

Add to keyparser tests:

```rust
#[test]
fn text_object_diw() {
    let mut parser = KeyParser::new();
    assert_eq!(parser.handle_key('d'), VimAction::Noop);
    assert_eq!(parser.handle_key('i'), VimAction::Noop);
    assert_eq!(
        parser.handle_key('w'),
        VimAction::Operator(OperatorAction::DeleteTextObject(
            TextObject::Inner(TextObjectKind::Word)
        ))
    );
}

#[test]
fn text_object_ci_double_quote() {
    let mut parser = KeyParser::new();
    parser.handle_key('c');
    parser.handle_key('i');
    let action = parser.handle_key('"');
    assert_eq!(
        action,
        VimAction::Operator(OperatorAction::ChangeTextObject(
            TextObject::Inner(TextObjectKind::DoubleQuote)
        ))
    );
    assert_eq!(parser.mode(), Mode::Insert);
}

#[test]
fn text_object_ya_paren() {
    let mut parser = KeyParser::new();
    parser.handle_key('y');
    parser.handle_key('a');
    let action = parser.handle_key('(');
    assert_eq!(
        action,
        VimAction::Operator(OperatorAction::YankTextObject(
            TextObject::Around(TextObjectKind::Paren)
        ))
    );
}

#[test]
fn text_object_invalid_resets() {
    let mut parser = KeyParser::new();
    parser.handle_key('d');
    parser.handle_key('i');
    assert_eq!(parser.handle_key('z'), VimAction::Noop); // invalid object
    // Parser should be clean
    assert_eq!(parser.handle_key('j'), VimAction::Motion(MotionKind::Down));
}
```

Add imports in the test module:
```rust
use crate::vim::text_object::{TextObject, TextObjectKind};
```

Run: `cargo test --lib vim::keyparser`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
git add src/vim/action.rs src/vim/keyparser.rs src/vim/operator.rs src/editor.rs
git commit -m "feat: wire text objects into actions, keyparser, and operator engine"
```

---

### Task 8: Visual Mode Types

Extend Mode enum and add visual mode action types.

**Files:**
- Modify: `src/vim/mode.rs`
- Modify: `src/vim/action.rs`

- [ ] **Step 1: Extend Mode enum**

In `src/vim/mode.rs`, add visual variants:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Command,
    Visual,
    VisualLine,
    VisualBlock,
}

impl Mode {
    pub fn status_text(&self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Command => "COMMAND",
            Mode::Visual => "VISUAL",
            Mode::VisualLine => "V-LINE",
            Mode::VisualBlock => "V-BLOCK",
        }
    }

    pub fn is_visual(&self) -> bool {
        matches!(self, Mode::Visual | Mode::VisualLine | Mode::VisualBlock)
    }
}
```

Update tests:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_status_text() {
        assert_eq!(Mode::Normal.status_text(), "NORMAL");
        assert_eq!(Mode::Insert.status_text(), "INSERT");
        assert_eq!(Mode::Command.status_text(), "COMMAND");
        assert_eq!(Mode::Visual.status_text(), "VISUAL");
        assert_eq!(Mode::VisualLine.status_text(), "V-LINE");
        assert_eq!(Mode::VisualBlock.status_text(), "V-BLOCK");
    }

    #[test]
    fn is_visual() {
        assert!(!Mode::Normal.is_visual());
        assert!(!Mode::Insert.is_visual());
        assert!(Mode::Visual.is_visual());
        assert!(Mode::VisualLine.is_visual());
        assert!(Mode::VisualBlock.is_visual());
    }
}
```

- [ ] **Step 2: Add visual action types to action.rs**

In `src/vim/action.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum VisualKind {
    Char,
    Line,
    Block,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VisualOperatorAction {
    Delete,
    Change,
    Yank,
}
```

Add to `VimAction`:
```rust
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
    EnterInsert(InsertEntry),
    EnterVisual(VisualKind),
    VisualOperator(VisualOperatorAction),
    SwapVisualAnchor,
    Noop,
}
```

- [ ] **Step 3: Fix exhaustive match warnings**

Run `cargo check` and fix any non-exhaustive match arms in `src/editor.rs` `apply_action()` and `src/vim/keyparser.rs`. Add placeholder arms:

In `editor.rs` `apply_action()`, add before `VimAction::Noop`:
```rust
VimAction::EnterVisual(_) => {} // Handled in Task 10
VimAction::VisualOperator(_) => {} // Handled in Task 10
VimAction::SwapVisualAnchor => {} // Handled in Task 10
```

In `editor_view.rs` cursor rendering, update match:
```rust
match mode {
    Mode::Normal | Mode::Command => { /* block cursor */ }
    Mode::Insert => { /* line cursor */ }
    Mode::Visual | Mode::VisualLine | Mode::VisualBlock => { /* block cursor */ }
}
```

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add src/vim/mode.rs src/vim/action.rs src/editor.rs src/renderer/editor_view.rs
git commit -m "feat: add visual mode types and action enums"
```

---

### Task 9: Visual Mode Selection State + Range Resolution

Add VisualAnchor to Editor and selection range computation.

**Files:**
- Modify: `src/editor.rs`

- [ ] **Step 1: Add VisualAnchor struct and field**

In `src/editor.rs`:

```rust
#[derive(Debug, Clone, Copy)]
pub struct VisualAnchor {
    pub line: usize,
    pub col: usize,
}

pub struct Editor {
    pub buffer: TextBuffer,
    pub key_parser: KeyParser,
    pub operator_engine: OperatorEngine,
    pub file_path: Option<String>,
    pub should_quit: bool,
    pub status_message: Option<String>,
    pub command_parser: CommandParser,
    pub visual_anchor: Option<VisualAnchor>,
}
```

Update `Editor::new()` to include `visual_anchor: None`.

- [ ] **Step 2: Add selection range resolution methods**

```rust
impl Editor {
    /// Returns the char range [start, end) of the visual selection.
    pub fn visual_selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.visual_anchor?;
        let anchor_offset = self.buffer.line_to_char(anchor.line) + anchor.col;
        let cursor_offset = self.buffer.cursor_offset();

        match self.mode() {
            Mode::Visual => {
                let start = anchor_offset.min(cursor_offset);
                let end = anchor_offset.max(cursor_offset) + 1; // inclusive
                Some((start, end.min(self.buffer.len_chars())))
            }
            Mode::VisualLine => {
                let start_line = anchor.line.min(self.buffer.cursor_line());
                let end_line = anchor.line.max(self.buffer.cursor_line());
                let start = self.buffer.line_to_char(start_line);
                let end = if end_line + 1 < self.buffer.line_count() {
                    self.buffer.line_to_char(end_line + 1)
                } else {
                    self.buffer.len_chars()
                };
                Some((start, end))
            }
            Mode::VisualBlock => {
                // Block mode returns the rectangle bounds, not a single range.
                // For operators, we handle block mode specially in apply_action.
                // This method returns the bounding char range for highlighting purposes.
                let start_line = anchor.line.min(self.buffer.cursor_line());
                let end_line = anchor.line.max(self.buffer.cursor_line());
                let start = self.buffer.line_to_char(start_line);
                let end = if end_line + 1 < self.buffer.line_count() {
                    self.buffer.line_to_char(end_line + 1)
                } else {
                    self.buffer.len_chars()
                };
                Some((start, end))
            }
            _ => None,
        }
    }

    /// Returns (start_line, end_line, start_col, end_col) for block selection.
    pub fn visual_block_bounds(&self) -> Option<(usize, usize, usize, usize)> {
        let anchor = self.visual_anchor?;
        if self.mode() != Mode::VisualBlock {
            return None;
        }
        let start_line = anchor.line.min(self.buffer.cursor_line());
        let end_line = anchor.line.max(self.buffer.cursor_line());
        let start_col = anchor.col.min(self.buffer.cursor_col());
        let end_col = anchor.col.max(self.buffer.cursor_col());
        Some((start_line, end_line, start_col, end_col))
    }
}
```

- [ ] **Step 3: Implement EnterVisual and Escape handling in apply_action**

Update `apply_action()`:

```rust
VimAction::EnterVisual(ref kind) => {
    self.visual_anchor = Some(VisualAnchor {
        line: self.buffer.cursor_line(),
        col: self.buffer.cursor_col(),
    });
    // Mode is already set by KeyParser
}
VimAction::SwapVisualAnchor => {
    if let Some(ref mut anchor) = self.visual_anchor {
        let old_anchor_line = anchor.line;
        let old_anchor_col = anchor.col;
        anchor.line = self.buffer.cursor_line();
        anchor.col = self.buffer.cursor_col();
        self.buffer.set_cursor(old_anchor_line, old_anchor_col);
    }
}
```

Update the `SwitchMode(Mode::Normal)` arm to clear visual state. Note: cursor-back-one-col only applies when exiting Insert mode (not visual mode):
```rust
VimAction::SwitchMode(Mode::Normal) => {
    self.visual_anchor = None;
    // Only do Insert-mode cleanup if an undo group was open
    // (end_undo_group returns true if a group was actually ended)
    let was_insert = self.buffer.end_undo_group();
    if was_insert {
        let col = self.buffer.cursor_col();
        if col > 0 {
            self.buffer.set_cursor(self.buffer.cursor_line(), col - 1);
        }
    }
    self.buffer.clamp_cursor_normal();
}
```

This requires `end_undo_group()` to return `bool`. Update `src/buffer/text_buffer.rs`:
```rust
pub fn end_undo_group(&mut self) -> bool {
    self.history.end_group()
}
```

And `src/buffer/history.rs` — `end_group()` should return `bool` indicating if a group was actually ended:
```rust
pub fn end_group(&mut self) -> bool {
    if let Some(actions) = self.current_group.take() {
        if !actions.is_empty() {
            self.push_entry(UndoEntry::Group(actions));
        }
        true
    } else {
        false
    }
}
```

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add src/editor.rs
git commit -m "feat: add visual selection state and range resolution"
```

---

### Task 10: Visual Mode KeyParser + Operator Execution

Handle visual mode keys and execute visual operators.

**Files:**
- Modify: `src/vim/keyparser.rs`
- Modify: `src/editor.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Add visual mode handling to KeyParser**

In `src/vim/keyparser.rs`, add to `handle_key()`:

```rust
pub fn handle_key(&mut self, ch: char) -> VimAction {
    match self.mode {
        Mode::Normal => self.handle_normal(ch),
        Mode::Insert => self.handle_insert(ch),
        Mode::Command => VimAction::Noop,
        Mode::Visual | Mode::VisualLine | Mode::VisualBlock => self.handle_visual(ch),
    }
}
```

Add new method `handle_visual()`:
```rust
fn handle_visual(&mut self, ch: char) -> VimAction {
    // Handle pending sequences (e.g. gg)
    if !self.pending.is_empty() {
        let combined = format!("{}{}", self.pending, ch);
        self.pending.clear();
        return match combined.as_str() {
            "gg" => VimAction::Motion(MotionKind::FileTop),
            _ => VimAction::Noop,
        };
    }

    match ch {
        // Motions work in visual mode (move cursor, selection follows)
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
        'g' => {
            self.pending.push(ch);
            VimAction::Noop
        }

        // Operators on selection
        'd' => {
            self.mode = Mode::Normal;
            VimAction::VisualOperator(VisualOperatorAction::Delete)
        }
        'c' => {
            self.mode = Mode::Insert;
            VimAction::VisualOperator(VisualOperatorAction::Change)
        }
        'y' => {
            self.mode = Mode::Normal;
            VimAction::VisualOperator(VisualOperatorAction::Yank)
        }
        'x' => {
            self.mode = Mode::Normal;
            VimAction::VisualOperator(VisualOperatorAction::Delete)
        }

        // Swap anchor/cursor
        'o' => VimAction::SwapVisualAnchor,

        // Switch between visual sub-modes
        'v' => {
            if self.mode == Mode::Visual {
                self.mode = Mode::Normal;
                VimAction::SwitchMode(Mode::Normal)
            } else {
                self.mode = Mode::Visual;
                VimAction::SwitchMode(Mode::Visual)
            }
        }
        'V' => {
            if self.mode == Mode::VisualLine {
                self.mode = Mode::Normal;
                VimAction::SwitchMode(Mode::Normal)
            } else {
                self.mode = Mode::VisualLine;
                VimAction::SwitchMode(Mode::VisualLine)
            }
        }

        _ => VimAction::Noop,
    }
}
```

In `handle_normal()`, add visual mode entry keys (in the single-char match block):
```rust
'v' => {
    self.mode = Mode::Visual;
    VimAction::EnterVisual(VisualKind::Char)
}
'V' => {
    self.mode = Mode::VisualLine;
    VimAction::EnterVisual(VisualKind::Line)
}
```

Also update `handle_escape()` to work from visual modes (already works since it sets mode to Normal).

- [ ] **Step 2: Handle Ctrl+V in app.rs**

In `src/app.rs`, add Ctrl+V detection before the text input loop. Add this after the Ctrl+R handling block:

```rust
// Ctrl+V for visual block mode — only in Normal mode
if self.editor.mode() == Mode::Normal
    && input.modifiers.ctrl
    && input.key_pressed(egui::Key::V)
{
    // Directly set visual block mode
    self.editor.key_parser.set_mode(Mode::VisualBlock);
    let action = VimAction::EnterVisual(VisualKind::Block);
    self.editor.apply_action(action);
    return;
}
```

Add import for `VimAction` and `VisualKind` at the top of `app.rs`:
```rust
use crate::vim::{Mode, VimAction, VisualKind};
```

Note: `set_mode` currently is `#[cfg(test)]` only. Remove the `#[cfg(test)]` attribute from `set_mode` in keyparser.rs to make it available.

- [ ] **Step 3: Implement VisualOperator in Editor.apply_action()**

Replace the placeholder arms in `apply_action()`:

```rust
VimAction::VisualOperator(ref vis_op) => {
    let register = register; // already captured above
    if let Some((start, end)) = self.visual_selection_range() {
        let content = self.buffer.slice(start, end);
        let linewise = self.mode().is_visual()
            && matches!(self.key_parser.mode(), Mode::Normal)
            || matches!(self.mode(), Mode::VisualLine);
        // Determine linewise based on pre-switch mode
        // (KeyParser already switched mode before we get here)
        let was_visual_line = content.ends_with('\n');

        match vis_op {
            VisualOperatorAction::Delete => {
                self.operator_engine
                    .registers
                    .set(register, content, was_visual_line);
                self.buffer.delete_range(start, end);
                self.buffer.update_cursor_from_offset(start);
                self.buffer.clamp_cursor_normal();
            }
            VisualOperatorAction::Change => {
                self.operator_engine
                    .registers
                    .set(register, content, was_visual_line);
                self.buffer.delete_range(start, end);
                self.buffer.update_cursor_from_offset(start);
                self.buffer.begin_undo_group();
            }
            VisualOperatorAction::Yank => {
                self.operator_engine
                    .registers
                    .set(register, content, was_visual_line);
                self.buffer.update_cursor_from_offset(start);
                self.buffer.clamp_cursor_normal();
            }
        }
    }
    self.visual_anchor = None;
}
```

- [ ] **Step 4: Write and run visual mode tests**

Add to `src/vim/keyparser.rs` tests:

```rust
#[test]
fn v_enters_visual_mode() {
    let mut parser = KeyParser::new();
    let action = parser.handle_key('v');
    assert_eq!(action, VimAction::EnterVisual(VisualKind::Char));
    assert_eq!(parser.mode(), Mode::Visual);
}

#[test]
fn visual_v_enters_visual_line() {
    let mut parser = KeyParser::new();
    let action = parser.handle_key('V');
    assert_eq!(action, VimAction::EnterVisual(VisualKind::Line));
    assert_eq!(parser.mode(), Mode::VisualLine);
}

#[test]
fn visual_mode_d_deletes_selection() {
    let mut parser = KeyParser::new();
    parser.handle_key('v'); // enter visual
    let action = parser.handle_key('d');
    assert_eq!(action, VimAction::VisualOperator(VisualOperatorAction::Delete));
    assert_eq!(parser.mode(), Mode::Normal);
}

#[test]
fn visual_mode_y_yanks_selection() {
    let mut parser = KeyParser::new();
    parser.handle_key('v');
    let action = parser.handle_key('y');
    assert_eq!(action, VimAction::VisualOperator(VisualOperatorAction::Yank));
    assert_eq!(parser.mode(), Mode::Normal);
}

#[test]
fn visual_mode_c_changes_selection() {
    let mut parser = KeyParser::new();
    parser.handle_key('v');
    let action = parser.handle_key('c');
    assert_eq!(action, VimAction::VisualOperator(VisualOperatorAction::Change));
    assert_eq!(parser.mode(), Mode::Insert);
}

#[test]
fn visual_mode_o_swaps_anchor() {
    let mut parser = KeyParser::new();
    parser.handle_key('v');
    let action = parser.handle_key('o');
    assert_eq!(action, VimAction::SwapVisualAnchor);
}

#[test]
fn visual_mode_escape_returns_normal() {
    let mut parser = KeyParser::new();
    parser.handle_key('v');
    let action = parser.handle_escape();
    assert_eq!(action, VimAction::SwitchMode(Mode::Normal));
    assert_eq!(parser.mode(), Mode::Normal);
}

#[test]
fn visual_mode_motions_work() {
    let mut parser = KeyParser::new();
    parser.handle_key('v');
    assert_eq!(parser.handle_key('j'), VimAction::Motion(MotionKind::Down));
    assert_eq!(parser.handle_key('w'), VimAction::Motion(MotionKind::WordForward));
}
```

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add src/vim/keyparser.rs src/editor.rs src/app.rs
git commit -m "feat: add visual mode keybindings and operator execution"
```

---

### Task 11: Visual Mode Rendering

Render selection highlights for all three visual modes.

**Files:**
- Modify: `src/renderer/editor_view.rs`
- Modify: `src/renderer/status_bar.rs`

- [ ] **Step 1: Pass visual selection data to renderer**

In `src/app.rs`, update the `render()` call to pass visual selection info. First, add a `visual_info` parameter. Modify `editor_view.rs` to accept it.

Add a struct for passing visual info:

In `src/editor.rs`, add a public method:
```rust
/// Get visual highlight ranges for the renderer.
/// Returns Vec of (start_col, end_col) per visible line for highlighting.
pub fn visual_highlights_for_line(&self, line_idx: usize) -> Option<(usize, usize)> {
    let anchor = self.visual_anchor?;
    match self.mode() {
        Mode::Visual => {
            let anchor_offset = self.buffer.line_to_char(anchor.line) + anchor.col;
            let cursor_offset = self.buffer.cursor_offset();
            let sel_start = anchor_offset.min(cursor_offset);
            let sel_end = anchor_offset.max(cursor_offset) + 1;

            let line_start = self.buffer.line_to_char(line_idx);
            let line_end = line_start + self.buffer.line_len_chars(line_idx);

            if sel_end <= line_start || sel_start >= line_end {
                return None; // No overlap
            }

            let col_start = if sel_start > line_start {
                sel_start - line_start
            } else {
                0
            };
            let col_end = if sel_end < line_end {
                sel_end - line_start
            } else {
                self.buffer.line_content_len(line_idx)
            };

            Some((col_start, col_end))
        }
        Mode::VisualLine => {
            let start_line = anchor.line.min(self.buffer.cursor_line());
            let end_line = anchor.line.max(self.buffer.cursor_line());
            if line_idx >= start_line && line_idx <= end_line {
                Some((0, self.buffer.line_content_len(line_idx)))
            } else {
                None
            }
        }
        Mode::VisualBlock => {
            let start_line = anchor.line.min(self.buffer.cursor_line());
            let end_line = anchor.line.max(self.buffer.cursor_line());
            if line_idx >= start_line && line_idx <= end_line {
                let start_col = anchor.col.min(self.buffer.cursor_col());
                let end_col = anchor.col.max(self.buffer.cursor_col()) + 1;
                Some((start_col, end_col))
            } else {
                None
            }
        }
        _ => None,
    }
}
```

- [ ] **Step 2: Update EditorView.render() to accept Editor reference**

Change the `render()` signature in `src/renderer/editor_view.rs` to accept an `&Editor` instead of individual parts. This reduces parameter count and gives access to visual highlight data.

```rust
use crate::editor::Editor;

pub fn render(
    &mut self,
    ui: &mut egui::Ui,
    editor: &Editor,
    theme: &Theme,
    font_size: f32,
) {
    let buffer = &editor.buffer;
    let mode = editor.mode();
    let file_path = editor.file_path.as_deref();
    let command_input = editor.command_input();
    let status_message = editor.status_message.as_deref();
    // ... rest of existing logic, but now can call editor.visual_highlights_for_line(i)
```

Update the per-line rendering loop. After drawing text and before drawing the cursor, add selection highlighting:

```rust
// Selection highlight (visual modes)
if let Some((sel_start_col, sel_end_col)) = editor.visual_highlights_for_line(i) {
    let sel_x_start: f32 = if sel_start_col == 0 {
        0.0
    } else {
        let prefix: String = display.chars().take(sel_start_col).collect();
        painter
            .layout_no_wrap(prefix, font_id.clone(), theme.foreground)
            .rect
            .width()
    };
    let sel_x_end: f32 = {
        let prefix: String = display.chars().take(sel_end_col).collect();
        painter
            .layout_no_wrap(prefix, font_id.clone(), theme.foreground)
            .rect
            .width()
    };
    let sel_rect = Rect::from_min_size(
        egui::pos2(text_x + sel_x_start, y),
        Vec2::new(sel_x_end - sel_x_start, line_height),
    );
    painter.rect_filled(sel_rect, 0.0, theme.selection);
}
```

- [ ] **Step 3: Update app.rs to use new render signature**

In `src/app.rs`, update the render call:

```rust
self.editor_view.render(
    ui,
    &self.editor,
    &self.theme,
    self.config.editor.font_size,
);
```

- [ ] **Step 4: Update status bar for visual modes**

The status bar already uses `mode.status_text()` which now returns "VISUAL", "V-LINE", "V-BLOCK". No changes needed.

Run: `cargo test`
Expected: All tests pass (rendering is tested visually)

- [ ] **Step 5: Commit**

```bash
git add src/editor.rs src/renderer/editor_view.rs src/app.rs
git commit -m "feat: add visual mode selection highlighting"
```

---

### Task 12: Dot-Repeat

Record last editing action and replay it with `.`.

**Files:**
- Modify: `src/vim/action.rs`
- Modify: `src/vim/keyparser.rs`
- Modify: `src/editor.rs`

- [ ] **Step 1: Add DotRepeat to VimAction**

In `src/vim/action.rs`, add to `VimAction`:
```rust
DotRepeat,
```

- [ ] **Step 2: Add LastAction struct and recording to Editor**

In `src/editor.rs`:

```rust
#[derive(Debug, Clone)]
pub struct LastAction {
    pub entry: Option<InsertEntry>,
    pub actions: Vec<VimAction>,
    pub count: usize,
}

pub struct Editor {
    // ... existing fields ...
    pub last_action: Option<LastAction>,
    recording_action: Option<LastAction>,
    replaying: bool,
}
```

Update `Editor::new()` to include:
```rust
last_action: None,
recording_action: None,
replaying: false,
```

- [ ] **Step 3: Implement recording in apply_action**

Add recording logic. Operators are recorded immediately. Insert sessions accumulate.

At the top of `apply_action()`, after `let register = ...`:

```rust
// Record for dot-repeat (skip during replay)
if !self.replaying {
    match &action {
        VimAction::Operator(_) | VimAction::VisualOperator(_) => {
            self.last_action = Some(LastAction {
                entry: None,
                actions: vec![action.clone()],
                count,
            });
            self.recording_action = None;
        }
        VimAction::EnterInsert(entry) => {
            self.recording_action = Some(LastAction {
                entry: Some(entry.clone()),
                actions: vec![],
                count: 1,
            });
        }
        VimAction::InsertChar(_) | VimAction::DeleteCharBefore => {
            if let Some(ref mut rec) = self.recording_action {
                rec.actions.push(action.clone());
            }
        }
        VimAction::SwitchMode(Mode::Normal) => {
            if let Some(rec) = self.recording_action.take() {
                self.last_action = Some(rec);
            }
        }
        _ => {}
    }
}
```

- [ ] **Step 4: Implement DotRepeat handling**

Add to `apply_action()`:

```rust
VimAction::DotRepeat => {
    if let Some(ref last) = self.last_action.clone() {
        self.replaying = true;
        let repeat_count = if count > 1 { count } else { last.count };

        if let Some(ref entry) = last.entry {
            // Insert session replay
            for _ in 0..repeat_count {
                self.apply_action(VimAction::EnterInsert(entry.clone()));
                for a in &last.actions {
                    self.apply_action(a.clone());
                }
                self.apply_action(VimAction::SwitchMode(Mode::Normal));
            }
        } else {
            // Operator replay
            for a in &last.actions {
                for _ in 0..repeat_count {
                    self.apply_action(a.clone());
                }
            }
        }
        self.replaying = false;
    }
}
```

- [ ] **Step 5: Add `.` to KeyParser**

In `src/vim/keyparser.rs`, in the `handle_normal()` single-char match, add:

```rust
'.' => VimAction::DotRepeat,
```

- [ ] **Step 6: Write and run tests**

Add to `src/vim/keyparser.rs` tests:

```rust
#[test]
fn dot_emits_dot_repeat() {
    let mut parser = KeyParser::new();
    assert_eq!(parser.handle_key('.'), VimAction::DotRepeat);
}
```

Add integration-style tests in `src/editor.rs` (add `#[cfg(test)] mod tests`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::vim::action::*;

    #[test]
    fn dot_repeat_operator() {
        let mut editor = Editor::new(None);
        editor.buffer = TextBuffer::from_text("hello\nworld\nfoo");
        editor.buffer.set_cursor(0, 0);

        // dd deletes first line
        editor.apply_action(VimAction::Operator(OperatorAction::DeleteLine));
        assert_eq!(editor.buffer.text(), "world\nfoo");

        // . repeats dd
        editor.apply_action(VimAction::DotRepeat);
        assert_eq!(editor.buffer.text(), "foo");
    }

    #[test]
    fn dot_repeat_insert_session() {
        let mut editor = Editor::new(None);
        editor.buffer = TextBuffer::from_text("hello");
        editor.buffer.set_cursor(0, 4); // on 'o'

        // A (append at end) + type "!" + Escape
        editor.apply_action(VimAction::EnterInsert(InsertEntry::EndOfLine));
        editor.apply_action(VimAction::InsertChar('!'));
        editor.apply_action(VimAction::SwitchMode(Mode::Normal));
        assert_eq!(editor.buffer.text(), "hello!");

        // Move to another position and dot-repeat
        editor.buffer.set_cursor(0, 0);
        editor.apply_action(VimAction::DotRepeat);
        assert_eq!(editor.buffer.text(), "hello!!");
    }

    #[test]
    fn dot_repeat_with_count_override() {
        let mut editor = Editor::new(None);
        editor.buffer = TextBuffer::from_text("aaa\nbbb\nccc\nddd");
        editor.buffer.set_cursor(0, 0);

        // dd (count=1)
        editor.apply_action(VimAction::Operator(OperatorAction::DeleteLine));
        assert_eq!(editor.buffer.line_count(), 3);

        // 2. should repeat dd twice
        editor.key_parser.handle_key('2');
        editor.apply_action(VimAction::DotRepeat);
        assert_eq!(editor.buffer.text(), "ddd");
    }
}
```

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 7: Commit**

```bash
git add src/vim/action.rs src/vim/keyparser.rs src/editor.rs
git commit -m "feat: add dot-repeat for operators and insert sessions"
```

---

### Task 13: Search Core

Add SearchState with match finding and navigation.

**Files:**
- Create: `src/vim/search.rs`
- Modify: `src/vim/mod.rs`
- Modify: `src/vim/action.rs`

- [ ] **Step 1: Add search action types**

In `src/vim/action.rs`, add:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum SearchDirection {
    Forward,
    Backward,
}
```

Add to `VimAction`:
```rust
EnterSearch(SearchDirection),
SearchNext,
SearchPrev,
```

- [ ] **Step 2: Write failing tests for SearchState**

Create `src/vim/search.rs`:

```rust
use crate::vim::action::SearchDirection;

pub struct SearchState {
    pub pattern: String,
    pub direction: SearchDirection,
    pub matches: Vec<(usize, usize)>, // (start_offset, end_offset) in char indices
    pub current_match: Option<usize>,  // index into matches vec
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            pattern: String::new(),
            direction: SearchDirection::Forward,
            matches: Vec::new(),
            current_match: None,
        }
    }

    /// Find all literal substring matches in the given text.
    /// Returns (start, end) pairs as char offsets.
    pub fn find_matches(&mut self, text: &str) {
        self.matches.clear();
        self.current_match = None;

        if self.pattern.is_empty() {
            return;
        }

        todo!()
    }

    /// Jump to the next match from the given cursor offset.
    pub fn next_match(&mut self, cursor_offset: usize) -> Option<usize> {
        todo!()
    }

    /// Jump to the previous match from the given cursor offset.
    pub fn prev_match(&mut self, cursor_offset: usize) -> Option<usize> {
        todo!()
    }

    /// Jump to the nearest match in the current search direction.
    pub fn jump_to_nearest(&mut self, cursor_offset: usize) -> Option<usize> {
        match self.direction {
            SearchDirection::Forward => self.next_match(cursor_offset),
            SearchDirection::Backward => self.prev_match(cursor_offset),
        }
    }

    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    pub fn current_match_index(&self) -> Option<usize> {
        self.current_match
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_matches_simple() {
        let mut state = SearchState::new();
        state.pattern = "hello".into();
        state.find_matches("say hello world hello end");
        assert_eq!(state.matches.len(), 2);
        assert_eq!(state.matches[0], (4, 9));
        assert_eq!(state.matches[1], (16, 21));
    }

    #[test]
    fn find_matches_empty_pattern() {
        let mut state = SearchState::new();
        state.pattern = String::new();
        state.find_matches("hello");
        assert!(state.matches.is_empty());
    }

    #[test]
    fn find_matches_no_match() {
        let mut state = SearchState::new();
        state.pattern = "xyz".into();
        state.find_matches("hello world");
        assert!(state.matches.is_empty());
    }

    #[test]
    fn find_matches_unicode() {
        let mut state = SearchState::new();
        state.pattern = "på".into();
        state.find_matches("hej på dig");
        assert_eq!(state.matches.len(), 1);
        assert_eq!(state.matches[0], (4, 6)); // char indices
    }

    #[test]
    fn next_match_forward() {
        let mut state = SearchState::new();
        state.pattern = "x".into();
        state.direction = SearchDirection::Forward;
        state.find_matches("axbxcx");
        // cursor at 0, next should find index 1 ('x' at pos 1)
        let offset = state.next_match(0);
        assert_eq!(offset, Some(1));
        assert_eq!(state.current_match, Some(0));
    }

    #[test]
    fn next_match_wraps() {
        let mut state = SearchState::new();
        state.pattern = "a".into();
        state.find_matches("abc");
        // cursor past the only match, should wrap
        let offset = state.next_match(2);
        assert_eq!(offset, Some(0));
    }

    #[test]
    fn prev_match_backward() {
        let mut state = SearchState::new();
        state.pattern = "x".into();
        state.find_matches("axbxcx");
        // cursor at 5, prev should find the match at pos 3
        let offset = state.prev_match(5);
        assert_eq!(offset, Some(3));
    }

    #[test]
    fn prev_match_wraps() {
        let mut state = SearchState::new();
        state.pattern = "x".into();
        state.find_matches("axbx");
        // cursor at 0, prev should wrap to last match
        let offset = state.prev_match(0);
        assert_eq!(offset, Some(3));
    }
}
```

Run: `cargo test --lib vim::search`
Expected: FAIL — `todo!()`

- [ ] **Step 3: Implement SearchState methods**

Replace the `todo!()` stubs:

```rust
pub fn find_matches(&mut self, text: &str) {
    self.matches.clear();
    self.current_match = None;

    if self.pattern.is_empty() {
        return;
    }

    let pattern_chars: Vec<char> = self.pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();
    let pat_len = pattern_chars.len();

    if pat_len > text_chars.len() {
        return;
    }

    for i in 0..=(text_chars.len() - pat_len) {
        if text_chars[i..i + pat_len] == pattern_chars[..] {
            self.matches.push((i, i + pat_len));
        }
    }
}

pub fn next_match(&mut self, cursor_offset: usize) -> Option<usize> {
    if self.matches.is_empty() {
        return None;
    }
    // Find first match starting after cursor_offset
    for (idx, &(start, _)) in self.matches.iter().enumerate() {
        if start > cursor_offset {
            self.current_match = Some(idx);
            return Some(start);
        }
    }
    // Wrap to first match
    self.current_match = Some(0);
    Some(self.matches[0].0)
}

pub fn prev_match(&mut self, cursor_offset: usize) -> Option<usize> {
    if self.matches.is_empty() {
        return None;
    }
    // Find last match starting before cursor_offset
    for (idx, &(start, _)) in self.matches.iter().enumerate().rev() {
        if start < cursor_offset {
            self.current_match = Some(idx);
            return Some(start);
        }
    }
    // Wrap to last match
    let last = self.matches.len() - 1;
    self.current_match = Some(last);
    Some(self.matches[last].0)
}
```

Run: `cargo test --lib vim::search`
Expected: All tests PASS

- [ ] **Step 4: Export search module**

In `src/vim/mod.rs`, add:
```rust
pub(crate) mod search;
```

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add src/vim/search.rs src/vim/mod.rs src/vim/action.rs
git commit -m "feat: add search state with match finding and navigation"
```

---

### Task 14: Search Input Mode + KeyParser Integration

Wire search into KeyParser, Editor, and app input handling.

**Files:**
- Modify: `src/vim/keyparser.rs`
- Modify: `src/editor.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Add search keys to KeyParser**

In `handle_normal()`, add to the single-char match:

```rust
'/' => VimAction::EnterSearch(SearchDirection::Forward),
'?' => VimAction::EnterSearch(SearchDirection::Backward),
'n' => VimAction::SearchNext,
'N' => VimAction::SearchPrev,
```

Add import:
```rust
use crate::vim::action::SearchDirection;
```

- [ ] **Step 2: Add SearchState to Editor**

In `src/editor.rs`:

```rust
use crate::vim::search::SearchState;
use crate::vim::action::SearchDirection;
```

Add field to `Editor`:
```rust
pub search_state: SearchState,
pub search_input: Option<String>, // Some while entering search pattern
```

Update `Editor::new()`:
```rust
search_state: SearchState::new(),
search_input: None,
```

Add search methods:
```rust
pub fn start_search(&mut self, direction: SearchDirection) {
    self.search_state.direction = direction;
    self.search_input = Some(String::new());
}

pub fn handle_search_char(&mut self, ch: char) {
    if let Some(ref mut input) = self.search_input {
        input.push(ch);
    }
}

pub fn handle_search_backspace(&mut self) {
    if let Some(ref mut input) = self.search_input {
        input.pop();
        if input.is_empty() {
            self.search_input = None;
            let action = self.key_parser.handle_escape();
            self.apply_action(action);
        }
    }
}

pub fn execute_search(&mut self) {
    if let Some(input) = self.search_input.take() {
        if !input.is_empty() {
            self.search_state.pattern = input;
            self.search_state.find_matches(&self.buffer.text());
            let cursor_offset = self.buffer.cursor_offset();
            if let Some(offset) = self.search_state.jump_to_nearest(cursor_offset) {
                self.buffer.update_cursor_from_offset(offset);
                let total = self.search_state.match_count();
                let current = self.search_state.current_match_index().unwrap_or(0) + 1;
                self.status_message = Some(format!("[{}/{}]", current, total));
            } else {
                self.status_message = Some("Pattern not found".to_string());
            }
        }
        let action = self.key_parser.handle_escape();
        self.apply_action(action);
    }
}

pub fn search_input_display(&self) -> Option<String> {
    self.search_input.as_ref().map(|input| {
        let prefix = match self.search_state.direction {
            SearchDirection::Forward => "/",
            SearchDirection::Backward => "?",
        };
        format!("{}{}", prefix, input)
    })
}
```

- [ ] **Step 3: Handle SearchNext/SearchPrev in apply_action**

Add to `apply_action()`:
```rust
VimAction::EnterSearch(ref direction) => {
    self.start_search(direction.clone());
}
VimAction::SearchNext => {
    if !self.search_state.pattern.is_empty() {
        // Re-search if matches cache is empty (buffer was modified)
        if self.search_state.matches.is_empty() {
            self.search_state.find_matches(&self.buffer.text());
        }
        let cursor = self.buffer.cursor_offset();
        if let Some(offset) = self.search_state.next_match(cursor) {
            self.buffer.update_cursor_from_offset(offset);
            let total = self.search_state.match_count();
            let current = self.search_state.current_match_index().unwrap_or(0) + 1;
            self.status_message = Some(format!("[{}/{}]", current, total));
        }
    }
}
VimAction::SearchPrev => {
    if !self.search_state.pattern.is_empty() {
        if self.search_state.matches.is_empty() {
            self.search_state.find_matches(&self.buffer.text());
        }
        let cursor = self.buffer.cursor_offset();
        if let Some(offset) = self.search_state.prev_match(cursor) {
            self.buffer.update_cursor_from_offset(offset);
            let total = self.search_state.match_count();
            let current = self.search_state.current_match_index().unwrap_or(0) + 1;
            self.status_message = Some(format!("[{}/{}]", current, total));
        }
    }
}
```

Invalidate search cache on buffer modifications. At the beginning of `apply_action()`, after the Noop check:
```rust
// Invalidate search cache on edits
match &action {
    VimAction::InsertChar(_)
    | VimAction::DeleteCharBefore
    | VimAction::Operator(_)
    | VimAction::VisualOperator(_)
    | VimAction::Paste
    | VimAction::Undo
    | VimAction::Redo => {
        self.search_state.matches.clear();
    }
    _ => {}
}
```

- [ ] **Step 4: Handle search input mode in app.rs**

In `src/app.rs`, in `handle_input()`, add a block for search input mode before the normal key handling. Place it after the command mode block:

```rust
// Search input mode
if self.editor.search_input.is_some() {
    if input.key_pressed(egui::Key::Enter) {
        self.editor.execute_search();
        return;
    }
    if input.key_pressed(egui::Key::Escape)
        || (input.modifiers.ctrl && input.key_pressed(egui::Key::OpenBracket))
    {
        self.editor.search_input = None;
        let action = self.editor.key_parser.handle_escape();
        self.editor.apply_action(action);
        return;
    }
    if input.key_pressed(egui::Key::Backspace) {
        self.editor.handle_search_backspace();
        return;
    }
    for event in &input.events {
        if let egui::Event::Text(text) = event {
            for ch in text.chars() {
                self.editor.handle_search_char(ch);
            }
        }
    }
    return;
}
```

Update the render call to pass search input display. In the status bar section, pass the search input as an additional parameter or overlay it on the command input area. The simplest approach: show search input display in place of command_input when searching:

In `src/app.rs`, update render:
```rust
let command_input = self.editor.command_input();
let search_display = self.editor.search_input_display();
// Use search display if active, otherwise command input
let bottom_input = search_display.as_deref().or(command_input);
```

Then pass `bottom_input` instead of `command_input` to the render method.

- [ ] **Step 5: Write and run tests**

Add to keyparser tests:
```rust
#[test]
fn forward_search_key() {
    let mut parser = KeyParser::new();
    assert_eq!(
        parser.handle_key('/'),
        VimAction::EnterSearch(SearchDirection::Forward)
    );
}

#[test]
fn backward_search_key() {
    let mut parser = KeyParser::new();
    assert_eq!(
        parser.handle_key('?'),
        VimAction::EnterSearch(SearchDirection::Backward)
    );
}

#[test]
fn search_next_key() {
    let mut parser = KeyParser::new();
    assert_eq!(parser.handle_key('n'), VimAction::SearchNext);
}

#[test]
fn search_prev_key() {
    let mut parser = KeyParser::new();
    assert_eq!(parser.handle_key('N'), VimAction::SearchPrev);
}
```

Add import:
```rust
use crate::vim::action::SearchDirection;
```

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
git add src/vim/keyparser.rs src/editor.rs src/app.rs
git commit -m "feat: add search input mode with / ? n N keybindings"
```

---

### Task 15: Search Match Rendering

Render search match highlights and add theme colors.

**Files:**
- Modify: `src/renderer/theme.rs`
- Modify: `src/renderer/editor_view.rs`
- Modify: `src/editor.rs`

- [ ] **Step 1: Add search colors to Theme**

In `src/renderer/theme.rs`, add to `Theme`:
```rust
pub search_match: Color32,
pub search_current: Color32,
```

Update `default_dark()`:
```rust
search_match: Color32::from_rgba_premultiplied(0xf9, 0xe2, 0xaf, 0x50), // soft yellow, semi-transparent
search_current: Color32::from_rgba_premultiplied(0xfa, 0xb3, 0x87, 0x80), // orange, more opaque
```

- [ ] **Step 2: Add search highlight helper to Editor**

In `src/editor.rs`:

```rust
/// Get search highlight ranges for a visible line.
/// Returns Vec of (start_col, end_col, is_current) tuples.
pub fn search_highlights_for_line(&self, line_idx: usize) -> Vec<(usize, usize, bool)> {
    if self.search_state.matches.is_empty() {
        return Vec::new();
    }

    let line_start = self.buffer.line_to_char(line_idx);
    let line_end = line_start + self.buffer.line_len_chars(line_idx);
    let current_idx = self.search_state.current_match_index();

    self.search_state
        .matches
        .iter()
        .enumerate()
        .filter(|(_, &(start, end))| end > line_start && start < line_end)
        .map(|(idx, &(start, end))| {
            let col_start = if start > line_start {
                start - line_start
            } else {
                0
            };
            let col_end = if end < line_end {
                end - line_start
            } else {
                self.buffer.line_content_len(line_idx)
            };
            let is_current = current_idx == Some(idx);
            (col_start, col_end, is_current)
        })
        .collect()
}
```

- [ ] **Step 3: Render search highlights in editor_view.rs**

In the per-line rendering loop in `editor_view.rs`, after drawing background but before drawing text, add:

```rust
// Search match highlights
for (match_start_col, match_end_col, is_current) in
    editor.search_highlights_for_line(i)
{
    let match_x_start: f32 = if match_start_col == 0 {
        0.0
    } else {
        let prefix: String = display.chars().take(match_start_col).collect();
        painter
            .layout_no_wrap(prefix, font_id.clone(), theme.foreground)
            .rect
            .width()
    };
    let match_x_end: f32 = {
        let prefix: String = display.chars().take(match_end_col).collect();
        painter
            .layout_no_wrap(prefix, font_id.clone(), theme.foreground)
            .rect
            .width()
    };
    let match_rect = Rect::from_min_size(
        egui::pos2(text_x + match_x_start, y),
        Vec2::new(match_x_end - match_x_start, line_height),
    );
    let color = if is_current {
        theme.search_current
    } else {
        theme.search_match
    };
    painter.rect_filled(match_rect, 0.0, color);
}
```

Draw search highlights before selection highlights and text, so they layer correctly.

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add src/renderer/theme.rs src/renderer/editor_view.rs src/editor.rs
git commit -m "feat: add search match highlighting with current match indicator"
```

---

## Summary

| Task | Description | New Files | Key Tests |
|------|------------|-----------|-----------|
| 1 | Register system | `register.rs` | unnamed/named get/set, linewise flag |
| 2 | Wire registers into OperatorEngine | — | register-aware delete/yank/paste |
| 3 | Register prefix in KeyParser | — | `"a` prefix, take_register() |
| 4 | Text object Word/BigWord | `text_object.rs` | inner/around word, bigword, unicode |
| 5 | Text object quotes | — | inner/around `"` and `'`, edge cases |
| 6 | Text object brackets | — | inner/around `()/[]/{}`, nesting, multiline |
| 7 | Text object integration | — | diw, ci", ya( in keyparser + operator |
| 8 | Visual mode types | — | Mode enum, action types, is_visual() |
| 9 | Visual selection state | — | VisualAnchor, range resolution |
| 10 | Visual KeyParser + operators | — | v/V/d/c/y/o keybindings, Ctrl+V |
| 11 | Visual rendering | — | selection highlighting |
| 12 | Dot-repeat | — | operator repeat, insert session repeat |
| 13 | Search core | `search.rs` | find_matches, next/prev, wrap, unicode |
| 14 | Search input + integration | — | `/` `?` `n` `N` keybindings, input mode |
| 15 | Search rendering | — | match highlights, theme colors |
