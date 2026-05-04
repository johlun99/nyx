// src/buffer/text_buffer.rs
use ropey::{Rope, RopeSlice};

pub struct TextBuffer {
    rope: Rope,
    cursor_line: usize,
    cursor_col: usize,
}

impl TextBuffer {
    pub fn new() -> Self {
        Self { rope: Rope::new(), cursor_line: 0, cursor_col: 0 }
    }

    pub fn from_text(text: &str) -> Self {
        Self { rope: Rope::from_str(text), cursor_line: 0, cursor_col: 0 }
    }

    pub fn cursor_line(&self) -> usize { self.cursor_line }
    pub fn cursor_col(&self) -> usize { self.cursor_col }

    pub fn set_cursor(&mut self, line: usize, col: usize) {
        self.set_cursor_with_mode(line, col, false);
    }

    pub fn set_cursor_with_mode(&mut self, line: usize, col: usize, allow_past_end: bool) {
        self.cursor_line = line.min(self.line_count().saturating_sub(1));
        let content_len = self.line_content_len(self.cursor_line);
        let max_col = if allow_past_end { content_len } else { content_len.saturating_sub(1) };
        self.cursor_col = col.min(max_col);
    }

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

    pub fn text(&self) -> String { self.rope.to_string() }

    pub fn slice(&self, start: usize, end: usize) -> String {
        self.rope.slice(start..end).to_string()
    }

    pub fn line_count(&self) -> usize { self.rope.len_lines() }

    pub fn line_slice(&self, idx: usize) -> RopeSlice<'_> { self.rope.line(idx) }

    pub fn line_content_len(&self, line_idx: usize) -> usize {
        let line = self.rope.line(line_idx);
        let len = line.len_chars();
        if len > 0 && line.char(len - 1) == '\n' { len - 1 } else { len }
    }

    pub fn line_len_chars(&self, line_idx: usize) -> usize {
        self.rope.line(line_idx).len_chars()
    }

    pub fn len_chars(&self) -> usize { self.rope.len_chars() }

    pub fn line_to_char(&self, line_idx: usize) -> usize {
        self.rope.line_to_char(line_idx)
    }

    pub fn insert_char(&mut self, ch: char) {
        let offset = self.cursor_offset();
        self.rope.insert_char(offset, ch);
        if ch == '\n' { self.cursor_line += 1; self.cursor_col = 0; }
        else { self.cursor_col += 1; }
    }

    pub fn delete_char_before_cursor(&mut self) {
        let offset = self.cursor_offset();
        if offset == 0 { return; }
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
        if offset >= self.rope.len_chars() { return; }
        self.rope.remove(offset..offset + 1);
    }

    pub fn delete_range(&mut self, start: usize, end: usize) {
        self.rope.remove(start..end);
    }

    pub fn insert_text_at(&mut self, offset: usize, text: &str) {
        self.rope.insert(offset, text);
    }
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
        buf.set_cursor_with_mode(0, 5, true);
        buf.insert_char('\n');
        assert_eq!(buf.text(), "hello\n");
        assert_eq!(buf.cursor_line(), 1);
        assert_eq!(buf.cursor_col(), 0);
    }

    #[test]
    fn delete_char_before_cursor() {
        let mut buf = TextBuffer::from_text("hello");
        buf.set_cursor_with_mode(0, 5, true);
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
        assert_eq!(buf.line_content_len(0), 5);
        assert_eq!(buf.line_content_len(1), 5);
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
        assert_eq!(buf.cursor_col(), 0);
    }

    #[test]
    fn empty_buffer_operations() {
        let mut buf = TextBuffer::new();
        assert_eq!(buf.line_count(), 1);
        buf.delete_char_before_cursor(); // should not panic
        buf.delete_char_at_cursor(); // should not panic
    }
}
