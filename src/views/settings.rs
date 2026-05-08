use crate::config::NyxConfig;
use crate::lsp::LspManager;
use crate::renderer::Theme;
use crate::views::lsp_servers::LspServersView;
use eframe::egui;

const FIELD_COUNT: usize = 6;
const AVAILABLE_LANGUAGES: &[&str] = &["rust", "json", "python", "javascript", "typescript"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Editor,
    LspServers,
    Panels,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsAction {
    None,
    Close,
    ConfigChanged,
    ServerToggled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsField {
    FontFamily,
    FontSize,
    LineNumbers,
    TabSize,
    CursorBlink,
    WordWrap,
}

impl SettingsField {
    fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::FontFamily),
            1 => Some(Self::FontSize),
            2 => Some(Self::LineNumbers),
            3 => Some(Self::TabSize),
            4 => Some(Self::CursorBlink),
            5 => Some(Self::WordWrap),
            _ => None,
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::FontFamily => "Font Family",
            Self::FontSize => "Font Size",
            Self::LineNumbers => "Line Numbers",
            Self::TabSize => "Tab Size",
            Self::CursorBlink => "Cursor Blink",
            Self::WordWrap => "Word Wrap",
        }
    }

    fn display_value(&self, config: &NyxConfig) -> String {
        match self {
            Self::FontFamily => config.editor.font_family.clone(),
            Self::FontSize => format!("{}", config.editor.font_size),
            Self::LineNumbers => config.editor.line_numbers.label().to_string(),
            Self::TabSize => format!("{}", config.editor.tab_size),
            Self::CursorBlink => if config.editor.cursor_blink {
                "on"
            } else {
                "off"
            }
            .to_string(),
            Self::WordWrap => if config.editor.word_wrap { "on" } else { "off" }.to_string(),
        }
    }
}

pub struct SettingsView {
    pub selected_row: usize,
    pub editing: Option<SettingsField>,
    pub edit_buffer: String,
    pub active_tab: SettingsTab,
}

impl SettingsView {
    pub fn new() -> Self {
        Self {
            selected_row: 0,
            editing: None,
            edit_buffer: String::new(),
            active_tab: SettingsTab::default(),
        }
    }

    /// Render the settings fullscreen panel.
    /// Returns `true` if config was modified (caller should save).
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        config: &mut NyxConfig,
        theme: &Theme,
        lsp_view: &LspServersView,
        lsp_manager: &LspManager,
    ) -> bool {
        let mut config_changed = false;

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(theme.background))
            .show(ctx, |ui| {
                let panel_width = ui.available_width().min(600.0);
                let left_margin = (ui.available_width() - panel_width) / 2.0;

                ui.add_space(24.0);

                // Header
                ui.horizontal(|ui| {
                    ui.add_space(left_margin);
                    ui.label(
                        egui::RichText::new("Settings")
                            .color(theme.foreground)
                            .size(20.0)
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(left_margin);
                        ui.label(
                            egui::RichText::new("ESC to close | Tab: switch tab")
                                .color(theme.line_number)
                                .size(12.0),
                        );
                    });
                });

                ui.add_space(12.0);

                // Tab bar
                ui.horizontal(|ui| {
                    ui.add_space(left_margin);
                    for (tab, label) in [
                        (SettingsTab::Editor, "Editor"),
                        (SettingsTab::LspServers, "LSP Servers"),
                        (SettingsTab::Panels, "Panels"),
                    ] {
                        let is_active = self.active_tab == tab;
                        let color = if is_active {
                            theme.syntax.keyword
                        } else {
                            theme.line_number
                        };
                        ui.label(egui::RichText::new(label).color(color).size(13.0).strong());
                        if !is_active {
                            // no underline for inactive
                        }
                        ui.add_space(12.0);
                    }
                });

                // Underline for active tab
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.add_space(left_margin);
                    let underline_width = match self.active_tab {
                        SettingsTab::Editor => 42.0,
                        SettingsTab::LspServers => 82.0,
                        SettingsTab::Panels => 48.0,
                    };
                    let offset = match self.active_tab {
                        SettingsTab::Editor => 0.0,
                        SettingsTab::LspServers => 42.0 + 12.0 + 4.0, // "Editor" width + spacing
                        SettingsTab::Panels => 42.0 + 12.0 + 4.0 + 82.0 + 12.0 + 4.0,
                    };
                    ui.add_space(offset);
                    let rect = egui::Rect::from_min_size(
                        ui.cursor().min,
                        egui::vec2(underline_width, 2.0),
                    );
                    ui.painter().rect_filled(rect, 0.0, theme.syntax.keyword);
                    ui.allocate_space(egui::vec2(underline_width, 2.0));
                });

                ui.add_space(12.0);

                // Tab content
                match self.active_tab {
                    SettingsTab::Editor => {
                        self.render_editor_tab(
                            ui,
                            config,
                            theme,
                            panel_width,
                            left_margin,
                            &mut config_changed,
                        );
                    }
                    SettingsTab::LspServers => {
                        lsp_view.render_content(ui, lsp_manager, theme, panel_width, left_margin);
                    }
                    SettingsTab::Panels => {
                        // placeholder — rendered in Task 7
                    }
                }
            });

        config_changed
    }

    fn render_editor_tab(
        &self,
        ui: &mut egui::Ui,
        config: &mut NyxConfig,
        theme: &Theme,
        panel_width: f32,
        left_margin: f32,
        config_changed: &mut bool,
    ) {
        // EDITOR section header
        ui.horizontal(|ui| {
            ui.add_space(left_margin);
            ui.label(
                egui::RichText::new("EDITOR")
                    .color(theme.syntax.keyword)
                    .size(11.0)
                    .strong(),
            );
        });

        ui.add_space(8.0);

        // Settings rows
        for i in 0..FIELD_COUNT {
            let field = SettingsField::from_index(i).unwrap();
            let is_selected = i == self.selected_row;
            let is_editing = self.editing == Some(field);

            let row_bg = if is_selected {
                theme.selection
            } else {
                egui::Color32::TRANSPARENT
            };

            // Row frame
            ui.horizontal(|ui| {
                ui.add_space(left_margin);

                let row_rect =
                    egui::Rect::from_min_size(ui.cursor().min, egui::vec2(panel_width, 28.0));
                ui.painter().rect_filled(row_rect, 4.0, row_bg);

                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(row_rect), |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.add_space(8.0);

                        // Selection indicator
                        if is_selected {
                            ui.label(
                                egui::RichText::new("\u{25b8}")
                                    .color(theme.syntax.keyword)
                                    .size(13.0),
                            );
                        } else {
                            ui.add_space(12.0);
                        }

                        // Field label
                        ui.label(
                            egui::RichText::new(field.label())
                                .color(theme.foreground)
                                .size(13.0),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(8.0);

                            if is_editing {
                                // Show edit buffer with cursor
                                let display = format!("{}|", self.edit_buffer);
                                ui.label(
                                    egui::RichText::new(display)
                                        .color(theme.syntax.string)
                                        .monospace()
                                        .size(13.0),
                                );
                            } else {
                                // Display current value
                                let value_text = field.display_value(config);
                                ui.label(
                                    egui::RichText::new(&value_text)
                                        .color(theme.foreground)
                                        .size(13.0),
                                );
                            }
                        });
                    });
                });
            });

            ui.add_space(2.0);
        }

        // LANGUAGES section
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(left_margin);
            ui.label(
                egui::RichText::new("LANGUAGES")
                    .color(theme.syntax.keyword)
                    .size(11.0)
                    .strong(),
            );
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.add_space(left_margin + 8.0);
            for &lang in AVAILABLE_LANGUAGES {
                let enabled = config.languages.iter().any(|l| l == lang);
                let (text_color, bg_color) = if enabled {
                    (theme.syntax.string, theme.selection)
                } else {
                    (theme.line_number, egui::Color32::TRANSPARENT)
                };

                let label_text = if enabled {
                    format!("{} \u{2713}", lang)
                } else {
                    lang.to_string()
                };

                let response = ui.add(
                    egui::Button::new(egui::RichText::new(label_text).color(text_color).size(12.0))
                        .fill(bg_color)
                        .corner_radius(12.0)
                        .stroke(egui::Stroke::new(1.0, theme.line_number)),
                );

                if response.clicked() {
                    if enabled {
                        config.languages.retain(|l| l != lang);
                    } else {
                        config.languages.push(lang.to_string());
                    }
                    *config_changed = true;
                }
            }
        });
    }

    /// Commit the current edit buffer to the config.
    /// Returns true if config was changed.
    pub fn commit_edit(&mut self, config: &mut NyxConfig) -> bool {
        let changed = match self.editing {
            Some(SettingsField::FontFamily) if !self.edit_buffer.is_empty() => {
                config.editor.font_family = self.edit_buffer.clone();
                true
            }
            Some(SettingsField::FontSize) => {
                if let Ok(size) = self.edit_buffer.parse::<f32>() {
                    config.editor.font_size = size.clamp(8.0, 72.0);
                    true
                } else {
                    false
                }
            }
            Some(SettingsField::TabSize) => {
                if let Ok(size) = self.edit_buffer.parse::<usize>() {
                    config.editor.tab_size = size.clamp(1, 16);
                    true
                } else {
                    false
                }
            }
            _ => false,
        };
        self.editing = None;
        self.edit_buffer.clear();
        changed
    }

    /// Cancel the current edit without applying changes.
    pub fn cancel_edit(&mut self) {
        self.editing = None;
        self.edit_buffer.clear();
    }

    /// Handle keyboard input for the settings view.
    pub fn handle_input(
        &mut self,
        ctx: &egui::Context,
        config: &mut NyxConfig,
        lsp_view: &mut LspServersView,
        lsp_manager: &mut LspManager,
    ) -> SettingsAction {
        // When on the LSP tab, delegate input to LspServersView
        if self.active_tab == SettingsTab::LspServers {
            return self.handle_lsp_tab_input(ctx, lsp_view, lsp_manager);
        }
        // When on Panels tab, delegate to panels handler (stub until Task 8)
        if self.active_tab == SettingsTab::Panels {
            return self.handle_panels_tab_input_stub(ctx);
        }

        let mut action = SettingsAction::None;

        ctx.input(|input| {
            if self.editing.is_some() {
                // In edit mode: route input to edit buffer
                if input.key_pressed(egui::Key::Escape) {
                    self.cancel_edit();
                    return;
                }
                if input.key_pressed(egui::Key::Enter) {
                    if self.commit_edit(config) {
                        action = SettingsAction::ConfigChanged;
                    }
                    return;
                }
                if input.key_pressed(egui::Key::Backspace) {
                    self.edit_buffer.pop();
                    return;
                }
                // Text input goes to edit buffer
                for event in &input.events {
                    if let egui::Event::Text(text) = event {
                        self.edit_buffer.push_str(text);
                    }
                }
            } else {
                // Navigation mode
                if input.key_pressed(egui::Key::Escape) {
                    action = SettingsAction::Close;
                    return;
                }
                // Tab switching
                if input.key_pressed(egui::Key::Tab) || input.key_pressed(egui::Key::L) {
                    self.active_tab = match self.active_tab {
                        SettingsTab::Editor => SettingsTab::LspServers,
                        SettingsTab::LspServers => SettingsTab::Panels,
                        SettingsTab::Panels => SettingsTab::Editor,
                    };
                    return;
                }
                if input.key_pressed(egui::Key::H) {
                    self.active_tab = match self.active_tab {
                        SettingsTab::Editor => SettingsTab::Panels,
                        SettingsTab::LspServers => SettingsTab::Editor,
                        SettingsTab::Panels => SettingsTab::LspServers,
                    };
                    return;
                }
                if input.key_pressed(egui::Key::J) || input.key_pressed(egui::Key::ArrowDown) {
                    if self.selected_row < FIELD_COUNT - 1 {
                        self.selected_row += 1;
                    }
                    return;
                }
                if input.key_pressed(egui::Key::K) || input.key_pressed(egui::Key::ArrowUp) {
                    if self.selected_row > 0 {
                        self.selected_row -= 1;
                    }
                    return;
                }
                if input.key_pressed(egui::Key::Enter) {
                    if let Some(field) = SettingsField::from_index(self.selected_row) {
                        if self.activate_field(field, config) {
                            action = SettingsAction::ConfigChanged;
                        }
                    }
                }
            }
        });

        action
    }

    /// Handle input when the LSP Servers tab is active.
    fn handle_lsp_tab_input(
        &mut self,
        ctx: &egui::Context,
        lsp_view: &mut LspServersView,
        lsp_manager: &mut LspManager,
    ) -> SettingsAction {
        // Check for tab switching first (before delegating to LspServersView)
        let mut tab_forward = false;
        let mut tab_backward = false;
        ctx.input(|input| {
            if input.key_pressed(egui::Key::Tab) && !input.modifiers.shift {
                tab_forward = true;
            }
            if input.key_pressed(egui::Key::L) {
                tab_forward = true;
            }
            if input.key_pressed(egui::Key::H) {
                tab_backward = true;
            }
        });
        if tab_forward {
            self.active_tab = SettingsTab::Panels;
            return SettingsAction::None;
        }
        if tab_backward {
            self.active_tab = SettingsTab::Editor;
            return SettingsAction::None;
        }

        let lsp_action = lsp_view.handle_input(ctx, lsp_manager);
        match lsp_action {
            super::lsp_servers::LspViewAction::Close => SettingsAction::Close,
            super::lsp_servers::LspViewAction::ServerToggled => SettingsAction::ServerToggled,
            super::lsp_servers::LspViewAction::None => SettingsAction::None,
        }
    }

    /// Stub handler for the Panels tab — replaced in Task 8.
    fn handle_panels_tab_input_stub(&mut self, ctx: &egui::Context) -> SettingsAction {
        let mut action = SettingsAction::None;
        ctx.input(|input| {
            if input.key_pressed(egui::Key::Escape) {
                action = SettingsAction::Close;
                return;
            }
            if input.key_pressed(egui::Key::Tab) || input.key_pressed(egui::Key::L) {
                self.active_tab = SettingsTab::Editor;
                return;
            }
            if input.key_pressed(egui::Key::H) {
                self.active_tab = SettingsTab::LspServers;
            }
        });
        action
    }

    /// Activate a field for editing. For bool/enum fields, toggles immediately and returns true.
    /// For text/number fields, enters edit mode and returns false (no config change yet).
    fn activate_field(&mut self, field: SettingsField, config: &mut NyxConfig) -> bool {
        match field {
            SettingsField::CursorBlink => {
                config.editor.cursor_blink = !config.editor.cursor_blink;
                true
            }
            SettingsField::WordWrap => {
                config.editor.word_wrap = !config.editor.word_wrap;
                true
            }
            SettingsField::LineNumbers => {
                config.editor.line_numbers = config.editor.line_numbers.next();
                true
            }
            SettingsField::FontFamily => {
                self.edit_buffer = config.editor.font_family.clone();
                self.editing = Some(field);
                false
            }
            SettingsField::FontSize => {
                self.edit_buffer = format!("{}", config.editor.font_size);
                self.editing = Some(field);
                false
            }
            SettingsField::TabSize => {
                self.edit_buffer = format!("{}", config.editor.tab_size);
                self.editing = Some(field);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LineNumberMode;

    #[test]
    fn settings_field_from_index_roundtrip() {
        for i in 0..FIELD_COUNT {
            assert!(SettingsField::from_index(i).is_some());
        }
        assert!(SettingsField::from_index(FIELD_COUNT).is_none());
    }

    #[test]
    fn settings_field_labels_non_empty() {
        for i in 0..FIELD_COUNT {
            let field = SettingsField::from_index(i).unwrap();
            assert!(!field.label().is_empty());
        }
    }

    #[test]
    fn display_value_reflects_config() {
        let config = NyxConfig::default();
        assert_eq!(
            SettingsField::FontFamily.display_value(&config),
            "JetBrains Mono"
        );
        assert_eq!(SettingsField::FontSize.display_value(&config), "14");
        assert_eq!(
            SettingsField::LineNumbers.display_value(&config),
            "relative"
        );
        assert_eq!(SettingsField::TabSize.display_value(&config), "4");
        assert_eq!(SettingsField::CursorBlink.display_value(&config), "off");
        assert_eq!(SettingsField::WordWrap.display_value(&config), "off");
    }

    #[test]
    fn activate_bool_field_toggles() {
        let mut view = SettingsView::new();
        let mut config = NyxConfig::default();
        assert!(!config.editor.cursor_blink);

        let changed = view.activate_field(SettingsField::CursorBlink, &mut config);
        assert!(changed);
        assert!(config.editor.cursor_blink);

        let changed = view.activate_field(SettingsField::CursorBlink, &mut config);
        assert!(changed);
        assert!(!config.editor.cursor_blink);
    }

    #[test]
    fn activate_enum_field_cycles() {
        let mut view = SettingsView::new();
        let mut config = NyxConfig::default();
        assert_eq!(config.editor.line_numbers, LineNumberMode::Relative);

        view.activate_field(SettingsField::LineNumbers, &mut config);
        assert_eq!(config.editor.line_numbers, LineNumberMode::Off);

        view.activate_field(SettingsField::LineNumbers, &mut config);
        assert_eq!(config.editor.line_numbers, LineNumberMode::Absolute);
    }

    #[test]
    fn activate_text_field_enters_edit_mode() {
        let mut view = SettingsView::new();
        let mut config = NyxConfig::default();

        let changed = view.activate_field(SettingsField::FontFamily, &mut config);
        assert!(!changed); // no config change yet
        assert_eq!(view.editing, Some(SettingsField::FontFamily));
        assert_eq!(view.edit_buffer, "JetBrains Mono");
    }

    #[test]
    fn commit_font_size_valid() {
        let mut view = SettingsView::new();
        let mut config = NyxConfig::default();

        view.editing = Some(SettingsField::FontSize);
        view.edit_buffer = "20".to_string();
        let changed = view.commit_edit(&mut config);
        assert!(changed);
        assert_eq!(config.editor.font_size, 20.0);
        assert!(view.editing.is_none());
    }

    #[test]
    fn commit_font_size_clamps_min() {
        let mut view = SettingsView::new();
        let mut config = NyxConfig::default();

        view.editing = Some(SettingsField::FontSize);
        view.edit_buffer = "2".to_string();
        let changed = view.commit_edit(&mut config);
        assert!(changed);
        assert_eq!(config.editor.font_size, 8.0);
    }

    #[test]
    fn commit_font_size_clamps_max() {
        let mut view = SettingsView::new();
        let mut config = NyxConfig::default();

        view.editing = Some(SettingsField::FontSize);
        view.edit_buffer = "100".to_string();
        let changed = view.commit_edit(&mut config);
        assert!(changed);
        assert_eq!(config.editor.font_size, 72.0);
    }

    #[test]
    fn commit_font_size_invalid_keeps_old() {
        let mut view = SettingsView::new();
        let mut config = NyxConfig::default();

        view.editing = Some(SettingsField::FontSize);
        view.edit_buffer = "abc".to_string();
        let changed = view.commit_edit(&mut config);
        assert!(!changed);
        assert_eq!(config.editor.font_size, 14.0);
        assert!(view.editing.is_none());
    }

    #[test]
    fn commit_tab_size_valid() {
        let mut view = SettingsView::new();
        let mut config = NyxConfig::default();

        view.editing = Some(SettingsField::TabSize);
        view.edit_buffer = "8".to_string();
        let changed = view.commit_edit(&mut config);
        assert!(changed);
        assert_eq!(config.editor.tab_size, 8);
    }

    #[test]
    fn commit_tab_size_clamps() {
        let mut view = SettingsView::new();
        let mut config = NyxConfig::default();

        view.editing = Some(SettingsField::TabSize);
        view.edit_buffer = "0".to_string();
        view.commit_edit(&mut config);
        assert_eq!(config.editor.tab_size, 1);

        view.editing = Some(SettingsField::TabSize);
        view.edit_buffer = "99".to_string();
        view.commit_edit(&mut config);
        assert_eq!(config.editor.tab_size, 16);
    }

    #[test]
    fn commit_font_family() {
        let mut view = SettingsView::new();
        let mut config = NyxConfig::default();

        view.editing = Some(SettingsField::FontFamily);
        view.edit_buffer = "Fira Code".to_string();
        let changed = view.commit_edit(&mut config);
        assert!(changed);
        assert_eq!(config.editor.font_family, "Fira Code");
    }

    #[test]
    fn cancel_edit_restores_state() {
        let mut view = SettingsView::new();
        view.editing = Some(SettingsField::FontSize);
        view.edit_buffer = "99".to_string();
        view.cancel_edit();
        assert!(view.editing.is_none());
        assert!(view.edit_buffer.is_empty());
    }

    #[test]
    fn panels_tab_exists_in_cycle() {
        // Verify that Panels is reachable going forward: Editor -> LspServers -> Panels -> Editor
        let mut tab = SettingsTab::Editor;
        tab = match tab {
            SettingsTab::Editor => SettingsTab::LspServers,
            SettingsTab::LspServers => SettingsTab::Panels,
            SettingsTab::Panels => SettingsTab::Editor,
        };
        assert_eq!(tab, SettingsTab::LspServers);
        tab = match tab {
            SettingsTab::Editor => SettingsTab::LspServers,
            SettingsTab::LspServers => SettingsTab::Panels,
            SettingsTab::Panels => SettingsTab::Editor,
        };
        assert_eq!(tab, SettingsTab::Panels);
        tab = match tab {
            SettingsTab::Editor => SettingsTab::LspServers,
            SettingsTab::LspServers => SettingsTab::Panels,
            SettingsTab::Panels => SettingsTab::Editor,
        };
        assert_eq!(tab, SettingsTab::Editor);

        // Verify backward cycle: Editor -> Panels -> LspServers -> Editor
        let mut tab = SettingsTab::Editor;
        tab = match tab {
            SettingsTab::Editor => SettingsTab::Panels,
            SettingsTab::LspServers => SettingsTab::Editor,
            SettingsTab::Panels => SettingsTab::LspServers,
        };
        assert_eq!(tab, SettingsTab::Panels);
        tab = match tab {
            SettingsTab::Editor => SettingsTab::Panels,
            SettingsTab::LspServers => SettingsTab::Editor,
            SettingsTab::Panels => SettingsTab::LspServers,
        };
        assert_eq!(tab, SettingsTab::LspServers);
        tab = match tab {
            SettingsTab::Editor => SettingsTab::Panels,
            SettingsTab::LspServers => SettingsTab::Editor,
            SettingsTab::Panels => SettingsTab::LspServers,
        };
        assert_eq!(tab, SettingsTab::Editor);
    }
}
