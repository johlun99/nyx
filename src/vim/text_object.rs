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

    // Strategy: find the nearest quote at or before the cursor (open candidate),
    // then find the next quote after it (close candidate). If the cursor sits
    // between them, that is the enclosing pair.
    //
    // Walk backward from the cursor to find the opening quote, then forward
    // from the position after it to find the closing quote. This naturally
    // handles lines with multiple independent pairs (e.g. "it's 'fine' now")
    // by anchoring on the quote nearest to — and to the left of — the cursor.

    // Find opening quote: rightmost quote at or before cursor position
    let open_in_line = (0..=cursor_in_line)
        .rev()
        .find(|&i| line_chars[i] == quote)?;

    // Find closing quote: leftmost quote strictly after open
    let close_in_line = (open_in_line + 1..line_chars.len())
        .find(|&i| line_chars[i] == quote)?;

    // Cursor must be within [open, close] (inclusive) to be considered inside
    if cursor_in_line > close_in_line {
        return None;
    }

    Some((line_start + open_in_line + 1, line_start + close_in_line))
}

fn resolve_around_quote(chars: &[char], offset: usize, quote: char) -> Option<(usize, usize)> {
    let inner = resolve_inner_quote(chars, offset, quote)?;
    // Around includes the quotes themselves (one char before inner start, one after inner end)
    Some((inner.0 - 1, inner.1 + 1))
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
}
