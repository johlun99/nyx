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
