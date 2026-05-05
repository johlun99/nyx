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
    _buffer: &TextBuffer,
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

fn resolve_inner_quote(_chars: &[char], _offset: usize, _quote: char) -> Option<(usize, usize)> {
    None // Implemented in Task 5
}

fn resolve_around_quote(_chars: &[char], _offset: usize, _quote: char) -> Option<(usize, usize)> {
    None // Implemented in Task 5
}

fn resolve_inner_bracket(
    _chars: &[char],
    _offset: usize,
    _open: char,
    _close: char,
) -> Option<(usize, usize)> {
    None // Implemented in Task 6
}

fn resolve_around_bracket(
    _chars: &[char],
    _offset: usize,
    _open: char,
    _close: char,
) -> Option<(usize, usize)> {
    None // Implemented in Task 6
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
