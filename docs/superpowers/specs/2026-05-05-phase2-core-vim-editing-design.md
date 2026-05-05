# Phase 2: Core Vim Editing — Design Spec

## Goal

Add the core vim editing features that make Nyx usable for real editing work: visual modes, text objects, registers, dot-repeat, and search.

## Approach

Bottom-up, register-first. Build shared foundations (registers, text objects) before the features that depend on them (visual modes, dot-repeat, search). Each layer builds cleanly on the previous.

## Scope

- Register system (unnamed `"`, named `a`–`z`, system clipboard `+`)
- Text objects (iw/aw, iW/aW, i"/a", i'/a', i(/a(, i[/a[, i{/a{)
- Visual modes (v, V, Ctrl+V) with operator support (d, c, y)
- Dot-repeat (`.` command)
- Basic search (`/` forward, `?` backward, `n`/`N` navigation, match highlighting)

## Out of Scope

- Regex search
- Macros (q recording)
- Marks
- Folding
- Multiple cursors

---

## 1. Register System

### New file: `src/vim/register.rs`

**Data structures:**

```rust
pub struct RegisterEntry {
    pub content: String,
    pub linewise: bool, // true = came from dd/yy/V-yank, affects paste behavior
}

pub struct RegisterFile {
    unnamed: RegisterEntry,
    named: HashMap<char, RegisterEntry>,
    // '+' register delegates to OS clipboard via `arboard` crate
}
```

**Behavior:**

- `RegisterFile::get(name: Option<char>) -> RegisterEntry`
  - `None` → unnamed register
  - `Some('+')` → read from system clipboard (via `arboard`). `linewise` = false for system clipboard reads.
  - `Some('a'..='z')` → named register
- `RegisterFile::set(name: Option<char>, content: String, linewise: bool)`
  - `None` → write to unnamed register
  - `Some('+')` → write to system clipboard via `arboard`
  - `Some('a'..='z')` → write to named register AND unnamed register
- All yank/delete operations write to the unnamed register by default
- If a register prefix was given (`"a`), write to that named register AND the unnamed register
- Paste reads from unnamed by default, or from the specified register if prefixed

**Dependency:** Add `arboard` crate to `Cargo.toml` for cross-platform clipboard access.

### Integration with OperatorEngine

- `OperatorEngine.clipboard: String` is replaced by a `RegisterFile`
- `OperatorEngine` gains a `pending_register: Option<char>` field set by the key parser
- All yank/delete operations route through `RegisterFile::set()`
- Paste routes through `RegisterFile::get()`

### KeyParser changes

- Recognize `"` followed by `a`–`z` or `+` as a register prefix
- Store as `pending_register: Option<char>` on the parser
- The register prefix is consumed by the next operator/yank/paste action
- Sequence: `"a` → pending_register = Some('a'), then `dd` → DeleteLine into register 'a'

---

## 2. Text Objects

### New file: `src/vim/text_object.rs`

**Data structures:**

```rust
pub enum TextObject {
    Inner(TextObjectKind),
    Around(TextObjectKind),
}

pub enum TextObjectKind {
    Word,        // w
    BigWord,     // W (whitespace-delimited)
    DoubleQuote, // "
    SingleQuote, // '
    Paren,       // ( or )
    Bracket,     // [ or ]
    Brace,       // { or }
}
```

**Resolution function:**

`resolve_text_object(buffer: &TextBuffer, obj: &TextObject) -> Option<(usize, usize)>`

Returns char range `[start, end)` or `None` if no matching object found.

**Word/BigWord:**
- Find word boundaries around cursor position
- `Inner` = just the word characters
- `Around` = word + trailing whitespace (or leading if at end of line)
- `Word` uses vim's word definition: a word is a sequence of alphanumeric/underscore chars, OR a sequence of other non-whitespace chars (punctuation). Boundaries are transitions between these categories.
- `BigWord` treats any non-whitespace sequence as a word (only whitespace is a delimiter)

**Quotes (`"`, `'`):**
- Scan current line for matching pair containing cursor
- `Inner` = characters between the quotes (exclusive)
- `Around` = including the quote characters themselves
- If cursor is on a quote character, use that as one boundary and find the match

**Brackets/Parens/Braces:**
- Scan outward from cursor for matching pair, respecting nesting
- `Inner` = characters between delimiters (exclusive)
- `Around` = including the delimiter characters
- Supports multi-line matching (not limited to current line)

### OperatorAction additions

```rust
pub enum OperatorAction {
    Delete(MotionKind),
    Change(MotionKind),
    DeleteLine,
    ChangeLine,
    YankLine,
    // New:
    DeleteTextObject(TextObject),
    ChangeTextObject(TextObject),
    YankTextObject(TextObject),
}
```

### KeyParser changes

- After `d`, `c`, or `y` (pending operator), recognize `i` or `a` followed by `w/W/"/'/(/)/[/]/{/}` as a text object
- Pending buffer grows to 3 chars: `d` → `di` → `di"` → `Operator(DeleteTextObject(Inner(DoubleQuote)))`
- `i` and `a` as text object prefixes are only recognized when there's already a pending operator (they don't conflict with insert-mode `i`/`a` in Normal mode, since those fire immediately without pending state)

---

## 3. Visual Modes

### Mode enum expansion

```rust
pub enum Mode {
    Normal,
    Insert,
    Command,
    Visual,      // v — character-wise selection
    VisualLine,  // V — line-wise selection
    VisualBlock, // Ctrl+V — block/column selection
}
```

`status_text()` returns `"VISUAL"`, `"V-LINE"`, `"V-BLOCK"` respectively.

### Selection state

Stored in Editor:

```rust
pub struct VisualAnchor {
    pub line: usize,
    pub col: usize,
}
```

- Set to current cursor position when entering visual mode
- Selection range = `(anchor, cursor)` — cursor movement changes selection
- No separate "selection end" needed

### Selection range resolution

- **Visual (v):** Char range from `min(anchor, cursor)` to `max(anchor, cursor)` inclusive (character-wise)
- **Visual-Line (V):** All lines from `min(anchor.line, cursor.line)` to `max(...)`, full lines included
- **Visual-Block (Ctrl+V):** Rectangle `(min_line, min_col)` to `(max_line, max_col)`

### Operators in visual mode

- `d` — delete selection, yank to register, return to Normal
- `c` — delete selection, yank to register, enter Insert
- `y` — yank selection to register, return to Normal, cursor moves to selection start
- Register prefix works in visual mode: `"ad` deletes selection into register `a`

### Rendering (`editor_view.rs`)

- Selection highlighted using `theme.selection` color as filled rects behind text
- **Visual:** Highlight char range, potentially spanning multiple lines
- **Visual-Line:** Highlight full line width for selected lines
- **Visual-Block:** Highlight rectangular column region across selected lines

### KeyParser changes

- `v` in Normal mode → `VimAction::EnterVisual(VisualKind::Char)`
- `V` in Normal mode → `VimAction::EnterVisual(VisualKind::Line)`
- Ctrl+V in Normal mode → `VimAction::EnterVisual(VisualKind::Block)`
- In visual modes: all motions move cursor (selection follows automatically)
- `d`, `c`, `y` in visual modes → operate on selection
- `o` in visual mode → swap anchor and cursor
- `Escape` → exit to Normal mode, clear selection
- Switching between visual modes (e.g. `v` then `V`) changes mode but keeps anchor

### VimAction additions

```rust
pub enum VimAction {
    // ... existing ...
    EnterVisual(VisualKind),
    VisualOperator(VisualOperatorAction),
    SwapVisualAnchor,
}

pub enum VisualKind {
    Char,
    Line,
    Block,
}

pub enum VisualOperatorAction {
    Delete,
    Change,
    Yank,
}
```

---

## 4. Dot-Repeat

### What gets recorded

- Operator actions: `dw`, `dd`, `ci"`, `x`, etc.
- Insert sessions: the entire sequence from `i`/`a`/`o`/etc → typed characters → `Escape`
- Visual operator actions: recorded as the equivalent normal-mode operation on the affected range
- **NOT recorded:** motions without edits, yanks (no buffer change), undo/redo, mode switches without edits

### Storage in Editor

```rust
pub struct LastAction {
    pub entry: Option<InsertEntry>,
    pub actions: Vec<VimAction>,
    pub count: usize,
}
```

### Recording mechanism

- When an operator fires (d/c/x), capture it as `LastAction { entry: None, actions: [the_action], count }`
- When entering Insert mode, start accumulating: store the `InsertEntry`, then collect all `InsertChar`/`DeleteCharBefore` actions, until `Escape` finalizes the recording
- Only the most recent editing action is stored (overwritten each time)

### Replay mechanism

- `.` in Normal mode → `VimAction::DotRepeat`
- Editor handles `DotRepeat`:
  - For operators: call `apply_action()` with the stored action
  - For insert sessions: `apply_action(EnterInsert(entry))` → replay each `InsertChar`/`DeleteCharBefore` → `apply_action(SwitchMode(Normal))`
- If `.` is pressed with a count (`5.`), the new count overrides the stored count
- During replay, recording is disabled (same pattern as undo/redo in History) to avoid overwriting `LastAction`

### KeyParser changes

- `.` in Normal mode → `VimAction::DotRepeat`

---

## 5. Search

### New file: `src/vim/search.rs`

**Data structures:**

```rust
pub struct SearchState {
    pub pattern: String,
    pub direction: SearchDirection,
    pub matches: Vec<(usize, usize)>, // (start_offset, end_offset) in char indices
    pub current_match: Option<usize>,  // index into matches vec
}

pub enum SearchDirection {
    Forward,
    Backward,
}
```

### Search flow

1. `/` or `?` enters search input mode (similar to command mode)
   - Status bar shows `/pattern` or `?pattern` as user types
   - `Enter` executes search
   - `Escape` cancels, clears highlights
2. On Enter: scan entire buffer for literal substring matches (case-sensitive), cache positions
3. Jump to nearest match in search direction from cursor
4. `n` — next match (same direction), wraps around
5. `N` — next match (opposite direction), wraps around

### Match invalidation

- When buffer is modified, clear `matches` cache
- On next `n`/`N`, re-search lazily using stored pattern

### Rendering

Add to Theme:
```rust
pub search_match: Color32,    // background for all matches
pub search_current: Color32,  // background for current/active match
```

In `editor_view.rs`: render match highlights as background rects behind text, similar to selection highlighting. Current match uses `search_current`, others use `search_match`.

### Status bar

- During search input: shows `/pattern` or `?pattern`
- After search: shows match info like `[3/12]` as status message

### KeyParser changes

- `/` in Normal mode → `VimAction::EnterSearch(SearchDirection::Forward)`
- `?` in Normal mode → `VimAction::EnterSearch(SearchDirection::Backward)`
- `n` in Normal mode → `VimAction::SearchNext`
- `N` in Normal mode → `VimAction::SearchPrev`

### VimAction additions

```rust
pub enum VimAction {
    // ... existing ...
    EnterSearch(SearchDirection),
    SearchNext,
    SearchPrev,
}
```

---

## File Summary

| File | Action | Purpose |
|------|--------|---------|
| `src/vim/register.rs` | Create | RegisterFile, RegisterEntry, system clipboard via arboard |
| `src/vim/text_object.rs` | Create | TextObject enum, resolve_text_object() |
| `src/vim/search.rs` | Create | SearchState, SearchDirection, match finding |
| `src/vim/action.rs` | Modify | Add TextObject operators, VisualOperator, DotRepeat, Search actions |
| `src/vim/mode.rs` | Modify | Add Visual, VisualLine, VisualBlock variants |
| `src/vim/keyparser.rs` | Modify | Register prefix, text object sequences, visual/search keys, `.` |
| `src/vim/operator.rs` | Modify | Replace clipboard with RegisterFile, add text object execution |
| `src/vim/motion.rs` | Modify | (minor) may need adjustments for visual mode cursor behavior |
| `src/vim/mod.rs` | Modify | Export new modules |
| `src/editor.rs` | Modify | VisualAnchor, LastAction, SearchState, apply_action expansion |
| `src/renderer/editor_view.rs` | Modify | Selection highlighting, search match highlighting |
| `src/renderer/theme.rs` | Modify | Add search_match, search_current colors |
| `src/renderer/status_bar.rs` | Modify | Visual mode labels, search input display, match count |
| `Cargo.toml` | Modify | Add arboard dependency |
