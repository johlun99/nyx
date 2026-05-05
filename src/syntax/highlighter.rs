use crate::buffer::TextBuffer;
use crate::renderer::theme::Theme;
use eframe::egui::Color32;

use super::SyntaxState;

/// Map a tree-sitter capture name to a theme color.
pub fn capture_to_color(name: &str, theme: &Theme) -> Color32 {
    match name {
        "keyword" => theme.syntax.keyword,
        "string" => theme.syntax.string,
        "comment" => theme.syntax.comment,
        "function" => theme.syntax.function,
        "type" => theme.syntax.r#type,
        "number" => theme.syntax.number,
        "operator" => theme.syntax.operator,
        "punctuation" => theme.syntax.punctuation,
        _ => theme.foreground,
    }
}

/// Get highlight spans for a single line.
/// Returns Vec of (col_start, col_end, color) in char offsets within the line.
pub fn highlights_for_line(
    syntax_state: &SyntaxState,
    buffer: &TextBuffer,
    line_idx: usize,
    theme: &Theme,
) -> Vec<(usize, usize, Color32)> {
    if line_idx >= buffer.line_count() {
        return Vec::new();
    }

    let line_start_byte = buffer.line_to_byte(line_idx);
    let line_end_byte = if line_idx + 1 < buffer.line_count() {
        buffer.line_to_byte(line_idx + 1)
    } else {
        buffer.len_bytes()
    };
    let line_start_char = buffer.line_to_char(line_idx);

    syntax_state
        .highlights()
        .iter()
        .filter(|(start, end, _)| *end > line_start_byte && *start < line_end_byte)
        .map(|(start, end, name)| {
            let clamped_start = (*start).max(line_start_byte);
            let clamped_end = (*end).min(line_end_byte);
            let col_start = buffer.byte_to_char(clamped_start) - line_start_char;
            let col_end = buffer.byte_to_char(clamped_end) - line_start_char;
            let color = capture_to_color(name, theme);
            (col_start, col_end, color)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> Theme {
        Theme::default_dark()
    }

    #[test]
    fn capture_to_color_maps_keyword() {
        let t = theme();
        assert_eq!(capture_to_color("keyword", &t), t.syntax.keyword);
    }

    #[test]
    fn capture_to_color_maps_string() {
        let t = theme();
        assert_eq!(capture_to_color("string", &t), t.syntax.string);
    }

    #[test]
    fn capture_to_color_unknown_falls_back_to_foreground() {
        let t = theme();
        assert_eq!(capture_to_color("unknown_capture", &t), t.foreground);
    }

    #[test]
    fn highlights_for_line_returns_spans() {
        let t = theme();
        let mut state = SyntaxState::new("rust", "rs").unwrap();
        let source = "fn main() {}\n";
        let buffer = TextBuffer::from_text(source);
        state.parse(source);

        let spans = highlights_for_line(&state, &buffer, 0, &t);
        assert!(
            !spans.is_empty(),
            "Expected highlight spans for 'fn main() {{}}'"
        );

        // "fn" should be highlighted as keyword at columns 0..2
        let has_fn_keyword = spans
            .iter()
            .any(|&(start, end, color)| start == 0 && end == 2 && color == t.syntax.keyword);
        assert!(has_fn_keyword, "Expected 'fn' to be highlighted as keyword");
    }

    #[test]
    fn highlights_for_line_no_syntax_state_returns_empty() {
        // No SyntaxState means no highlights — tested via the renderer fallback.
        // Here we test that an empty file produces no spans.
        let t = theme();
        let mut state = SyntaxState::new("json", "json").unwrap();
        let source = "";
        let buffer = TextBuffer::from_text(source);
        state.parse(source);
        let spans = highlights_for_line(&state, &buffer, 0, &t);
        assert!(spans.is_empty());
    }

    #[test]
    fn highlights_for_line_multiline() {
        let t = theme();
        let mut state = SyntaxState::new("rust", "rs").unwrap();
        let source = "let x = 42;\nfn foo() {}";
        let buffer = TextBuffer::from_text(source);
        state.parse(source);

        // Line 0: "let x = 42;" — should have "let" as keyword, "42" as number
        let spans0 = highlights_for_line(&state, &buffer, 0, &t);
        let has_let = spans0
            .iter()
            .any(|&(start, end, color)| start == 0 && end == 3 && color == t.syntax.keyword);
        assert!(has_let, "Expected 'let' keyword on line 0");

        // Line 1: "fn foo() {}" — should have "fn" as keyword
        let spans1 = highlights_for_line(&state, &buffer, 1, &t);
        let has_fn = spans1
            .iter()
            .any(|&(start, end, color)| start == 0 && end == 2 && color == t.syntax.keyword);
        assert!(has_fn, "Expected 'fn' keyword on line 1");
    }

    #[test]
    fn highlights_for_line_unicode() {
        let t = theme();
        let mut state = SyntaxState::new("rust", "rs").unwrap();
        let source = "let å = 42;\n";
        let buffer = TextBuffer::from_text(source);
        state.parse(source);

        let spans = highlights_for_line(&state, &buffer, 0, &t);
        // "42" should be highlighted as number — char columns, not byte columns
        let has_number = spans.iter().any(|&(_, _, color)| color == t.syntax.number);
        assert!(has_number, "Expected number highlight for '42'");
    }
}
