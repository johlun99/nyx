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
    let content_len = buffer.line_content_len(buffer.cursor_line());
    let content: Vec<char> = buffer.line_slice(buffer.cursor_line())
        .chars()
        .take(content_len)
        .collect();

    if content.is_empty() {
        return;
    }

    let mut col = buffer.cursor_col().min(content.len().saturating_sub(1));

    // Skip current word (whitespace-delimited, i.e. WORD motion)
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

    let content_len = buffer.line_content_len(buffer.cursor_line());
    let content: Vec<char> = buffer.line_slice(buffer.cursor_line())
        .chars()
        .take(content_len)
        .collect();

    if content.is_empty() {
        return;
    }

    let mut c = (col - 1).min(content.len().saturating_sub(1));

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
    let content_len = buffer.line_content_len(buffer.cursor_line());
    let content: Vec<char> = buffer.line_slice(buffer.cursor_line())
        .chars()
        .take(content_len)
        .collect();
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

    #[test]
    fn motion_word_backward_from_first_word_middle() {
        let mut buf = TextBuffer::from_text("hello world");
        buf.set_cursor(0, 2);
        execute_motion(&mut buf, &MotionKind::WordBackward);
        assert_eq!(buf.cursor_col(), 0);
    }

    #[test]
    fn motion_word_backward_from_whitespace() {
        let mut buf = TextBuffer::from_text("hello   world");
        buf.set_cursor(0, 6); // on whitespace between words
        execute_motion(&mut buf, &MotionKind::WordBackward);
        assert_eq!(buf.cursor_col(), 0); // back to start of 'hello'
    }

    #[test]
    fn motion_word_backward_at_line_start() {
        let mut buf = TextBuffer::from_text("hello\nworld");
        buf.set_cursor(1, 0);
        execute_motion(&mut buf, &MotionKind::WordBackward);
        assert_eq!(buf.cursor_line(), 0);
        assert_eq!(buf.cursor_col(), 4); // end of 'hello'
    }

    #[test]
    fn motion_word_forward_empty_line() {
        let mut buf = TextBuffer::from_text("\nhello");
        execute_motion(&mut buf, &MotionKind::WordForward);
        // Should not panic on empty line
        assert_eq!(buf.cursor_line(), 0);
    }

    #[test]
    fn motion_on_empty_buffer_word_motions() {
        let mut buf = TextBuffer::new();
        execute_motion(&mut buf, &MotionKind::WordForward);
        execute_motion(&mut buf, &MotionKind::WordBackward);
        execute_motion(&mut buf, &MotionKind::WordEnd);
        assert_eq!(buf.cursor_col(), 0);
    }
}
