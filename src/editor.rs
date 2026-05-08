// src/editor.rs
use crate::buffer::TextBuffer;
use crate::syntax::indent::compute_indent;
use crate::syntax::languages::language_for_extension;
use crate::syntax::SyntaxState;
use crate::vim::action::SearchDirection;
use crate::vim::command::{CommandParser, CommandResult};
use crate::vim::motion::execute_motion;
use crate::vim::operator::OperatorEngine;
use crate::vim::search::SearchState;
use crate::vim::{InsertEntry, KeyParser, Mode, VimAction, VisualOperatorAction};

#[derive(Debug, Clone)]
pub struct LastAction {
    pub entry: Option<InsertEntry>,
    pub actions: Vec<VimAction>,
    pub count: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct VisualAnchor {
    pub line: usize,
    pub col: usize,
}

/// A position in the jump list.
#[derive(Debug, Clone, Copy)]
pub struct JumpPosition {
    pub line: usize,
    pub col: usize,
}

/// Vim-style jump list: records cursor positions before big jumps (gd, gr, gg, G, searches).
pub struct JumpList {
    entries: Vec<JumpPosition>,
    cursor: usize, // points *past* the last pushed entry; back goes to cursor-1
}

impl JumpList {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            cursor: 0,
        }
    }

    /// Record current position before a jump.
    pub fn push(&mut self, line: usize, col: usize) {
        // Truncate any forward history
        self.entries.truncate(self.cursor);
        self.entries.push(JumpPosition { line, col });
        self.cursor = self.entries.len();
    }

    /// Go back (Ctrl+O). Returns the position to jump to.
    pub fn go_back(&mut self) -> Option<JumpPosition> {
        if self.cursor > 0 {
            self.cursor -= 1;
            Some(self.entries[self.cursor])
        } else {
            None
        }
    }

    /// Go forward (Ctrl+I). Returns the position to jump to.
    pub fn go_forward(&mut self) -> Option<JumpPosition> {
        if self.cursor < self.entries.len() {
            let pos = self.entries[self.cursor];
            self.cursor += 1;
            Some(pos)
        } else {
            None
        }
    }
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
    pub last_action: Option<LastAction>,
    pub search_state: SearchState,
    pub search_input: Option<String>,
    recording_action: Option<LastAction>,
    replaying: bool,
    pub syntax_state: Option<SyntaxState>,
    pub tab_size: usize,
    pub jump_list: JumpList,
    last_saved_text: String,
    pending_did_save: bool,
}

impl Editor {
    pub fn new(file_path: Option<String>) -> Self {
        let buffer = if let Some(ref path) = file_path {
            match crate::file_io::read_file(std::path::Path::new(path)) {
                Ok(content) => TextBuffer::from_text(&content),
                Err(e) => {
                    tracing::warn!("Could not read {}: {}", path, e);
                    TextBuffer::new()
                }
            }
        } else {
            TextBuffer::from_text(
                "Welcome to Nyx!\n\nPress i to enter insert mode.\nPress : for commands.\nPress :q to quit.\n"
            )
        };

        let syntax_state = if let Some(ref path) = file_path {
            let ext = std::path::Path::new(path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            language_for_extension(ext).and_then(|lang_name| {
                let mut state = SyntaxState::new(lang_name, ext)?;
                state.parse(&buffer.text());
                Some(state)
            })
        } else {
            None
        };
        let last_saved_text = buffer.text();

        Self {
            buffer,
            key_parser: KeyParser::new(),
            operator_engine: OperatorEngine::new(),
            file_path,
            should_quit: false,
            status_message: None,
            command_parser: CommandParser::new(),
            visual_anchor: None,
            last_action: None,
            search_state: SearchState::new(),
            search_input: None,
            recording_action: None,
            replaying: false,
            syntax_state,
            tab_size: 4,
            jump_list: JumpList::new(),
            last_saved_text,
            pending_did_save: false,
        }
    }

    pub fn mode(&self) -> Mode {
        self.key_parser.mode()
    }

    pub fn set_tab_size(&mut self, size: usize) {
        self.tab_size = size;
    }

    /// Check if backspace should remove a full tab-width of spaces.
    fn should_dedent(&self) -> bool {
        let col = self.buffer.cursor_col();
        if col < self.tab_size || !col.is_multiple_of(self.tab_size) {
            return false;
        }
        let line = self.buffer.cursor_line();
        let line_text = self.buffer.line_slice(line).to_string();
        line_text[..col].chars().all(|c| c == ' ')
    }

    pub fn apply_action(&mut self, action: VimAction) {
        if action == VimAction::Noop {
            return;
        }

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
                if let Some(ref mut ss) = self.syntax_state {
                    ss.mark_dirty();
                }
            }
            _ => {}
        }

        self.status_message = None;

        let count = self.key_parser.take_count();
        let register = self.key_parser.take_register();

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

        match action {
            VimAction::SwitchMode(Mode::Normal) => {
                self.visual_anchor = None;
                let was_insert = self.buffer.end_undo_group();
                if was_insert {
                    let col = self.buffer.cursor_col();
                    if col > 0 {
                        self.buffer.set_cursor(self.buffer.cursor_line(), col - 1);
                    }
                }
                self.buffer.clamp_cursor_normal();
            }
            VimAction::SwitchMode(_) => {}
            VimAction::Motion(ref motion) => {
                for _ in 0..count {
                    execute_motion(&mut self.buffer, motion);
                }
            }
            VimAction::InsertChar(ch) => {
                if ch == '\n' {
                    let current_line = self.buffer.cursor_line();
                    self.buffer.insert_char('\n');
                    let indent = compute_indent(
                        &self.buffer,
                        self.syntax_state.as_ref(),
                        current_line,
                        self.tab_size,
                    );
                    if indent > 0 {
                        let indent_str: String = " ".repeat(indent);
                        let offset = self.buffer.cursor_offset();
                        self.buffer.insert_text_at(offset, &indent_str);
                        self.buffer
                            .set_cursor_with_mode(self.buffer.cursor_line(), indent, true);
                    }
                } else {
                    self.buffer.insert_char(ch);
                }
            }
            VimAction::DeleteCharBefore => {
                if self.mode() == Mode::Insert && self.should_dedent() {
                    for _ in 0..self.tab_size {
                        self.buffer.delete_char_before_cursor();
                    }
                } else {
                    self.buffer.delete_char_before_cursor();
                }
            }
            VimAction::EnterInsert(entry) => {
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
            VimAction::Operator(ref op_action) => {
                for _ in 0..count {
                    self.operator_engine
                        .execute(&mut self.buffer, op_action, register);
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
            VimAction::EnterVisual(ref _kind) => {
                self.visual_anchor = Some(VisualAnchor {
                    line: self.buffer.cursor_line(),
                    col: self.buffer.cursor_col(),
                });
            }
            VimAction::VisualOperator(ref vis_op) => {
                if let Some((start, end)) = self.visual_selection_range() {
                    let content = self.buffer.slice(start, end);
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
            VimAction::SwapVisualAnchor => {
                if let Some(ref mut anchor) = self.visual_anchor {
                    let old_anchor_line = anchor.line;
                    let old_anchor_col = anchor.col;
                    anchor.line = self.buffer.cursor_line();
                    anchor.col = self.buffer.cursor_col();
                    self.buffer.set_cursor(old_anchor_line, old_anchor_col);
                }
            }
            VimAction::DotRepeat => {
                if let Some(ref last) = self.last_action.clone() {
                    self.replaying = true;
                    let repeat_count = if count > 1 { count } else { last.count };

                    if let Some(ref entry) = last.entry.clone() {
                        // Insert session replay
                        for _ in 0..repeat_count {
                            self.apply_action(VimAction::EnterInsert(entry.clone()));
                            for a in last.actions.clone() {
                                self.apply_action(a);
                            }
                            self.apply_action(VimAction::SwitchMode(Mode::Normal));
                        }
                    } else {
                        // Operator replay
                        for a in last.actions.clone() {
                            for _ in 0..repeat_count {
                                self.apply_action(a.clone());
                            }
                        }
                    }
                    self.replaying = false;
                }
            }
            VimAction::EnterSearch(ref direction) => {
                self.start_search(direction.clone());
            }
            VimAction::SearchNext => {
                if !self.search_state.pattern.is_empty() {
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
            // LSP actions are handled in app.rs, not here
            VimAction::LspGotoDefinition | VimAction::LspReferences | VimAction::LspHover => {}
            VimAction::Noop => unreachable!(),
        }
    }

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
                // For block mode, return bounding line range for highlighting
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

    /// Get visual highlight ranges for the renderer.
    /// Returns (start_col, end_col) for highlighting on a given line.
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

                let col_start = sel_start.saturating_sub(line_start);
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

    /// Returns (start_line, end_line, start_col, end_col) for block selection.
    #[allow(dead_code)]
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

    fn handle_insert_entry(&mut self, entry: InsertEntry) {
        match entry {
            InsertEntry::AtCursor => {}
            InsertEntry::AfterCursor => {
                let content_len = self.buffer.line_content_len(self.buffer.cursor_line());
                let new_col = (self.buffer.cursor_col() + 1).min(content_len);
                self.buffer
                    .set_cursor_with_mode(self.buffer.cursor_line(), new_col, true);
            }
            InsertEntry::EndOfLine => {
                let content_len = self.buffer.line_content_len(self.buffer.cursor_line());
                self.buffer
                    .set_cursor_with_mode(self.buffer.cursor_line(), content_len, true);
            }
            InsertEntry::FirstNonBlank => {
                let line = self
                    .buffer
                    .line_slice(self.buffer.cursor_line())
                    .to_string();
                let col = line
                    .chars()
                    .take_while(|c| c.is_whitespace() && *c != '\n')
                    .count();
                self.buffer
                    .set_cursor_with_mode(self.buffer.cursor_line(), col, true);
            }
            InsertEntry::NewLineBelow => {
                let line = self.buffer.cursor_line();
                let indent = compute_indent(
                    &self.buffer,
                    self.syntax_state.as_ref(),
                    line,
                    self.tab_size,
                );
                // Insert after the content of the current line (before its trailing \n if any)
                let insert_offset =
                    self.buffer.line_to_char(line) + self.buffer.line_content_len(line);
                let indent_str: String = " ".repeat(indent);
                self.buffer
                    .insert_text_at(insert_offset, &format!("\n{}", indent_str));
                self.buffer.set_cursor_with_mode(line + 1, indent, true);
            }
            InsertEntry::NewLineAbove => {
                let line = self.buffer.cursor_line();
                let indent = compute_indent(
                    &self.buffer,
                    self.syntax_state.as_ref(),
                    line,
                    self.tab_size,
                );
                let line_start = self.buffer.line_to_char(line);
                let indent_str: String = " ".repeat(indent);
                self.buffer
                    .insert_text_at(line_start, &format!("{}\n", indent_str));
                self.buffer.set_cursor_with_mode(line, indent, true);
            }
        }
    }

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
        if self.command_parser.input.is_empty() {
            let action = self.key_parser.handle_escape();
            self.apply_action(action);
        }
    }

    /// Execute the current command and return the result.
    /// Returns `Some(rename_name)` if the command was `:rename`.
    pub fn execute_command(&mut self) -> Option<String> {
        let result = self.command_parser.execute();
        let mut message_to_keep: Option<String> = None;
        let mut rename_name: Option<String> = None;
        match result {
            CommandResult::Quit => {
                if self.has_unsaved_changes() {
                    message_to_keep =
                        Some("No write since last change (add ! to override: :q!)".to_string());
                } else {
                    self.should_quit = true;
                }
            }
            CommandResult::ForceQuit => {
                self.should_quit = true;
            }
            CommandResult::Write => {
                self.save_file();
            }
            CommandResult::WriteQuit => {
                if self.save_file() {
                    self.should_quit = true;
                }
            }
            CommandResult::Rename(name) => {
                rename_name = Some(name);
            }
            CommandResult::Unknown(cmd) => {
                message_to_keep = Some(format!("Unknown command: {}", cmd));
            }
        }
        self.command_parser.clear();
        let action = self.key_parser.handle_escape();
        self.apply_action(action);
        if let Some(message) = message_to_keep {
            self.status_message = Some(message);
        }
        rename_name
    }

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
                let col_start = start.saturating_sub(line_start);
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

    /// Re-parse syntax tree if dirty. Call before rendering.
    pub fn ensure_syntax_parsed(&mut self) {
        if let Some(ref mut ss) = self.syntax_state {
            ss.ensure_parsed(&self.buffer.text());
        }
    }

    /// Get syntax highlight spans for a line.
    /// Returns Vec of (col_start, col_end, color) in char offsets.
    pub fn syntax_highlights_for_line(
        &self,
        line_idx: usize,
        theme: &crate::renderer::theme::Theme,
    ) -> Vec<(usize, usize, eframe::egui::Color32)> {
        match self.syntax_state {
            Some(ref ss) => {
                crate::syntax::highlighter::highlights_for_line(ss, &self.buffer, line_idx, theme)
            }
            None => Vec::new(),
        }
    }

    fn save_file(&mut self) -> bool {
        if let Some(ref path) = self.file_path {
            match crate::file_io::write_file(std::path::Path::new(path), &self.buffer.text()) {
                Ok(_) => {
                    self.status_message = Some(format!("Written: {}", path));
                    self.last_saved_text = self.buffer.text();
                    self.pending_did_save = true;
                    tracing::info!("File saved: {}", path);
                    true
                }
                Err(e) => {
                    self.status_message = Some(format!("Error writing {}: {}", path, e));
                    tracing::error!("Failed to save {}: {}", path, e);
                    false
                }
            }
        } else {
            self.status_message = Some("No file path".to_string());
            false
        }
    }

    fn has_unsaved_changes(&self) -> bool {
        self.buffer.text() != self.last_saved_text
    }

    pub fn take_did_save_event(&mut self) -> bool {
        let had = self.pending_did_save;
        self.pending_did_save = false;
        had
    }
}

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

    #[test]
    fn editor_creates_syntax_state_for_known_extension() {
        let tmp = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
        std::fs::write(tmp.path(), "fn main() {}").unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        let editor = Editor::new(Some(path));
        assert!(editor.syntax_state.is_some());
    }

    #[test]
    fn editor_no_syntax_state_for_unknown_extension() {
        let tmp = tempfile::NamedTempFile::with_suffix(".xyz").unwrap();
        std::fs::write(tmp.path(), "hello").unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        let editor = Editor::new(Some(path));
        assert!(editor.syntax_state.is_none());
    }

    #[test]
    fn new_line_below_copies_indent() {
        let mut editor = Editor::new(None);
        editor.buffer = TextBuffer::from_text("    hello\nworld");
        editor.buffer.set_cursor(0, 4);

        editor.set_tab_size(4);
        editor.apply_action(VimAction::EnterInsert(InsertEntry::NewLineBelow));

        // New line should be inserted after line 0 with 4 spaces indent
        assert_eq!(editor.buffer.cursor_line(), 1);
        let line = editor.buffer.line_slice(1).to_string();
        assert!(
            line.starts_with("    "),
            "Expected 4 spaces indent, got: {:?}",
            line
        );
        assert_eq!(editor.buffer.cursor_col(), 4);
    }

    #[test]
    fn new_line_above_copies_indent() {
        let mut editor = Editor::new(None);
        editor.buffer = TextBuffer::from_text("    hello\nworld");
        editor.buffer.set_cursor(0, 4);

        editor.set_tab_size(4);
        editor.apply_action(VimAction::EnterInsert(InsertEntry::NewLineAbove));

        // New line should be inserted above line 0 with 4 spaces indent
        assert_eq!(editor.buffer.cursor_line(), 0);
        let line = editor.buffer.line_slice(0).to_string();
        assert!(
            line.starts_with("    "),
            "Expected 4 spaces indent, got: {:?}",
            line
        );
        assert_eq!(editor.buffer.cursor_col(), 4);
    }

    #[test]
    fn enter_in_insert_mode_copies_indent() {
        let mut editor = Editor::new(None);
        editor.buffer = TextBuffer::from_text("    hello");
        editor.buffer.set_cursor_with_mode(0, 9, true); // end of "    hello"

        editor.set_tab_size(4);
        editor.buffer.begin_undo_group();
        editor.apply_action(VimAction::InsertChar('\n'));

        assert_eq!(editor.buffer.cursor_line(), 1);
        assert_eq!(editor.buffer.cursor_col(), 4); // indented to match line above
    }

    #[test]
    fn syntax_highlights_work_for_new_file() {
        // Simulates: cargo run -- newfile.py (file doesn't exist)
        // Then typing "def foo():" in insert mode
        let mut editor = Editor::new(Some("nonexistent.py".to_string()));
        assert!(
            editor.syntax_state.is_some(),
            "Should create syntax state from extension even if file doesn't exist"
        );

        // Type "def foo():" character by character
        for ch in "def foo():".chars() {
            editor.apply_action(VimAction::InsertChar(ch));
        }

        // Simulate what app.rs does each frame
        editor.ensure_syntax_parsed();

        let theme = crate::renderer::theme::Theme::default_dark();
        let spans = editor.syntax_highlights_for_line(0, &theme);
        assert!(
            !spans.is_empty(),
            "Expected syntax highlights after typing into new .py file"
        );
        // "def" should be a keyword
        let has_keyword = spans
            .iter()
            .any(|&(start, end, color)| start == 0 && end == 3 && color == theme.syntax.keyword);
        assert!(has_keyword, "Expected 'def' highlighted as keyword");
    }

    #[test]
    fn enter_after_python_colon_indents() {
        let tmp = tempfile::NamedTempFile::with_suffix(".py").unwrap();
        std::fs::write(tmp.path(), "def foo():").unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        let mut editor = Editor::new(Some(path));

        // Cursor at end of "def foo():"
        editor.buffer.set_cursor_with_mode(0, 10, true);
        editor.set_tab_size(4);
        editor.buffer.begin_undo_group();
        editor.apply_action(VimAction::InsertChar('\n'));

        assert_eq!(editor.buffer.cursor_line(), 1);
        assert_eq!(
            editor.buffer.cursor_col(),
            4,
            "Expected 4-space indent after colon, got {}. Line content: {:?}",
            editor.buffer.cursor_col(),
            editor.buffer.line_slice(1).to_string()
        );
    }

    #[test]
    fn quit_is_blocked_when_unsaved_changes_exist() {
        let tmp = tempfile::NamedTempFile::with_suffix(".txt").unwrap();
        std::fs::write(tmp.path(), "hello\n").unwrap();
        let mut editor = Editor::new(Some(tmp.path().to_string_lossy().to_string()));

        editor.buffer.set_cursor_with_mode(0, 5, true);
        editor.buffer.insert_char('!');
        editor.command_parser.input = "q".to_string();
        editor.execute_command();

        assert!(!editor.should_quit);
        assert_eq!(
            editor.status_message.as_deref(),
            Some("No write since last change (add ! to override: :q!)")
        );
    }

    #[test]
    fn force_quit_ignores_unsaved_changes() {
        let tmp = tempfile::NamedTempFile::with_suffix(".txt").unwrap();
        std::fs::write(tmp.path(), "hello\n").unwrap();
        let mut editor = Editor::new(Some(tmp.path().to_string_lossy().to_string()));

        editor.buffer.set_cursor_with_mode(0, 5, true);
        editor.buffer.insert_char('!');
        editor.command_parser.input = "q!".to_string();
        editor.execute_command();

        assert!(editor.should_quit);
    }
}
