// src/lsp/protocol.rs
//! LSP types: Position, Range, Diagnostic, CompletionItem, etc.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// LSP Position — 0-based line and UTF-16 character offset.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// LSP Range.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

/// LSP TextDocumentIdentifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextDocumentIdentifier {
    pub uri: String,
}

/// LSP VersionedTextDocumentIdentifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionedTextDocumentIdentifier {
    pub uri: String,
    pub version: i32,
}

/// LSP TextDocumentItem (used in didOpen).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextDocumentItem {
    pub uri: String,
    pub language_id: String,
    pub version: i32,
    pub text: String,
}

/// Diagnostic severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[allow(dead_code)]
pub struct DiagnosticSeverity(pub u8);

#[allow(dead_code)]
impl DiagnosticSeverity {
    pub const ERROR: Self = Self(1);
    pub const WARNING: Self = Self(2);
    pub const INFORMATION: Self = Self(3);
    pub const HINT: Self = Self(4);
}

/// LSP Diagnostic.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: Option<DiagnosticSeverity>,
    pub message: String,
    #[serde(default)]
    pub source: Option<String>,
}

/// CompletionItemKind numeric values from LSP spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct CompletionItemKind(pub u8);

#[allow(dead_code)]
impl CompletionItemKind {
    pub const TEXT: Self = Self(1);
    pub const METHOD: Self = Self(2);
    pub const FUNCTION: Self = Self(3);
    pub const CONSTRUCTOR: Self = Self(4);
    pub const FIELD: Self = Self(5);
    pub const VARIABLE: Self = Self(6);
    pub const CLASS: Self = Self(7);
    pub const INTERFACE: Self = Self(8);
    pub const MODULE: Self = Self(9);
    pub const PROPERTY: Self = Self(10);
    pub const UNIT: Self = Self(11);
    pub const VALUE: Self = Self(12);
    pub const ENUM: Self = Self(13);
    pub const KEYWORD: Self = Self(14);
    pub const SNIPPET: Self = Self(15);
    pub const STRUCT: Self = Self(22);
    pub const CONSTANT: Self = Self(21);

    pub fn icon(self) -> &'static str {
        match self.0 {
            2 | 3 => "fn",
            4 => "ct",
            5 | 10 => "fd",
            6 => "vr",
            7 | 22 => "st",
            8 => "if",
            9 => "md",
            13 => "en",
            14 => "kw",
            15 => "sn",
            21 => "cn",
            _ => "  ",
        }
    }
}

/// LSP CompletionItem.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionItem {
    pub label: String,
    #[serde(default)]
    pub kind: Option<CompletionItemKind>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub insert_text: Option<String>,
    #[serde(default)]
    pub filter_text: Option<String>,
    #[serde(default)]
    pub sort_text: Option<String>,
}

impl CompletionItem {
    /// The text to insert when this completion is accepted.
    pub fn text_to_insert(&self) -> &str {
        self.insert_text.as_deref().unwrap_or(&self.label)
    }
}

/// LSP CompletionList.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct CompletionList {
    pub is_incomplete: bool,
    pub items: Vec<CompletionItem>,
}

/// Completion response can be either a list or an array.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum CompletionResponse {
    Array(Vec<CompletionItem>),
    List(CompletionList),
}

impl CompletionResponse {
    pub fn into_items(self) -> Vec<CompletionItem> {
        match self {
            Self::Array(items) => items,
            Self::List(list) => list.items,
        }
    }
}

/// TextDocumentSyncKind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextDocumentSyncKind {
    None,
    Full,
    Incremental,
}

impl TextDocumentSyncKind {
    pub fn from_value(v: &Value) -> Self {
        match v.as_u64() {
            Some(1) => Self::Full,
            Some(2) => Self::Incremental,
            _ => Self::None,
        }
    }
}

/// Server capabilities we care about from the initialize response.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ServerCapabilities {
    pub text_document_sync: TextDocumentSyncKind,
    pub completion_provider: bool,
}

impl ServerCapabilities {
    pub fn from_value(caps: &Value) -> Self {
        let sync = caps
            .get("textDocumentSync")
            .map(|v| {
                // Can be a number or an object with "change" field
                if v.is_number() {
                    TextDocumentSyncKind::from_value(v)
                } else if let Some(change) = v.get("change") {
                    TextDocumentSyncKind::from_value(change)
                } else {
                    TextDocumentSyncKind::None
                }
            })
            .unwrap_or(TextDocumentSyncKind::None);

        let completion = caps.get("completionProvider").is_some();

        Self {
            text_document_sync: sync,
            completion_provider: completion,
        }
    }
}

/// Convert a char offset in the buffer to an LSP Position (line + UTF-16 offset).
pub fn char_offset_to_lsp_position(text: &str, char_offset: usize) -> Position {
    let mut line: u32 = 0;
    let mut col_utf16: u32 = 0;

    for (chars_seen, ch) in text.chars().enumerate() {
        if chars_seen >= char_offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col_utf16 = 0;
        } else {
            col_utf16 += ch.len_utf16() as u32;
        }
    }

    Position {
        line,
        character: col_utf16,
    }
}

/// Convert an LSP Position to a char offset.
pub fn lsp_position_to_char_offset(text: &str, pos: Position) -> usize {
    let mut line: u32 = 0;
    let mut col_utf16: u32 = 0;
    let mut offset: usize = 0;

    for ch in text.chars() {
        if line == pos.line && col_utf16 >= pos.character {
            break;
        }
        if line > pos.line {
            break;
        }
        if ch == '\n' {
            line += 1;
            col_utf16 = 0;
        } else {
            col_utf16 += ch.len_utf16() as u32;
        }
        offset += 1;
    }

    offset
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_conversion_ascii() {
        let text = "hello\nworld\nfoo";
        // 'w' is at line 1, col 0 => char offset 6
        let pos = char_offset_to_lsp_position(text, 6);
        assert_eq!(
            pos,
            Position {
                line: 1,
                character: 0
            }
        );

        let offset = lsp_position_to_char_offset(
            text,
            Position {
                line: 1,
                character: 0,
            },
        );
        assert_eq!(offset, 6);
    }

    #[test]
    fn position_conversion_unicode() {
        let text = "åäö\nhej";
        // 'h' is at char offset 4 (å=0, ä=1, ö=2, \n=3, h=4)
        let pos = char_offset_to_lsp_position(text, 4);
        assert_eq!(
            pos,
            Position {
                line: 1,
                character: 0
            }
        );

        let offset = lsp_position_to_char_offset(
            text,
            Position {
                line: 1,
                character: 0,
            },
        );
        assert_eq!(offset, 4);
    }

    #[test]
    fn position_conversion_emoji() {
        // 😀 is U+1F600, which is 2 UTF-16 code units
        let text = "a😀b\nc";
        // 'b' is at char offset 2, but UTF-16 col is 3 (a=1, 😀=2)
        let pos = char_offset_to_lsp_position(text, 2);
        assert_eq!(
            pos,
            Position {
                line: 0,
                character: 3
            }
        );

        let offset = lsp_position_to_char_offset(
            text,
            Position {
                line: 0,
                character: 3,
            },
        );
        assert_eq!(offset, 2);
    }

    #[test]
    fn position_start_of_file() {
        let text = "hello";
        let pos = char_offset_to_lsp_position(text, 0);
        assert_eq!(
            pos,
            Position {
                line: 0,
                character: 0
            }
        );

        let offset = lsp_position_to_char_offset(
            text,
            Position {
                line: 0,
                character: 0,
            },
        );
        assert_eq!(offset, 0);
    }

    #[test]
    fn completion_item_text_to_insert() {
        let item = CompletionItem {
            label: "println!".to_string(),
            kind: Some(CompletionItemKind::FUNCTION),
            detail: None,
            insert_text: Some("println!($0)".to_string()),
            filter_text: None,
            sort_text: None,
        };
        assert_eq!(item.text_to_insert(), "println!($0)");

        let item2 = CompletionItem {
            label: "String".to_string(),
            kind: Some(CompletionItemKind::STRUCT),
            detail: None,
            insert_text: None,
            filter_text: None,
            sort_text: None,
        };
        assert_eq!(item2.text_to_insert(), "String");
    }

    #[test]
    fn server_capabilities_from_value() {
        let json: Value = serde_json::from_str(
            r#"{
            "textDocumentSync": 1,
            "completionProvider": { "triggerCharacters": ["."] }
        }"#,
        )
        .unwrap();
        let caps = ServerCapabilities::from_value(&json);
        assert_eq!(caps.text_document_sync, TextDocumentSyncKind::Full);
        assert!(caps.completion_provider);
    }

    #[test]
    fn server_capabilities_object_sync() {
        let json: Value = serde_json::from_str(
            r#"{
            "textDocumentSync": { "change": 2 }
        }"#,
        )
        .unwrap();
        let caps = ServerCapabilities::from_value(&json);
        assert_eq!(caps.text_document_sync, TextDocumentSyncKind::Incremental);
        assert!(!caps.completion_provider);
    }
}
