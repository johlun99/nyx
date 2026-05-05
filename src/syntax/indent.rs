use super::SyntaxState;
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

/// Compute indent level for a new line being created after `reference_line`.
/// If syntax state is available, analyzes the parse tree.
/// Falls back to copy-indent when no syntax state or on parse errors.
/// Returns the number of spaces for the new line.
pub fn compute_indent(
    buffer: &TextBuffer,
    syntax_state: Option<&SyntaxState>,
    reference_line: usize,
    tab_size: usize,
) -> usize {
    match syntax_state {
        Some(state) if state.tree().is_some() => {
            treesitter_indent(buffer, state, reference_line, tab_size)
        }
        _ => copy_indent(buffer, reference_line),
    }
}

fn treesitter_indent(
    buffer: &TextBuffer,
    _state: &SyntaxState,
    reference_line: usize,
    tab_size: usize,
) -> usize {
    let base_indent = copy_indent(buffer, reference_line);
    let line = buffer.line_slice(reference_line).to_string();
    let trimmed = line.trim();

    // Count unmatched openers on the reference line
    let mut opener_count: i32 = 0;
    for ch in trimmed.chars() {
        match ch {
            '{' | '(' | '[' => opener_count += 1,
            '}' | ')' | ']' => opener_count -= 1,
            _ => {}
        }
    }

    if opener_count > 0 {
        // Line has unmatched openers — increase indent
        base_indent + tab_size
    } else if trimmed.starts_with('}') || trimmed.starts_with(')') || trimmed.starts_with(']') {
        // Line starts with a closer — decrease indent
        base_indent.saturating_sub(tab_size)
    } else {
        base_indent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::SyntaxState;

    #[test]
    fn compute_indent_without_syntax_falls_back_to_copy() {
        let buffer = TextBuffer::from_text("    hello\n");
        // No syntax state → copy-indent from line 0
        assert_eq!(compute_indent(&buffer, None, 0, 4), 4);
    }

    #[test]
    fn compute_indent_after_open_brace() {
        let source = "fn main() {\n";
        let buffer = TextBuffer::from_text(source);
        let mut state = SyntaxState::new("rust", "rs").unwrap();
        state.parse(source);
        // After "{" on line 0, new line should be indented by tab_size
        let indent = compute_indent(&buffer, Some(&state), 0, 4);
        assert_eq!(indent, 4);
    }

    #[test]
    fn compute_indent_after_close_brace() {
        let source = "fn main() {\n    let x = 1;\n}\n";
        let buffer = TextBuffer::from_text(source);
        let mut state = SyntaxState::new("rust", "rs").unwrap();
        state.parse(source);
        // Line 2 is "}" — indenting after it should return to 0
        let indent = compute_indent(&buffer, Some(&state), 2, 4);
        assert_eq!(indent, 0);
    }

    #[test]
    fn compute_indent_nested_braces() {
        let source = "fn main() {\n    if true {\n";
        let buffer = TextBuffer::from_text(source);
        let mut state = SyntaxState::new("rust", "rs").unwrap();
        state.parse(source);
        // After second "{" on line 1, should be 8 spaces
        let indent = compute_indent(&buffer, Some(&state), 1, 4);
        assert_eq!(indent, 8);
    }

    #[test]
    fn compute_indent_after_open_paren() {
        let source = "fn foo(\n";
        let buffer = TextBuffer::from_text(source);
        let mut state = SyntaxState::new("rust", "rs").unwrap();
        state.parse(source);
        let indent = compute_indent(&buffer, Some(&state), 0, 4);
        assert_eq!(indent, 4);
    }

    #[test]
    fn compute_indent_after_open_bracket() {
        let source = "let x = [\n";
        let buffer = TextBuffer::from_text(source);
        let mut state = SyntaxState::new("rust", "rs").unwrap();
        state.parse(source);
        let indent = compute_indent(&buffer, Some(&state), 0, 4);
        assert_eq!(indent, 4);
    }

    #[test]
    fn compute_indent_plain_line_copies_indent() {
        let source = "fn main() {\n    let x = 1;\n";
        let buffer = TextBuffer::from_text(source);
        let mut state = SyntaxState::new("rust", "rs").unwrap();
        state.parse(source);
        // Line 1 has "    let x = 1;" — no opener, so copy indent (4)
        let indent = compute_indent(&buffer, Some(&state), 1, 4);
        assert_eq!(indent, 4);
    }

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
