// src/editor.rs
use crate::buffer::TextBuffer;
use crate::vim::{KeyParser, VimAction, InsertEntry, Mode};
use crate::vim::motion::execute_motion;

pub struct Editor {
    pub buffer: TextBuffer,
    pub key_parser: KeyParser,
    pub file_path: Option<String>,
    pub should_quit: bool,
    pub status_message: Option<String>,
    pub command_input: String,
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
            command_input: String::new(),
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
            VimAction::Operator(_) | VimAction::Yank(_) | VimAction::Paste => {
                // Handled in Task 8
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
                self.buffer.set_cursor_with_mode(self.buffer.cursor_line(), new_col, true);
            }
            InsertEntry::EndOfLine => {
                let content_len = self.buffer.line_content_len(self.buffer.cursor_line());
                self.buffer.set_cursor_with_mode(self.buffer.cursor_line(), content_len, true);
            }
            InsertEntry::FirstNonBlank => {
                let line = self.buffer.line_slice(self.buffer.cursor_line()).to_string();
                let col = line.chars().take_while(|c| c.is_whitespace() && *c != '\n').count();
                self.buffer.set_cursor_with_mode(self.buffer.cursor_line(), col, true);
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

    // Command mode stubs (implemented in Task 9)
    pub fn execute_command(&mut self) {
        self.status_message = Some(format!("Unknown command: {}", self.command_input));
        self.command_input.clear();
        let action = self.key_parser.handle_escape();
        self.apply_action(action);
    }

    pub fn handle_command_backspace(&mut self) {
        self.command_input.pop();
        if self.command_input.is_empty() {
            let action = self.key_parser.handle_escape();
            self.apply_action(action);
        }
    }

    pub fn handle_command_char(&mut self, ch: char) {
        self.command_input.push(ch);
    }
}
