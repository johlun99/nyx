// src/renderer/theme.rs
use eframe::egui::Color32;

#[allow(dead_code)]
pub struct SyntaxColors {
    pub keyword: Color32,
    pub string: Color32,
    pub comment: Color32,
    pub function: Color32,
    pub r#type: Color32,
    pub number: Color32,
    pub operator: Color32,
    pub punctuation: Color32,
}

#[allow(dead_code)]
pub struct Theme {
    pub name: String,
    pub background: Color32,
    pub foreground: Color32,
    pub cursor: Color32,
    pub cursor_insert: Color32,
    pub selection: Color32,
    pub line_number: Color32,
    pub line_number_active: Color32,
    pub gutter_background: Color32,
    pub status_bar_bg: Color32,
    pub status_bar_fg: Color32,
    pub syntax: SyntaxColors,
    pub search_match: Color32,
    pub search_current: Color32,
    // Diagnostic colors
    pub error_fg: Color32,
    pub warning_fg: Color32,
    pub error_underline: Color32,
    pub warning_underline: Color32,
}

impl Theme {
    pub fn default_dark() -> Self {
        Self {
            name: "default-dark".into(),
            background: Color32::from_rgb(0x1e, 0x1e, 0x2e),
            foreground: Color32::from_rgb(0xcd, 0xd6, 0xf4),
            cursor: Color32::from_rgb(0xf5, 0xe0, 0xdc),
            cursor_insert: Color32::from_rgb(0xf5, 0xe0, 0xdc),
            selection: Color32::from_rgb(0x45, 0x47, 0x5a),
            line_number: Color32::from_rgb(0x6c, 0x70, 0x86),
            line_number_active: Color32::from_rgb(0xcd, 0xd6, 0xf4),
            gutter_background: Color32::from_rgb(0x1e, 0x1e, 0x2e),
            status_bar_bg: Color32::from_rgb(0x31, 0x32, 0x44),
            status_bar_fg: Color32::from_rgb(0xcd, 0xd6, 0xf4),
            syntax: SyntaxColors {
                keyword: Color32::from_rgb(0xcb, 0xa6, 0xf7),  // mauve
                string: Color32::from_rgb(0xa6, 0xe3, 0xa1),   // green
                comment: Color32::from_rgb(0x6c, 0x70, 0x86),  // overlay0
                function: Color32::from_rgb(0x89, 0xb4, 0xfa), // blue
                r#type: Color32::from_rgb(0xf9, 0xe2, 0xaf),   // yellow
                number: Color32::from_rgb(0xfa, 0xb3, 0x87),   // peach
                operator: Color32::from_rgb(0x89, 0xdc, 0xeb), // sky
                punctuation: Color32::from_rgb(0x93, 0x99, 0xb2), // overlay2
            },
            search_match: Color32::from_rgba_premultiplied(0x30, 0x2c, 0x22, 0x50),
            search_current: Color32::from_rgba_premultiplied(0x50, 0x38, 0x28, 0x70),
            // Catppuccin Mocha: red = #f38ba8, yellow = #f9e2af
            error_fg: Color32::from_rgb(0xf3, 0x8b, 0xa8),
            warning_fg: Color32::from_rgb(0xf9, 0xe2, 0xaf),
            error_underline: Color32::from_rgba_premultiplied(0xf3, 0x8b, 0xa8, 0xb0),
            warning_underline: Color32::from_rgba_premultiplied(0xf9, 0xe2, 0xaf, 0xb0),
        }
    }
}
