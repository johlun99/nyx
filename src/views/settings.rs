use crate::config::panels_config::PanelsConfig;
use crate::config::NyxConfig;
use crate::lsp::LspManager;
use crate::renderer::Theme;
use crate::views::lsp_servers::LspServersView;
use crate::views::PanelSlot;
use eframe::egui;

const FIELD_COUNT: usize = 6;
const AVAILABLE_LANGUAGES: &[&str] = &["rust", "json", "python", "javascript", "typescript"];
const KNOWN_MODULES: &[&str] = &["filetree", "terminal", "git", "search", "ai_chat"];

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
enum PanelsRowKind {
    Empty(PanelSlot),
    Tab(PanelSlot, usize),
    AddTab(PanelSlot),
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
    pub panels_selected_row: usize,
    pub panels_editing_tab: Option<(PanelSlot, usize)>,
}

impl SettingsView {
    pub fn new() -> Self {
        Self {
            selected_row: 0,
            editing: None,
            edit_buffer: String::new(),
            active_tab: SettingsTab::default(),
            panels_selected_row: 0,
            panels_editing_tab: None,
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
        panels_config: &PanelsConfig,
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
                        self.render_panels_tab(ui, theme, panel_width, left_margin, panels_config);
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

    /// Capitalize the first character of a string.
    fn capitalize(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }

    /// Render the Panels settings tab.
    fn render_panels_tab(
        &self,
        ui: &mut egui::Ui,
        theme: &Theme,
        panel_width: f32,
        left_margin: f32,
        panels_config: &PanelsConfig,
    ) {
        let slots = [PanelSlot::Left, PanelSlot::Bottom, PanelSlot::Right];
        let mut current_row: usize = 0;

        for slot in slots {
            let slot_label = match slot {
                PanelSlot::Left => "LEFT",
                PanelSlot::Bottom => "BOTTOM",
                PanelSlot::Right => "RIGHT",
            };

            // Section header
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(left_margin);
                ui.label(
                    egui::RichText::new(slot_label)
                        .color(theme.syntax.keyword)
                        .size(11.0)
                        .strong(),
                );
            });
            ui.add_space(4.0);

            let tabs = panels_config.tabs_for(slot);
            if tabs.is_empty() {
                // Empty row
                let is_selected = current_row == self.panels_selected_row;
                let row_bg = if is_selected {
                    theme.selection
                } else {
                    egui::Color32::TRANSPARENT
                };
                ui.horizontal(|ui| {
                    ui.add_space(left_margin);
                    let row_rect =
                        egui::Rect::from_min_size(ui.cursor().min, egui::vec2(panel_width, 24.0));
                    ui.painter().rect_filled(row_rect, 4.0, row_bg);
                    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(row_rect), |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.add_space(8.0);
                            if is_selected {
                                ui.label(
                                    egui::RichText::new("\u{25b8}")
                                        .color(theme.syntax.keyword)
                                        .size(12.0),
                                );
                            } else {
                                ui.add_space(12.0);
                            }
                            ui.label(
                                egui::RichText::new("(empty)")
                                    .color(theme.line_number)
                                    .size(12.0),
                            );
                        });
                    });
                });
                ui.add_space(2.0);
                current_row += 1;

                // Add tab row
                let is_selected = current_row == self.panels_selected_row;
                let row_bg = if is_selected {
                    theme.selection
                } else {
                    egui::Color32::TRANSPARENT
                };
                ui.horizontal(|ui| {
                    ui.add_space(left_margin);
                    let row_rect =
                        egui::Rect::from_min_size(ui.cursor().min, egui::vec2(panel_width, 24.0));
                    ui.painter().rect_filled(row_rect, 4.0, row_bg);
                    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(row_rect), |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.add_space(8.0);
                            if is_selected {
                                ui.label(
                                    egui::RichText::new("\u{25b8}")
                                        .color(theme.syntax.keyword)
                                        .size(12.0),
                                );
                            } else {
                                ui.add_space(12.0);
                            }
                            ui.label(
                                egui::RichText::new("+ Add tab...")
                                    .color(theme.syntax.string)
                                    .size(12.0),
                            );
                        });
                    });
                });
                ui.add_space(2.0);
                current_row += 1;
            } else {
                // Tab rows
                for (tab_idx, tab) in tabs.iter().enumerate() {
                    let is_selected = current_row == self.panels_selected_row;
                    let is_editing = self.panels_editing_tab == Some((slot, tab_idx));
                    let row_bg = if is_selected {
                        theme.selection
                    } else {
                        egui::Color32::TRANSPARENT
                    };

                    ui.horizontal(|ui| {
                        ui.add_space(left_margin);
                        let row_rect = egui::Rect::from_min_size(
                            ui.cursor().min,
                            egui::vec2(panel_width, if is_editing { 48.0 } else { 24.0 }),
                        );
                        ui.painter().rect_filled(row_rect, 4.0, row_bg);
                        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(row_rect), |ui| {
                            if is_editing {
                                // Editing: show checkboxes for each module
                                ui.vertical(|ui| {
                                    ui.add_space(4.0);
                                    ui.horizontal(|ui| {
                                        ui.add_space(8.0);
                                        ui.label(
                                            egui::RichText::new("\u{25b8}")
                                                .color(theme.syntax.keyword)
                                                .size(12.0),
                                        );
                                        ui.label(
                                            egui::RichText::new(format!("{}:", tab_idx + 1))
                                                .color(theme.foreground)
                                                .size(12.0),
                                        );
                                    });
                                    ui.horizontal(|ui| {
                                        ui.add_space(20.0);
                                        for (key_idx, &module) in KNOWN_MODULES.iter().enumerate() {
                                            let in_tab = tab.modules.iter().any(|m| m == module);
                                            let used_elsewhere =
                                                !in_tab && panels_config.has_module(module);
                                            let text = format!(
                                                "[{}] {}",
                                                if in_tab { "x" } else { " " },
                                                Self::capitalize(module)
                                            );
                                            let color = if used_elsewhere {
                                                theme.line_number
                                            } else if in_tab {
                                                theme.syntax.string
                                            } else {
                                                theme.foreground
                                            };
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "{} {}  ",
                                                    key_idx + 1,
                                                    text
                                                ))
                                                .color(color)
                                                .size(12.0),
                                            );
                                        }
                                    });
                                });
                            } else {
                                ui.horizontal_centered(|ui| {
                                    ui.add_space(8.0);
                                    if is_selected {
                                        ui.label(
                                            egui::RichText::new("\u{25b8}")
                                                .color(theme.syntax.keyword)
                                                .size(12.0),
                                        );
                                    } else {
                                        ui.add_space(12.0);
                                    }
                                    let modules_str = if tab.modules.is_empty() {
                                        "(empty tab)".to_string()
                                    } else {
                                        tab.modules
                                            .iter()
                                            .map(|m| Self::capitalize(m))
                                            .collect::<Vec<_>>()
                                            .join(", ")
                                    };
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{}: {}",
                                            tab_idx + 1,
                                            modules_str
                                        ))
                                        .color(theme.foreground)
                                        .size(12.0),
                                    );
                                });
                            }
                        });
                    });
                    ui.add_space(2.0);
                    current_row += 1;
                }

                // Add tab row
                let is_selected = current_row == self.panels_selected_row;
                let row_bg = if is_selected {
                    theme.selection
                } else {
                    egui::Color32::TRANSPARENT
                };
                ui.horizontal(|ui| {
                    ui.add_space(left_margin);
                    let row_rect =
                        egui::Rect::from_min_size(ui.cursor().min, egui::vec2(panel_width, 24.0));
                    ui.painter().rect_filled(row_rect, 4.0, row_bg);
                    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(row_rect), |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.add_space(8.0);
                            if is_selected {
                                ui.label(
                                    egui::RichText::new("\u{25b8}")
                                        .color(theme.syntax.keyword)
                                        .size(12.0),
                                );
                            } else {
                                ui.add_space(12.0);
                            }
                            ui.label(
                                egui::RichText::new("+ Add tab...")
                                    .color(theme.syntax.string)
                                    .size(12.0),
                            );
                        });
                    });
                });
                ui.add_space(2.0);
                current_row += 1;
            }
        }

        // suppress unused variable warning in non-debug builds
        let _ = current_row;
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
        panels_config: &mut PanelsConfig,
    ) -> SettingsAction {
        match self.active_tab {
            SettingsTab::Editor => self.handle_editor_tab_input(ctx, config),
            SettingsTab::LspServers => self.handle_lsp_tab_input(ctx, lsp_view, lsp_manager),
            SettingsTab::Panels => self.handle_panels_tab_input(ctx, panels_config),
        }
    }

    /// Handle input when the Editor tab is active.
    fn handle_editor_tab_input(
        &mut self,
        ctx: &egui::Context,
        config: &mut NyxConfig,
    ) -> SettingsAction {
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
                    self.active_tab = SettingsTab::LspServers;
                    return;
                }
                if input.key_pressed(egui::Key::H) {
                    self.active_tab = SettingsTab::Panels;
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

    /// Count selectable rows across all panel slots.
    fn panels_total_rows(panels_config: &PanelsConfig) -> usize {
        let mut total = 0;
        for slot in [PanelSlot::Left, PanelSlot::Bottom, PanelSlot::Right] {
            let tabs = panels_config.tabs_for(slot);
            if tabs.is_empty() {
                total += 2; // empty row + add row
            } else {
                total += tabs.len() + 1; // tab rows + add row
            }
        }
        total
    }

    /// Map a row index to a PanelsRowKind.
    fn panels_row_info(panels_config: &PanelsConfig, target_row: usize) -> Option<PanelsRowKind> {
        let mut current = 0;
        for slot in [PanelSlot::Left, PanelSlot::Bottom, PanelSlot::Right] {
            let tabs = panels_config.tabs_for(slot);
            if tabs.is_empty() {
                // empty row
                if current == target_row {
                    return Some(PanelsRowKind::Empty(slot));
                }
                current += 1;
                // add row
                if current == target_row {
                    return Some(PanelsRowKind::AddTab(slot));
                }
                current += 1;
            } else {
                for (idx, _) in tabs.iter().enumerate() {
                    if current == target_row {
                        return Some(PanelsRowKind::Tab(slot, idx));
                    }
                    current += 1;
                }
                // add row
                if current == target_row {
                    return Some(PanelsRowKind::AddTab(slot));
                }
                current += 1;
            }
        }
        None
    }

    /// Handle input when the Panels tab is active.
    fn handle_panels_tab_input(
        &mut self,
        ctx: &egui::Context,
        panels_config: &mut PanelsConfig,
    ) -> SettingsAction {
        let mut action = SettingsAction::None;
        let total_rows = Self::panels_total_rows(panels_config);

        // Collect key presses first to avoid borrow issues
        let mut escape = false;
        let mut tab_forward = false;
        let mut tab_backward = false;
        let mut nav_down = false;
        let mut nav_up = false;
        let mut enter = false;
        let mut delete = false;
        let mut num_key: Option<usize> = None;

        ctx.input(|input| {
            escape = input.key_pressed(egui::Key::Escape);
            tab_forward = input.key_pressed(egui::Key::Tab) || input.key_pressed(egui::Key::L);
            tab_backward = input.key_pressed(egui::Key::H);
            nav_down = input.key_pressed(egui::Key::J) || input.key_pressed(egui::Key::ArrowDown);
            nav_up = input.key_pressed(egui::Key::K) || input.key_pressed(egui::Key::ArrowUp);
            enter = input.key_pressed(egui::Key::Enter);
            delete = input.key_pressed(egui::Key::D);
            if input.key_pressed(egui::Key::Num1) {
                num_key = Some(0);
            } else if input.key_pressed(egui::Key::Num2) {
                num_key = Some(1);
            } else if input.key_pressed(egui::Key::Num3) {
                num_key = Some(2);
            } else if input.key_pressed(egui::Key::Num4) {
                num_key = Some(3);
            } else if input.key_pressed(egui::Key::Num5) {
                num_key = Some(4);
            }
        });

        if escape {
            if self.panels_editing_tab.is_some() {
                self.panels_editing_tab = None;
            } else {
                action = SettingsAction::Close;
            }
            return action;
        }

        // Number key: toggle module in edit mode
        if let (Some(module_idx), Some((slot, tab_idx))) = (num_key, self.panels_editing_tab) {
            if let Some(&module) = KNOWN_MODULES.get(module_idx) {
                let tabs = panels_config.tabs_for(slot);
                if let Some(tab) = tabs.get(tab_idx) {
                    if tab.modules.iter().any(|m| m == module) {
                        panels_config.remove_module(slot, tab_idx, module);
                        // After remove_module, the tab may have been deleted if it was the last module.
                        // Check if tab still exists; if not, exit edit mode.
                        if panels_config.tabs_for(slot).get(tab_idx).is_none() {
                            self.panels_editing_tab = None;
                            // clamp selected row
                            let new_total = Self::panels_total_rows(panels_config);
                            if self.panels_selected_row >= new_total && new_total > 0 {
                                self.panels_selected_row = new_total - 1;
                            }
                        }
                    } else {
                        panels_config.add_module(slot, tab_idx, module);
                    }
                }
            }
            return SettingsAction::ConfigChanged;
        }

        // Tab switching only when NOT in editing mode (for H/L)
        if self.panels_editing_tab.is_none() {
            if tab_forward {
                self.active_tab = SettingsTab::Editor;
                return SettingsAction::None;
            }
            if tab_backward {
                self.active_tab = SettingsTab::LspServers;
                return SettingsAction::None;
            }
        } else if tab_forward && !tab_backward {
            // Tab key in edit mode: just ignore (don't switch tabs)
        }

        // Navigation
        if nav_down {
            if self.panels_selected_row + 1 < total_rows {
                self.panels_selected_row += 1;
            }
            return action;
        }
        if nav_up {
            if self.panels_selected_row > 0 {
                self.panels_selected_row -= 1;
            }
            return action;
        }

        // Enter: activate selected row
        if enter {
            match Self::panels_row_info(panels_config, self.panels_selected_row) {
                Some(PanelsRowKind::Tab(slot, idx)) => {
                    self.panels_editing_tab = Some((slot, idx));
                }
                Some(PanelsRowKind::AddTab(slot)) => {
                    panels_config.add_tab(slot);
                    let new_tab_idx = panels_config.tabs_for(slot).len() - 1;
                    self.panels_editing_tab = Some((slot, new_tab_idx));
                    action = SettingsAction::ConfigChanged;
                }
                Some(PanelsRowKind::Empty(_)) | None => {}
            }
            return action;
        }

        // D: delete tab row (not in edit mode)
        if delete && self.panels_editing_tab.is_none() {
            if let Some(PanelsRowKind::Tab(slot, idx)) =
                Self::panels_row_info(panels_config, self.panels_selected_row)
            {
                panels_config.remove_tab(slot, idx);
                let new_total = Self::panels_total_rows(panels_config);
                if self.panels_selected_row >= new_total && new_total > 0 {
                    self.panels_selected_row = new_total - 1;
                }
                action = SettingsAction::ConfigChanged;
            }
        }

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

    #[test]
    fn panels_add_tab_to_empty_panel() {
        let mut panels_config = PanelsConfig {
            left: vec![],
            bottom: vec![],
            right: vec![],
        };
        assert!(panels_config.is_empty(PanelSlot::Bottom));

        // The "Add tab..." row for an empty panel is row index 1 (empty row = 0, add = 1)
        // For Left slot: rows 0=empty, 1=add
        // For Bottom slot: rows 2=empty, 3=add
        // For Right slot: rows 4=empty, 5=add
        let row_info = SettingsView::panels_row_info(&panels_config, 1);
        assert_eq!(row_info, Some(PanelsRowKind::AddTab(PanelSlot::Left)));

        // Simulate Enter on "Add tab..." for Left
        panels_config.add_tab(PanelSlot::Left);
        assert_eq!(panels_config.left.len(), 1);
        assert!(panels_config.left[0].modules.is_empty());
    }

    #[test]
    fn panels_remove_tab() {
        use crate::config::panels_config::PanelTab;
        let mut panels_config = PanelsConfig {
            left: vec![
                PanelTab {
                    modules: vec!["filetree".into()],
                },
                PanelTab {
                    modules: vec!["git".into()],
                },
            ],
            bottom: vec![],
            right: vec![],
        };

        // Row 0 = Tab(Left, 0), Row 1 = Tab(Left, 1), Row 2 = AddTab(Left)
        assert_eq!(
            SettingsView::panels_row_info(&panels_config, 0),
            Some(PanelsRowKind::Tab(PanelSlot::Left, 0))
        );
        assert_eq!(
            SettingsView::panels_row_info(&panels_config, 1),
            Some(PanelsRowKind::Tab(PanelSlot::Left, 1))
        );

        // Remove tab at index 0
        panels_config.remove_tab(PanelSlot::Left, 0);
        assert_eq!(panels_config.left.len(), 1);
        assert_eq!(panels_config.left[0].modules, vec!["git"]);

        // Now only 1 tab remains: Row 0 = Tab(Left, 0), Row 1 = AddTab(Left)
        assert_eq!(
            SettingsView::panels_row_info(&panels_config, 0),
            Some(PanelsRowKind::Tab(PanelSlot::Left, 0))
        );
        assert_eq!(
            SettingsView::panels_row_info(&panels_config, 1),
            Some(PanelsRowKind::AddTab(PanelSlot::Left))
        );
    }

    #[test]
    fn panels_toggle_module() {
        let mut panels_config = PanelsConfig::default();
        // Default: left has filetree tab
        assert_eq!(panels_config.left.len(), 1);
        assert_eq!(panels_config.left[0].modules, vec!["filetree"]);

        // Add "git" module to left tab 0
        panels_config.add_module(PanelSlot::Left, 0, "git");
        assert_eq!(panels_config.left[0].modules, vec!["filetree", "git"]);
        assert!(panels_config.has_module("git"));

        // Remove "git" — tab should still exist since "filetree" is there
        panels_config.remove_module(PanelSlot::Left, 0, "git");
        assert_eq!(panels_config.left[0].modules, vec!["filetree"]);
        assert!(!panels_config.has_module("git"));

        // "filetree" is in left tab, so adding it to a different tab is a no-op
        panels_config.add_tab(PanelSlot::Right);
        panels_config.add_module(PanelSlot::Right, 0, "filetree");
        assert!(panels_config.right[0].modules.is_empty());
    }
}
