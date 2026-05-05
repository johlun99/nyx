#![allow(dead_code)]

pub mod languages;

use tree_sitter::StreamingIterator as _;

pub struct SyntaxState {
    pub language_name: String,
    parser: tree_sitter::Parser,
    tree: Option<tree_sitter::Tree>,
    highlight_query: tree_sitter::Query,
    dirty: bool,
    /// Cached highlights: (start_byte, end_byte, capture_name)
    cached_highlights: Vec<(usize, usize, String)>,
}

impl SyntaxState {
    /// Create state for a language. Gets grammar via `languages::get_grammar_for_extension`,
    /// creates parser, loads highlight query via `include_str!`. Returns None if grammar or
    /// query unavailable.
    pub fn new(lang_name: &str, extension: &str) -> Option<Self> {
        let grammar = languages::get_grammar_for_extension(extension)?;

        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&grammar).ok()?;

        let query_source = query_source_for_language(lang_name)?;
        let query = tree_sitter::Query::new(&grammar, query_source).ok()?;

        Some(Self {
            language_name: lang_name.to_string(),
            parser,
            tree: None,
            highlight_query: query,
            dirty: false,
            cached_highlights: Vec::new(),
        })
    }

    /// Parse source text, update tree and cached_highlights. Sets dirty=false.
    pub fn parse(&mut self, source: &str) {
        self.tree = self.parser.parse(source, self.tree.as_ref());
        self.recompute_highlights(source);
        self.dirty = false;
    }

    /// Set dirty=true.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Re-parse only if dirty.
    pub fn ensure_parsed(&mut self, source: &str) {
        if self.dirty {
            self.parse(source);
        }
    }

    /// Return cached highlights.
    pub fn highlights(&self) -> &[(usize, usize, String)] {
        &self.cached_highlights
    }

    /// Return reference to tree.
    pub fn tree(&self) -> Option<&tree_sitter::Tree> {
        self.tree.as_ref()
    }

    fn recompute_highlights(&mut self, source: &str) {
        self.cached_highlights.clear();
        let Some(ref tree) = self.tree else { return };
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut captures =
            cursor.captures(&self.highlight_query, tree.root_node(), source.as_bytes());
        while let Some((query_match, capture_index)) = captures.next() {
            let capture = &query_match.captures[*capture_index];
            let name = self.highlight_query.capture_names()[capture.index as usize];
            self.cached_highlights.push((
                capture.node.start_byte(),
                capture.node.end_byte(),
                name.to_string(),
            ));
        }
    }
}

fn query_source_for_language(lang_name: &str) -> Option<&'static str> {
    match lang_name {
        "rust" => Some(include_str!("queries/rust/highlights.scm")),
        "json" => Some(include_str!("queries/json/highlights.scm")),
        "python" => Some(include_str!("queries/python/highlights.scm")),
        "javascript" => Some(include_str!("queries/javascript/highlights.scm")),
        "typescript" => Some(include_str!("queries/typescript/highlights.scm")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_syntax_state_for_rust() {
        let state = SyntaxState::new("rust", "rs");
        assert!(state.is_some());
        assert_eq!(state.unwrap().language_name, "rust");
    }

    #[test]
    fn create_syntax_state_for_unknown_language() {
        let state = SyntaxState::new("haskell", "hs");
        assert!(state.is_none());
    }

    #[test]
    fn parse_rust_source() {
        let mut state = SyntaxState::new("rust", "rs").unwrap();
        state.parse("fn main() {}");
        assert!(state.tree.is_some());
        assert!(!state.cached_highlights.is_empty());
    }

    #[test]
    fn dirty_flag_prevents_unnecessary_reparse() {
        let mut state = SyntaxState::new("rust", "rs").unwrap();
        state.parse("fn main() {}");
        assert!(!state.dirty);
        state.mark_dirty();
        assert!(state.dirty);
        state.ensure_parsed("fn main() {}");
        assert!(!state.dirty);
    }

    #[test]
    fn parse_json_source() {
        let mut state = SyntaxState::new("json", "json").unwrap();
        state.parse(r#"{"key": "value", "num": 42}"#);
        assert!(state.tree.is_some());
        assert!(!state.cached_highlights.is_empty());
    }

    #[test]
    fn highlights_contain_keyword_for_rust() {
        let mut state = SyntaxState::new("rust", "rs").unwrap();
        state.parse("fn main() {}");
        let has_keyword = state
            .cached_highlights
            .iter()
            .any(|(_, _, name)| name == "keyword");
        assert!(has_keyword, "Expected @keyword captures for 'fn'");
    }

    #[test]
    fn highlights_contain_function_for_rust() {
        let mut state = SyntaxState::new("rust", "rs").unwrap();
        state.parse("fn main() {}");
        let has_function = state
            .cached_highlights
            .iter()
            .any(|(_, _, name)| name == "function");
        assert!(has_function, "Expected @function captures for 'main'");
    }
}
