// src/views/lsp_servers.rs
//! LSP Servers management view (Cmd+L).

use crate::lsp::registry::{ServerRegistry, ServerStatus, KNOWN_SERVERS};
use crate::lsp::LspManager;
use crate::renderer::Theme;
use eframe::egui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspViewAction {
    None,
    Close,
    ServerToggled,
}

pub struct LspServersView {
    pub selected_row: usize,
    pub search: String,
}

impl LspServersView {
    pub fn new() -> Self {
        Self {
            selected_row: 0,
            search: String::new(),
        }
    }

    pub fn handle_input(
        &mut self,
        ctx: &egui::Context,
        lsp_manager: &mut LspManager,
    ) -> LspViewAction {
        let mut action = LspViewAction::None;
        let server_count = self.filtered_servers().len();

        ctx.input(|input| {
            if input.key_pressed(egui::Key::Escape) {
                if self.search.is_empty() {
                    action = LspViewAction::Close;
                } else {
                    self.search.clear();
                    self.selected_row = 0;
                }
                return;
            }

            // Navigation
            if input.key_pressed(egui::Key::J) || input.key_pressed(egui::Key::ArrowDown) {
                if server_count > 0 && self.selected_row < server_count - 1 {
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

            // Enter toggles enabled
            if input.key_pressed(egui::Key::Enter) {
                if let Some(server) = self.selected_server() {
                    let name = server.name;
                    let currently_enabled = lsp_manager.lsp_config.is_enabled(name);
                    let enabling = !currently_enabled;
                    lsp_manager.lsp_config.set_enabled(name, enabling);
                    lsp_manager.save_config();
                    // Auto-install if enabling and binary not found
                    if enabling && ServerRegistry::find_command(server, None).is_none() {
                        if ServerRegistry::download_url(server).is_some() {
                            lsp_manager.start_download(server);
                        } else if let Some(cmd) = server.install_command {
                            lsp_manager.start_install(server, cmd);
                        }
                    }
                    action = LspViewAction::ServerToggled;
                }
                return;
            }

            // 'd' to download
            if input.key_pressed(egui::Key::D) {
                if let Some(server) = self.selected_server() {
                    let status = lsp_manager.server_status(server);
                    if status == ServerStatus::NotInstalled
                        && ServerRegistry::download_url(server).is_some()
                    {
                        lsp_manager.start_download(server);
                    }
                }
                return;
            }

            // 'x' to uninstall
            if input.key_pressed(egui::Key::X) {
                if let Some(server) = self.selected_server() {
                    let status = lsp_manager.server_status(server);
                    if status == ServerStatus::Installed {
                        let _ = ServerRegistry::uninstall(server);
                    }
                }
                return;
            }

            // Search input (Backspace)
            if input.key_pressed(egui::Key::Backspace) {
                self.search.pop();
                self.selected_row = 0;
                return;
            }

            // Text input for search (only when not a navigation key)
            for event in &input.events {
                if let egui::Event::Text(text) = event {
                    if !input.modifiers.command && !input.modifiers.ctrl {
                        // Only allow search if the key isn't a navigation key
                        let ch = text.chars().next().unwrap_or(' ');
                        if ch != 'j' && ch != 'k' && ch != 'd' && ch != 'x' {
                            self.search.push_str(text);
                            self.selected_row = 0;
                        }
                    }
                }
            }
        });

        action
    }

    fn filtered_servers(&self) -> Vec<&'static crate::lsp::registry::KnownServer> {
        if self.search.is_empty() {
            return KNOWN_SERVERS.iter().collect();
        }
        let query = self.search.to_lowercase();
        KNOWN_SERVERS
            .iter()
            .filter(|s| {
                s.name.to_lowercase().contains(&query)
                    || s.language_ids
                        .iter()
                        .any(|l| l.to_lowercase().contains(&query))
            })
            .collect()
    }

    fn selected_server(&self) -> Option<&'static crate::lsp::registry::KnownServer> {
        let filtered = self.filtered_servers();
        filtered.get(self.selected_row).copied()
    }

    /// Render the LSP servers content into an existing Ui (for embedding in Settings).
    pub fn render_content(
        &self,
        ui: &mut egui::Ui,
        lsp_manager: &LspManager,
        theme: &Theme,
        panel_width: f32,
        left_margin: f32,
    ) {
        // Error message (if any)
        if let Some(ref error) = lsp_manager.last_error {
            ui.horizontal(|ui| {
                ui.add_space(left_margin);
                ui.label(
                    egui::RichText::new(error)
                        .color(theme.syntax.number) // peach/error color
                        .size(12.0),
                );
            });
            ui.add_space(8.0);
        }

        // Search
        ui.horizontal(|ui| {
            ui.add_space(left_margin);
            let search_display = if self.search.is_empty() {
                egui::RichText::new(
                    "Type to search... | Enter: toggle | d: download | x: uninstall",
                )
                .color(theme.line_number)
                .italics()
                .size(12.0)
            } else {
                egui::RichText::new(format!("Search: {}", self.search))
                    .color(theme.foreground)
                    .size(12.0)
            };
            ui.label(search_display);
        });

        ui.add_space(12.0);

        // Column headers
        ui.horizontal(|ui| {
            ui.add_space(left_margin + 28.0);
            ui.label(
                egui::RichText::new("SERVER")
                    .color(theme.syntax.keyword)
                    .size(11.0)
                    .strong(),
            );
        });

        ui.add_space(8.0);

        // Download progress (if active)
        if let Some(progress) = lsp_manager.download_progress() {
            ui.horizontal(|ui| {
                ui.add_space(left_margin + 8.0);
                let is_download = progress.total_bytes.is_some() || progress.bytes_downloaded > 0;
                let text = if let Some(err) = &progress.error {
                    format!("Error: {}", err)
                } else if progress.finished {
                    format!("{} installed successfully!", progress.server_name)
                } else if is_download {
                    if let Some(pct) = progress.percent() {
                        format!("Downloading {}... {:.0}%", progress.server_name, pct)
                    } else {
                        format!(
                            "Downloading {}... {} bytes",
                            progress.server_name, progress.bytes_downloaded
                        )
                    }
                } else {
                    format!("Installing {}...", progress.server_name)
                };
                let color = if progress.error.is_some() {
                    theme.syntax.number
                } else if progress.finished {
                    theme.syntax.string
                } else {
                    theme.foreground
                };
                ui.label(egui::RichText::new(text).color(color).size(12.0));
            });
            ui.add_space(8.0);
        }

        // Server list
        let filtered = self.filtered_servers();

        if filtered.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(left_margin);
                ui.label(
                    egui::RichText::new("No matching servers")
                        .color(theme.line_number)
                        .size(13.0),
                );
            });
        } else {
            for (i, server) in filtered.iter().enumerate() {
                let is_selected = i == self.selected_row;
                let status = lsp_manager.server_status(server);
                let enabled = lsp_manager.lsp_config.is_enabled(server.name);

                let row_bg = if is_selected {
                    theme.selection
                } else {
                    egui::Color32::TRANSPARENT
                };

                ui.horizontal(|ui| {
                    ui.add_space(left_margin);

                    let row_rect =
                        egui::Rect::from_min_size(ui.cursor().min, egui::vec2(panel_width, 32.0));
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

                            // Enabled checkbox
                            let check = if enabled { "\u{2713}" } else { " " };
                            let check_color = if enabled {
                                theme.syntax.string
                            } else {
                                theme.line_number
                            };
                            ui.label(
                                egui::RichText::new(format!("[{}]", check))
                                    .color(check_color)
                                    .monospace()
                                    .size(13.0),
                            );

                            // Server name
                            ui.label(
                                egui::RichText::new(server.name)
                                    .color(theme.foreground)
                                    .size(13.0),
                            );

                            // Languages
                            let langs = server.language_ids.join(", ");
                            ui.label(
                                egui::RichText::new(format!("({})", langs))
                                    .color(theme.line_number)
                                    .size(12.0),
                            );

                            // Status (right-aligned)
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.add_space(8.0);
                                    let (status_text, status_color) = match status {
                                        ServerStatus::Running => ("running", theme.syntax.string),
                                        ServerStatus::Installed => {
                                            ("installed", theme.syntax.function)
                                        }
                                        ServerStatus::NotInstalled => {
                                            ("not installed", theme.line_number)
                                        }
                                        ServerStatus::Error => ("error", theme.syntax.number),
                                    };
                                    ui.label(
                                        egui::RichText::new(status_text)
                                            .color(status_color)
                                            .size(12.0),
                                    );
                                },
                            );
                        });
                    });
                });

                // Show install hint for selected server when not installed and no auto-install
                if is_selected
                    && status == ServerStatus::NotInstalled
                    && ServerRegistry::download_url(server).is_none()
                    && server.install_command.is_none()
                {
                    if let Some(hint) = server.install_hint {
                        ui.horizontal(|ui| {
                            ui.add_space(left_margin + 48.0);
                            ui.label(
                                egui::RichText::new(format!("Install: {}", hint))
                                    .color(theme.line_number)
                                    .italics()
                                    .size(11.0),
                            );
                        });
                    }
                }

                ui.add_space(2.0);
            }
        }
    }
}
