use crate::renderer::Theme;
use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteAction {
    None,
    ToggleFiletree,
    OpenSettings,
    OpenKeybindings,
    OpenLspServers,
}

struct PaletteEntry {
    label: &'static str,
    description: &'static str,
    action: PaletteAction,
}

pub struct CommandPalette {
    filter: String,
    selected: usize,
    commands: Vec<PaletteEntry>,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            filter: String::new(),
            selected: 0,
            commands: vec![
                PaletteEntry {
                    label: "Toggle File Explorer",
                    description: "Show or hide the file tree panel",
                    action: PaletteAction::ToggleFiletree,
                },
                PaletteEntry {
                    label: "Open Settings",
                    description: "Open the settings view",
                    action: PaletteAction::OpenSettings,
                },
                PaletteEntry {
                    label: "Open Keybindings",
                    description: "Show keyboard shortcuts",
                    action: PaletteAction::OpenKeybindings,
                },
                PaletteEntry {
                    label: "LSP Servers",
                    description: "Manage language server configurations",
                    action: PaletteAction::OpenLspServers,
                },
            ],
        }
    }

    pub fn reset(&mut self) {
        self.filter.clear();
        self.selected = 0;
    }

    fn filtered(&self) -> Vec<&PaletteEntry> {
        if self.filter.is_empty() {
            return self.commands.iter().collect();
        }
        let query = self.filter.to_lowercase();
        self.commands
            .iter()
            .filter(|e| {
                e.label.to_lowercase().contains(&query)
                    || e.description.to_lowercase().contains(&query)
            })
            .collect()
    }

    /// Handle input for the command palette.
    /// Returns `(should_close, action)`.
    pub fn handle_input(&mut self, ctx: &egui::Context) -> (bool, PaletteAction) {
        let mut should_close = false;
        let mut action = PaletteAction::None;

        ctx.input(|input| {
            if input.key_pressed(egui::Key::Escape) {
                should_close = true;
                return;
            }
            if input.key_pressed(egui::Key::ArrowDown) || input.key_pressed(egui::Key::J) {
                let count = self.filtered().len();
                if count > 0 && self.selected < count - 1 {
                    self.selected += 1;
                }
                return;
            }
            if input.key_pressed(egui::Key::ArrowUp) || input.key_pressed(egui::Key::K) {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                return;
            }
            if input.key_pressed(egui::Key::Enter) {
                let filtered = self.filtered();
                if let Some(entry) = filtered.get(self.selected) {
                    action = entry.action;
                }
                should_close = true;
                return;
            }
            if input.key_pressed(egui::Key::Backspace) {
                self.filter.pop();
                self.selected = 0;
                return;
            }
            // Text input
            for event in &input.events {
                if let egui::Event::Text(text) = event {
                    if !input.modifiers.command && !input.modifiers.ctrl {
                        self.filter.push_str(text);
                        self.selected = 0;
                    }
                }
            }
        });

        (should_close, action)
    }

    /// Render the command palette overlay.
    pub fn render(&self, ctx: &egui::Context, theme: &Theme) {
        let screen = ctx.screen_rect();

        // Dim layer
        egui::Area::new(egui::Id::new("palette_dim"))
            .fixed_pos(screen.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.painter()
                    .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(160));
            });

        // Palette overlay — top-center, like VS Code
        let palette_width = (screen.width() * 0.5).clamp(300.0, 600.0);
        let palette_x = (screen.width() - palette_width) / 2.0;
        let palette_y = screen.height() * 0.15;

        egui::Area::new(egui::Id::new("palette_overlay"))
            .fixed_pos(egui::pos2(palette_x, palette_y))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::default()
                    .fill(theme.status_bar_bg)
                    .stroke(egui::Stroke::new(1.0, theme.line_number))
                    .corner_radius(8.0)
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.set_width(palette_width);

                        // Search input display
                        let search_display = if self.filter.is_empty() {
                            egui::RichText::new("Type a command...")
                                .color(theme.line_number)
                                .italics()
                                .size(14.0)
                        } else {
                            egui::RichText::new(format!("> {}", self.filter))
                                .color(theme.foreground)
                                .size(14.0)
                        };
                        ui.label(search_display);
                        ui.separator();

                        // Command list
                        let filtered = self.filtered();
                        if filtered.is_empty() {
                            ui.label(
                                egui::RichText::new("No matching commands")
                                    .color(theme.line_number)
                                    .size(13.0),
                            );
                        } else {
                            for (idx, entry) in filtered.iter().enumerate() {
                                let is_selected = idx == self.selected;
                                let (rect, _response) = ui.allocate_exact_size(
                                    egui::vec2(ui.available_width(), 28.0),
                                    egui::Sense::hover(),
                                );

                                if is_selected {
                                    ui.painter().rect_filled(rect, 4.0, theme.selection);
                                }

                                // Label
                                ui.painter().text(
                                    egui::pos2(rect.min.x + 8.0, rect.min.y + 3.0),
                                    egui::Align2::LEFT_TOP,
                                    entry.label,
                                    egui::FontId::monospace(13.0),
                                    theme.foreground,
                                );

                                // Description (right-aligned)
                                ui.painter().text(
                                    egui::pos2(rect.max.x - 8.0, rect.min.y + 5.0),
                                    egui::Align2::RIGHT_TOP,
                                    entry.description,
                                    egui::FontId::monospace(11.0),
                                    theme.line_number,
                                );
                            }
                        }
                    });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_palette_has_commands() {
        let p = CommandPalette::new();
        assert!(!p.commands.is_empty());
    }

    #[test]
    fn filter_narrows_results() {
        let mut p = CommandPalette::new();
        p.filter = "file".to_string();
        let filtered = p.filtered();
        assert!(filtered.len() < p.commands.len());
        assert!(filtered.iter().any(|e| e.label.contains("File")));
    }

    #[test]
    fn empty_filter_shows_all() {
        let p = CommandPalette::new();
        assert_eq!(p.filtered().len(), p.commands.len());
    }

    #[test]
    fn reset_clears_state() {
        let mut p = CommandPalette::new();
        p.filter = "test".to_string();
        p.selected = 2;
        p.reset();
        assert!(p.filter.is_empty());
        assert_eq!(p.selected, 0);
    }
}
