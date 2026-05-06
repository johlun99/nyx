#![allow(dead_code)]

pub mod highlighter;
pub mod indent;
pub mod languages;
pub mod simple;

use simple::{compute_simple_highlights, simple_rules_for_language, SimpleRules};
use tree_sitter::StreamingIterator as _;

enum HighlightEngine {
    TreeSitter {
        parser: tree_sitter::Parser,
        tree: Option<tree_sitter::Tree>,
        highlight_query: tree_sitter::Query,
    },
    Simple(SimpleRules),
}

pub struct SyntaxState {
    pub language_name: String,
    engine: HighlightEngine,
    dirty: bool,
    /// Cached highlights: (start_byte, end_byte, capture_name)
    cached_highlights: Vec<(usize, usize, String)>,
}

impl SyntaxState {
    /// Create state for a language. Tries tree-sitter first, then falls back
    /// to simple keyword-based highlighting. Returns None if neither is available.
    pub fn new(lang_name: &str, extension: &str) -> Option<Self> {
        // Try tree-sitter first
        if let Some(grammar) = languages::get_grammar_for_extension(extension) {
            let mut parser = tree_sitter::Parser::new();
            if parser.set_language(&grammar).is_ok() {
                if let Some(query_source) = query_source_for_language(lang_name) {
                    if let Ok(query) = tree_sitter::Query::new(&grammar, query_source) {
                        return Some(Self {
                            language_name: lang_name.to_string(),
                            engine: HighlightEngine::TreeSitter {
                                parser,
                                tree: None,
                                highlight_query: query,
                            },
                            dirty: false,
                            cached_highlights: Vec::new(),
                        });
                    }
                }
            }
        }

        // Fall back to simple keyword highlighter
        let rules = simple_rules_for_language(lang_name)?;
        Some(Self {
            language_name: lang_name.to_string(),
            engine: HighlightEngine::Simple(rules),
            dirty: false,
            cached_highlights: Vec::new(),
        })
    }

    /// Parse source text, update tree and cached_highlights. Sets dirty=false.
    pub fn parse(&mut self, source: &str) {
        match &mut self.engine {
            HighlightEngine::TreeSitter {
                parser,
                tree,
                highlight_query,
            } => {
                *tree = parser.parse(source, tree.as_ref());
                self.cached_highlights.clear();
                if let Some(ref t) = tree {
                    let mut cursor = tree_sitter::QueryCursor::new();
                    let mut captures =
                        cursor.captures(highlight_query, t.root_node(), source.as_bytes());
                    while let Some((query_match, capture_index)) = captures.next() {
                        let capture = &query_match.captures[*capture_index];
                        let name = highlight_query.capture_names()[capture.index as usize];
                        self.cached_highlights.push((
                            capture.node.start_byte(),
                            capture.node.end_byte(),
                            name.to_string(),
                        ));
                    }
                }
            }
            HighlightEngine::Simple(rules) => {
                self.cached_highlights = compute_simple_highlights(rules, source);
            }
        }
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

    /// Return reference to tree (None for Simple engine).
    pub fn tree(&self) -> Option<&tree_sitter::Tree> {
        match &self.engine {
            HighlightEngine::TreeSitter { tree, .. } => tree.as_ref(),
            HighlightEngine::Simple(_) => None,
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
        "c" => Some(include_str!("queries/c/highlights.scm")),
        "cpp" => Some(include_str!("queries/cpp/highlights.scm")),
        "csharp" => Some(include_str!("queries/csharp/highlights.scm")),
        "go" => Some(include_str!("queries/go/highlights.scm")),
        "java" => Some(include_str!("queries/java/highlights.scm")),
        "ruby" => Some(include_str!("queries/ruby/highlights.scm")),
        "php" => Some(include_str!("queries/php/highlights.scm")),
        "bash" => Some(include_str!("queries/bash/highlights.scm")),
        "css" => Some(include_str!("queries/css/highlights.scm")),
        "html" => Some(include_str!("queries/html/highlights.scm")),
        "toml" => Some(include_str!("queries/toml/highlights.scm")),
        "yaml" => Some(include_str!("queries/yaml/highlights.scm")),
        "markdown" => Some(include_str!("queries/markdown/highlights.scm")),
        "lua" => Some(include_str!("queries/lua/highlights.scm")),
        "swift" => Some(include_str!("queries/swift/highlights.scm")),
        "scala" => Some(include_str!("queries/scala/highlights.scm")),
        "zig" => Some(include_str!("queries/zig/highlights.scm")),
        "elixir" => Some(include_str!("queries/elixir/highlights.scm")),
        "haskell" => Some(include_str!("queries/haskell/highlights.scm")),
        "r" => Some(include_str!("queries/r/highlights.scm")),
        "dart" => Some(include_str!("queries/dart/highlights.scm")),
        "ocaml" => Some(include_str!("queries/ocaml/highlights.scm")),
        "svelte" => Some(include_str!("queries/svelte/highlights.scm")),
        "handlebars" => Some(include_str!("queries/handlebars/highlights.scm")),
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
        let state = SyntaxState::new("brainfuck", "bf");
        assert!(state.is_none());
    }

    #[test]
    fn parse_rust_source() {
        let mut state = SyntaxState::new("rust", "rs").unwrap();
        state.parse("fn main() {}");
        assert!(state.tree().is_some());
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
        assert!(state.tree().is_some());
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

    // Simple highlighter integration tests

    #[test]
    fn create_syntax_state_for_sql() {
        let state = SyntaxState::new("sql", "sql");
        assert!(state.is_some());
        assert_eq!(state.unwrap().language_name, "sql");
    }

    #[test]
    fn create_syntax_state_for_dockerfile() {
        let state = SyntaxState::new("dockerfile", "Dockerfile");
        assert!(state.is_some());
        assert_eq!(state.unwrap().language_name, "dockerfile");
    }

    #[test]
    fn sql_tree_returns_none() {
        let state = SyntaxState::new("sql", "sql").unwrap();
        assert!(state.tree().is_none());
    }

    #[test]
    fn parse_sql_source() {
        let mut state = SyntaxState::new("sql", "sql").unwrap();
        state.parse("SELECT id FROM users WHERE active = true");
        assert!(!state.cached_highlights.is_empty());
        let has_keyword = state
            .cached_highlights
            .iter()
            .any(|(_, _, name)| name == "keyword");
        assert!(has_keyword, "Expected keyword highlights for SQL");
    }

    #[test]
    fn parse_dockerfile_source() {
        let mut state = SyntaxState::new("dockerfile", "Dockerfile").unwrap();
        state.parse("FROM ubuntu:22.04\nRUN apt-get update");
        assert!(!state.cached_highlights.is_empty());
        let has_keyword = state
            .cached_highlights
            .iter()
            .any(|(_, _, name)| name == "keyword");
        assert!(has_keyword, "Expected keyword highlights for Dockerfile");
    }

    #[test]
    fn all_queries_compile() {
        let languages = [
            ("rust", "rs"),
            ("json", "json"),
            ("python", "py"),
            ("javascript", "js"),
            ("typescript", "ts"),
            ("c", "c"),
            ("cpp", "cpp"),
            ("csharp", "cs"),
            ("go", "go"),
            ("java", "java"),
            ("ruby", "rb"),
            ("php", "php"),
            ("bash", "sh"),
            ("css", "css"),
            ("html", "html"),
            ("toml", "toml"),
            ("yaml", "yml"),
            ("markdown", "md"),
            ("lua", "lua"),
            ("swift", "swift"),
            ("scala", "scala"),
            ("zig", "zig"),
            ("elixir", "ex"),
            ("haskell", "hs"),
            ("r", "r"),
            ("dart", "dart"),
            ("ocaml", "ml"),
            ("svelte", "svelte"),
            ("handlebars", "hbs"),
        ];
        let mut failed = Vec::new();
        for (lang, ext) in languages {
            if SyntaxState::new(lang, ext).is_none() {
                // Try to get more detail
                let grammar = languages::get_grammar_for_extension(ext);
                if grammar.is_none() {
                    failed.push(format!("{lang}: no grammar"));
                    continue;
                }
                let grammar = grammar.unwrap();
                let query_src = query_source_for_language(lang);
                if query_src.is_none() {
                    failed.push(format!("{lang}: no query source"));
                    continue;
                }
                match tree_sitter::Query::new(&grammar, query_src.unwrap()) {
                    Err(e) => failed.push(format!("{lang}: {e}")),
                    Ok(_) => failed.push(format!("{lang}: unknown failure")),
                }
            }
        }
        assert!(
            failed.is_empty(),
            "Failed languages:\n{}",
            failed.join("\n")
        );
    }

    #[test]
    fn simple_dirty_flag_works() {
        let mut state = SyntaxState::new("sql", "sql").unwrap();
        state.parse("SELECT 1");
        assert!(!state.dirty);
        state.mark_dirty();
        assert!(state.dirty);
        state.ensure_parsed("SELECT 2");
        assert!(!state.dirty);
        assert!(!state.cached_highlights.is_empty());
    }
}
