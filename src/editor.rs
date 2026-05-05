// src/editor.rs
use crate::buffer::TextBuffer;
use crate::vim::command::{CommandParser, CommandResult};
use crate::vim::motion::execute_motion;
use crate::vim::operator::OperatorEngine;
use crate::vim::{InsertEntry, KeyParser, Mode, VimAction};

pub struct Editor {
    pub buffer: TextBuffer,
    pub key_parser: KeyParser,
    pub operator_engine: OperatorEngine,
    pub file_path: Option<String>,
    pub should_quit: bool,
    pub status_message: Option<String>,
    pub command_parser: CommandParser,
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

        Self {
            buffer,
            key_parser: KeyParser::new(),
            operator_engine: OperatorEngine::new(),
            file_path,
            should_quit: false,
            status_message: None,
            command_parser: CommandParser::new(),
        }
    }

    pub fn mode(&self) -> Mode {
        self.key_parser.mode()
    }

    pub fn apply_action(&mut self, action: VimAction) {
        if action == VimAction::Noop {
            return;
        }
        self.status_message = None;

        let count = self.key_parser.take_count();
        match action {
            VimAction::SwitchMode(Mode::Normal) => {
                self.buffer.end_undo_group();
                let col = self.buffer.cursor_col();
                if col > 0 {
                    self.buffer.set_cursor(self.buffer.cursor_line(), col - 1);
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
                self.buffer.insert_char(ch);
            }
            VimAction::DeleteCharBefore => {
                self.buffer.delete_char_before_cursor();
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
            VimAction::Noop => unreachable!(),
        }
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
                let next_line_offset = if line + 1 < self.buffer.line_count() {
                    self.buffer.line_to_char(line + 1)
                } else {
                    self.buffer.len_chars()
                };
                self.buffer.insert_text_at(next_line_offset, "\n");
                self.buffer.set_cursor(line + 1, 0);
            }
            InsertEntry::NewLineAbove => {
                let line = self.buffer.cursor_line();
                let line_start = self.buffer.line_to_char(line);
                self.buffer.insert_text_at(line_start, "\n");
                self.buffer.set_cursor(line, 0);
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
        let action = self.key_parser.handle_escape();
        self.apply_action(action);
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
