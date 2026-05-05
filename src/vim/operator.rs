use crate::buffer::TextBuffer;
use crate::vim::action::{MotionKind, OperatorAction};
use crate::vim::motion::execute_motion;
use crate::vim::register::RegisterFile;
use crate::vim::text_object::resolve_text_object;

#[derive(Default)]
pub struct OperatorEngine {
    pub registers: RegisterFile,
}

impl OperatorEngine {
    pub fn new() -> Self {
        Self {
            registers: RegisterFile::new(),
        }
    }

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
                    buffer
                        .cursor_line()
                        .min(buffer.line_count().saturating_sub(1)),
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
        }
    }

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

    pub fn paste(&mut self, buffer: &mut TextBuffer, register: Option<char>) {
        let entry = self.registers.get_mut(register);
        if entry.content.is_empty() {
            return;
        }
        if entry.linewise {
            // Line paste: below current line
            let line = buffer.cursor_line();
            let line_start = buffer.cursor_offset() - buffer.cursor_col();
            let line_char_len = buffer.line_len_chars(line);
            buffer.insert_text_at(line_start + line_char_len, &entry.content);
            buffer.set_cursor(line + 1, 0);
        } else {
            // Inline paste: after cursor
            let offset = (buffer.cursor_offset() + 1).min(buffer.len_chars());
            buffer.insert_text_at(offset, &entry.content);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vim::text_object::{TextObject, TextObjectKind};

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
        engine.execute(
            &mut buf,
            &OperatorAction::Delete(MotionKind::WordForward),
            None,
        );
        assert_eq!(buf.text(), "world");
    }

    #[test]
    fn yank_line() {
        let mut buf = TextBuffer::from_text("hello\nworld");
        let mut engine = OperatorEngine::new();
        engine.execute(&mut buf, &OperatorAction::YankLine, None);
        assert_eq!(engine.registers.get(None).content, "hello\n");
        assert_eq!(buf.text(), "hello\nworld"); // unchanged
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
        engine.execute(
            &mut buf,
            &OperatorAction::Delete(MotionKind::WordForward),
            None,
        );
        assert_eq!(buf.text(), "världen");
    }

    #[test]
    fn change_word() {
        let mut buf = TextBuffer::from_text("hello world");
        let mut engine = OperatorEngine::new();
        engine.execute(
            &mut buf,
            &OperatorAction::Change(MotionKind::WordForward),
            None,
        );
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
        engine.execute(
            &mut buf,
            &OperatorAction::Delete(MotionKind::WordForward),
            None,
        );
        assert_eq!(buf.text(), "world");
        engine.paste(&mut buf, None);
        // Inline paste inserts after cursor (offset 0+1=1)
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
}
