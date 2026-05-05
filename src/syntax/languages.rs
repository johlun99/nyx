#![allow(dead_code)]

use tree_sitter::Language;

/// Map file extension (without dot) to language name.
pub fn language_for_extension(ext: &str) -> Option<&'static str> {
    match ext {
        "rs" => Some("rust"),
        "json" => Some("json"),
        "py" => Some("python"),
        "js" | "jsx" | "mjs" => Some("javascript"),
        "ts" | "tsx" => Some("typescript"),
        _ => None,
    }
}

/// Get the tree-sitter Language grammar for a language name.
pub fn get_language_grammar(name: &str) -> Option<Language> {
    match name {
        "rust" => Some(tree_sitter_rust::LANGUAGE.into()),
        "json" => Some(tree_sitter_json::LANGUAGE.into()),
        "python" => Some(tree_sitter_python::LANGUAGE.into()),
        "javascript" => Some(tree_sitter_javascript::LANGUAGE.into()),
        "typescript" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        _ => None,
    }
}

/// Get grammar for a specific file extension.
/// Needed because .tsx uses a different grammar (LANGUAGE_TSX) than .ts (LANGUAGE_TYPESCRIPT).
pub fn get_grammar_for_extension(ext: &str) -> Option<Language> {
    match ext {
        "tsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        _ => {
            let lang_name = language_for_extension(ext)?;
            get_language_grammar(lang_name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_extensions() {
        assert_eq!(language_for_extension("rs"), Some("rust"));
        assert_eq!(language_for_extension("json"), Some("json"));
        assert_eq!(language_for_extension("py"), Some("python"));
        assert_eq!(language_for_extension("js"), Some("javascript"));
        assert_eq!(language_for_extension("jsx"), Some("javascript"));
        assert_eq!(language_for_extension("mjs"), Some("javascript"));
        assert_eq!(language_for_extension("ts"), Some("typescript"));
        assert_eq!(language_for_extension("tsx"), Some("typescript"));
    }

    #[test]
    fn unknown_extension() {
        assert_eq!(language_for_extension("xyz"), None);
        assert_eq!(language_for_extension("txt"), None);
        assert_eq!(language_for_extension(""), None);
    }

    #[test]
    fn get_grammar_for_configured_language() {
        assert!(get_language_grammar("rust").is_some());
    }

    #[test]
    fn get_grammar_for_unknown_language() {
        assert!(get_language_grammar("haskell").is_none());
    }
}
