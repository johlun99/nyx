#![allow(dead_code)]

/// Simple keyword-based highlighter for languages without tree-sitter support.
/// Produces the same `(start_byte, end_byte, capture_name)` output as tree-sitter
/// so the rest of the highlighting pipeline works unchanged.
pub struct SimpleRules {
    pub keywords: &'static [&'static str],
    pub line_comment: Option<&'static str>,
    pub block_comment: Option<(&'static str, &'static str)>,
    pub string_delimiters: &'static [char],
    /// If true, keywords are matched case-insensitively.
    pub case_insensitive_keywords: bool,
}

pub fn simple_rules_for_language(name: &str) -> Option<SimpleRules> {
    match name {
        "sql" => Some(SimpleRules {
            keywords: &[
                "SELECT",
                "FROM",
                "WHERE",
                "INSERT",
                "INTO",
                "VALUES",
                "UPDATE",
                "SET",
                "DELETE",
                "CREATE",
                "DROP",
                "ALTER",
                "TABLE",
                "INDEX",
                "VIEW",
                "DATABASE",
                "JOIN",
                "INNER",
                "LEFT",
                "RIGHT",
                "OUTER",
                "CROSS",
                "ON",
                "AND",
                "OR",
                "NOT",
                "IN",
                "IS",
                "NULL",
                "AS",
                "ORDER",
                "BY",
                "GROUP",
                "HAVING",
                "LIMIT",
                "OFFSET",
                "UNION",
                "ALL",
                "DISTINCT",
                "BETWEEN",
                "LIKE",
                "EXISTS",
                "CASE",
                "WHEN",
                "THEN",
                "ELSE",
                "END",
                "BEGIN",
                "COMMIT",
                "ROLLBACK",
                "TRANSACTION",
                "PRIMARY",
                "KEY",
                "FOREIGN",
                "REFERENCES",
                "CONSTRAINT",
                "DEFAULT",
                "CHECK",
                "UNIQUE",
                "IF",
                "REPLACE",
                "TRIGGER",
                "FUNCTION",
                "PROCEDURE",
                "RETURNS",
                "RETURN",
                "DECLARE",
                "CURSOR",
                "FETCH",
                "OPEN",
                "CLOSE",
                "WITH",
                "RECURSIVE",
                "TEMPORARY",
                "TEMP",
                "CASCADE",
                "RESTRICT",
                "ASC",
                "DESC",
                "EXPLAIN",
                "ANALYZE",
                "GRANT",
                "REVOKE",
                "SCHEMA",
                "INT",
                "INTEGER",
                "BIGINT",
                "SMALLINT",
                "FLOAT",
                "DOUBLE",
                "DECIMAL",
                "NUMERIC",
                "CHAR",
                "VARCHAR",
                "TEXT",
                "BOOLEAN",
                "BOOL",
                "DATE",
                "TIME",
                "TIMESTAMP",
                "SERIAL",
                "AUTOINCREMENT",
                "TRUE",
                "FALSE",
                "COUNT",
                "SUM",
                "AVG",
                "MIN",
                "MAX",
                "COALESCE",
                "CAST",
                "ADD",
                "COLUMN",
                "RENAME",
                "TO",
                "TRUNCATE",
                "EXCEPT",
                "INTERSECT",
            ],
            line_comment: Some("--"),
            block_comment: Some(("/*", "*/")),
            string_delimiters: &['\'', '"'],
            case_insensitive_keywords: true,
        }),
        "dockerfile" => Some(SimpleRules {
            keywords: &[
                "FROM",
                "RUN",
                "CMD",
                "LABEL",
                "MAINTAINER",
                "EXPOSE",
                "ENV",
                "ADD",
                "COPY",
                "ENTRYPOINT",
                "VOLUME",
                "USER",
                "WORKDIR",
                "ARG",
                "ONBUILD",
                "STOPSIGNAL",
                "HEALTHCHECK",
                "SHELL",
                "AS",
            ],
            line_comment: Some("#"),
            block_comment: None,
            string_delimiters: &['\'', '"'],
            case_insensitive_keywords: false,
        }),
        _ => None,
    }
}

pub fn compute_simple_highlights(rules: &SimpleRules, source: &str) -> Vec<(usize, usize, String)> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut highlights = Vec::new();
    let mut i = 0;

    while i < len {
        // Block comments
        if let Some((open, close)) = rules.block_comment {
            if source[i..].starts_with(open) {
                let start = i;
                i += open.len();
                while i < len && !source[i..].starts_with(close) {
                    i += 1;
                }
                if i < len {
                    i += close.len();
                }
                highlights.push((start, i, "comment".to_string()));
                continue;
            }
        }

        // Line comments
        if let Some(prefix) = rules.line_comment {
            if source[i..].starts_with(prefix) {
                let start = i;
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
                highlights.push((start, i, "comment".to_string()));
                continue;
            }
        }

        // String literals
        if rules.string_delimiters.contains(&(bytes[i] as char)) {
            let quote = bytes[i];
            let start = i;
            i += 1;
            while i < len && bytes[i] != quote {
                if bytes[i] == b'\\' {
                    i += 1; // skip escaped char
                }
                i += 1;
            }
            if i < len {
                i += 1; // closing quote
            }
            highlights.push((start, i, "string".to_string()));
            continue;
        }

        // Numbers
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            // Don't match if it's part of an identifier (e.g. "x1")
            if start == 0 || !is_ident_char(bytes[start - 1]) {
                highlights.push((start, i, "number".to_string()));
            }
            continue;
        }

        // Words (identifiers / keywords)
        if is_ident_start(bytes[i]) {
            let start = i;
            while i < len && is_ident_char(bytes[i]) {
                i += 1;
            }
            let word = &source[start..i];
            if is_keyword(word, rules) {
                highlights.push((start, i, "keyword".to_string()));
            }
            continue;
        }

        i += 1;
    }

    highlights
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn is_keyword(word: &str, rules: &SimpleRules) -> bool {
    if rules.case_insensitive_keywords {
        let upper = word.to_ascii_uppercase();
        rules.keywords.contains(&upper.as_str())
    } else {
        rules.keywords.contains(&word)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sql_keywords_highlighted() {
        let rules = simple_rules_for_language("sql").unwrap();
        let source = "SELECT id FROM users WHERE active = true";
        let highlights = compute_simple_highlights(&rules, source);
        let keywords: Vec<&str> = highlights
            .iter()
            .filter(|(_, _, name)| name == "keyword")
            .map(|(s, e, _)| &source[*s..*e])
            .collect();
        assert!(keywords.contains(&"SELECT"));
        assert!(keywords.contains(&"FROM"));
        assert!(keywords.contains(&"WHERE"));
    }

    #[test]
    fn sql_case_insensitive_keywords() {
        let rules = simple_rules_for_language("sql").unwrap();
        let source = "select id from users";
        let highlights = compute_simple_highlights(&rules, source);
        let keywords: Vec<&str> = highlights
            .iter()
            .filter(|(_, _, name)| name == "keyword")
            .map(|(s, e, _)| &source[*s..*e])
            .collect();
        assert!(keywords.contains(&"select"));
        assert!(keywords.contains(&"from"));
    }

    #[test]
    fn sql_line_comment() {
        let rules = simple_rules_for_language("sql").unwrap();
        let source = "-- this is a comment\nSELECT 1";
        let highlights = compute_simple_highlights(&rules, source);
        assert_eq!(highlights[0].2, "comment");
        assert_eq!(
            &source[highlights[0].0..highlights[0].1],
            "-- this is a comment"
        );
    }

    #[test]
    fn sql_block_comment() {
        let rules = simple_rules_for_language("sql").unwrap();
        let source = "/* block comment */ SELECT 1";
        let highlights = compute_simple_highlights(&rules, source);
        assert_eq!(highlights[0].2, "comment");
        assert_eq!(
            &source[highlights[0].0..highlights[0].1],
            "/* block comment */"
        );
    }

    #[test]
    fn sql_string_literal() {
        let rules = simple_rules_for_language("sql").unwrap();
        let source = "SELECT 'hello world'";
        let highlights = compute_simple_highlights(&rules, source);
        let strings: Vec<&str> = highlights
            .iter()
            .filter(|(_, _, name)| name == "string")
            .map(|(s, e, _)| &source[*s..*e])
            .collect();
        assert!(strings.contains(&"'hello world'"));
    }

    #[test]
    fn sql_number() {
        let rules = simple_rules_for_language("sql").unwrap();
        let source = "SELECT 42";
        let highlights = compute_simple_highlights(&rules, source);
        let numbers: Vec<&str> = highlights
            .iter()
            .filter(|(_, _, name)| name == "number")
            .map(|(s, e, _)| &source[*s..*e])
            .collect();
        assert!(numbers.contains(&"42"));
    }

    #[test]
    fn dockerfile_instructions_highlighted() {
        let rules = simple_rules_for_language("dockerfile").unwrap();
        let source = "FROM ubuntu:22.04\nRUN apt-get update\nCOPY . /app\nCMD [\"./app\"]";
        let highlights = compute_simple_highlights(&rules, source);
        let keywords: Vec<&str> = highlights
            .iter()
            .filter(|(_, _, name)| name == "keyword")
            .map(|(s, e, _)| &source[*s..*e])
            .collect();
        assert!(keywords.contains(&"FROM"));
        assert!(keywords.contains(&"RUN"));
        assert!(keywords.contains(&"COPY"));
        assert!(keywords.contains(&"CMD"));
    }

    #[test]
    fn dockerfile_case_sensitive() {
        let rules = simple_rules_for_language("dockerfile").unwrap();
        let source = "from ubuntu\nFROM ubuntu";
        let highlights = compute_simple_highlights(&rules, source);
        let keywords: Vec<&str> = highlights
            .iter()
            .filter(|(_, _, name)| name == "keyword")
            .map(|(s, e, _)| &source[*s..*e])
            .collect();
        // Only uppercase FROM is a keyword in Dockerfile
        assert!(!keywords.contains(&"from"));
        assert!(keywords.contains(&"FROM"));
    }

    #[test]
    fn dockerfile_comment() {
        let rules = simple_rules_for_language("dockerfile").unwrap();
        let source = "# this is a comment\nFROM ubuntu";
        let highlights = compute_simple_highlights(&rules, source);
        assert_eq!(highlights[0].2, "comment");
        assert_eq!(
            &source[highlights[0].0..highlights[0].1],
            "# this is a comment"
        );
    }

    #[test]
    fn unknown_language_returns_none() {
        assert!(simple_rules_for_language("brainfuck").is_none());
    }

    #[test]
    fn empty_source() {
        let rules = simple_rules_for_language("sql").unwrap();
        let highlights = compute_simple_highlights(&rules, "");
        assert!(highlights.is_empty());
    }
}
