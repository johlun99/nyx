use crate::buffer::TextBuffer;

/// Compute indent level (number of spaces) for a new line.
/// `reference_line` is the line to copy indent from.
/// Returns the number of leading spaces on the reference line.
pub fn copy_indent(buffer: &TextBuffer, reference_line: usize) -> usize {
    if reference_line >= buffer.line_count() {
        return 0;
    }
    let line = buffer.line_slice(reference_line);
    line.chars()
        .take_while(|c| c.is_whitespace() && *c != '\n')
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_indent_no_indent() {
        let buffer = TextBuffer::from_text("hello\nworld");
        assert_eq!(copy_indent(&buffer, 0), 0);
    }

    #[test]
    fn copy_indent_with_spaces() {
        let buffer = TextBuffer::from_text("    hello\nworld");
        assert_eq!(copy_indent(&buffer, 0), 4);
    }

    #[test]
    fn copy_indent_with_tabs() {
        let buffer = TextBuffer::from_text("\thello\nworld");
        // Tabs count as 1 character of leading whitespace
        assert_eq!(copy_indent(&buffer, 0), 1);
    }

    #[test]
    fn copy_indent_mixed_whitespace() {
        let buffer = TextBuffer::from_text("  \t  hello\nworld");
        assert_eq!(copy_indent(&buffer, 0), 5);
    }

    #[test]
    fn copy_indent_empty_line() {
        let buffer = TextBuffer::from_text("\nhello");
        assert_eq!(copy_indent(&buffer, 0), 0);
    }

    #[test]
    fn copy_indent_blank_line() {
        let buffer = TextBuffer::from_text("    \nhello");
        assert_eq!(copy_indent(&buffer, 0), 4);
    }

    #[test]
    fn copy_indent_last_line() {
        let buffer = TextBuffer::from_text("hello\n    world");
        assert_eq!(copy_indent(&buffer, 1), 4);
    }
}
