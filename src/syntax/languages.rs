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
        "c" | "h" => Some("c"),
        "cpp" | "hpp" | "cc" | "cxx" | "hh" => Some("cpp"),
        "cs" => Some("csharp"),
        "go" => Some("go"),
        "java" => Some("java"),
        "rb" => Some("ruby"),
        "php" => Some("php"),
        "sh" | "bash" => Some("bash"),
        "css" => Some("css"),
        "html" | "htm" => Some("html"),
        "toml" => Some("toml"),
        "yml" | "yaml" => Some("yaml"),
        "md" => Some("markdown"),
        "lua" => Some("lua"),
        "swift" => Some("swift"),
        "scala" => Some("scala"),
        "zig" => Some("zig"),
        "ex" | "exs" => Some("elixir"),
        "hs" => Some("haskell"),
        "r" | "R" => Some("r"),
        "dart" => Some("dart"),
        "ml" | "mli" => Some("ocaml"),
        "svelte" => Some("svelte"),
        "hbs" | "handlebars" => Some("handlebars"),
        "sql" => Some("sql"),
        "Dockerfile" => Some("dockerfile"),
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
        "c" => Some(tree_sitter_c::LANGUAGE.into()),
        "cpp" => Some(tree_sitter_cpp::LANGUAGE.into()),
        "csharp" => Some(tree_sitter_c_sharp::LANGUAGE.into()),
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        "java" => Some(tree_sitter_java::LANGUAGE.into()),
        "ruby" => Some(tree_sitter_ruby::LANGUAGE.into()),
        "php" => Some(tree_sitter_php::LANGUAGE_PHP.into()),
        "bash" => Some(tree_sitter_bash::LANGUAGE.into()),
        "css" => Some(tree_sitter_css::LANGUAGE.into()),
        "html" => Some(tree_sitter_html::LANGUAGE.into()),
        "toml" => Some(tree_sitter_toml_ng::LANGUAGE.into()),
        "yaml" => Some(tree_sitter_yaml::LANGUAGE.into()),
        "markdown" => Some(tree_sitter_md::LANGUAGE.into()),
        "lua" => Some(tree_sitter_lua::LANGUAGE.into()),
        "swift" => Some(tree_sitter_swift::LANGUAGE.into()),
        "scala" => Some(tree_sitter_scala::LANGUAGE.into()),
        "zig" => Some(tree_sitter_zig::LANGUAGE.into()),
        "elixir" => Some(tree_sitter_elixir::LANGUAGE.into()),
        "haskell" => Some(tree_sitter_haskell::LANGUAGE.into()),
        "r" => Some(tree_sitter_r::LANGUAGE.into()),
        "dart" => Some(tree_sitter_dart::LANGUAGE.into()),
        "ocaml" => Some(tree_sitter_ocaml::LANGUAGE_OCAML.into()),
        "svelte" => Some(tree_sitter_svelte_ng::LANGUAGE.into()),
        "handlebars" => Some(tree_sitter_handlebars::LANGUAGE.into()),
        _ => None,
    }
}

/// Get grammar for a specific file extension.
/// Needed because .tsx uses a different grammar (LANGUAGE_TSX) than .ts (LANGUAGE_TYPESCRIPT).
pub fn get_grammar_for_extension(ext: &str) -> Option<Language> {
    match ext {
        "tsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        "mli" => Some(tree_sitter_ocaml::LANGUAGE_OCAML_INTERFACE.into()),
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
        assert_eq!(language_for_extension("c"), Some("c"));
        assert_eq!(language_for_extension("h"), Some("c"));
        assert_eq!(language_for_extension("cpp"), Some("cpp"));
        assert_eq!(language_for_extension("hpp"), Some("cpp"));
        assert_eq!(language_for_extension("cs"), Some("csharp"));
        assert_eq!(language_for_extension("go"), Some("go"));
        assert_eq!(language_for_extension("java"), Some("java"));
        assert_eq!(language_for_extension("rb"), Some("ruby"));
        assert_eq!(language_for_extension("php"), Some("php"));
        assert_eq!(language_for_extension("sh"), Some("bash"));
        assert_eq!(language_for_extension("bash"), Some("bash"));
        assert_eq!(language_for_extension("css"), Some("css"));
        assert_eq!(language_for_extension("html"), Some("html"));
        assert_eq!(language_for_extension("htm"), Some("html"));
        assert_eq!(language_for_extension("toml"), Some("toml"));
        assert_eq!(language_for_extension("yml"), Some("yaml"));
        assert_eq!(language_for_extension("yaml"), Some("yaml"));
        assert_eq!(language_for_extension("md"), Some("markdown"));
        assert_eq!(language_for_extension("lua"), Some("lua"));
        assert_eq!(language_for_extension("swift"), Some("swift"));
        assert_eq!(language_for_extension("scala"), Some("scala"));
        assert_eq!(language_for_extension("zig"), Some("zig"));
        assert_eq!(language_for_extension("ex"), Some("elixir"));
        assert_eq!(language_for_extension("exs"), Some("elixir"));
        assert_eq!(language_for_extension("hs"), Some("haskell"));
        assert_eq!(language_for_extension("r"), Some("r"));
        assert_eq!(language_for_extension("R"), Some("r"));
        assert_eq!(language_for_extension("dart"), Some("dart"));
        assert_eq!(language_for_extension("ml"), Some("ocaml"));
        assert_eq!(language_for_extension("mli"), Some("ocaml"));
        assert_eq!(language_for_extension("svelte"), Some("svelte"));
        assert_eq!(language_for_extension("hbs"), Some("handlebars"));
        assert_eq!(language_for_extension("sql"), Some("sql"));
        assert_eq!(language_for_extension("Dockerfile"), Some("dockerfile"));
    }

    #[test]
    fn unknown_extension() {
        assert_eq!(language_for_extension("xyz"), None);
        assert_eq!(language_for_extension("txt"), None);
        assert_eq!(language_for_extension(""), None);
    }

    #[test]
    fn get_grammar_for_all_languages() {
        let languages = [
            "rust",
            "json",
            "python",
            "javascript",
            "typescript",
            "c",
            "cpp",
            "csharp",
            "go",
            "java",
            "ruby",
            "php",
            "bash",
            "css",
            "html",
            "toml",
            "yaml",
            "markdown",
            "lua",
            "swift",
            "scala",
            "zig",
            "elixir",
            "haskell",
            "r",
            "dart",
            "ocaml",
            "svelte",
            "handlebars",
        ];
        for lang in languages {
            assert!(
                get_language_grammar(lang).is_some(),
                "Grammar missing for {lang}"
            );
        }
    }

    #[test]
    fn get_grammar_for_unknown_language() {
        assert!(get_language_grammar("brainfuck").is_none());
    }
}
