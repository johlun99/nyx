// src/renderer/status_bar.rs
use crate::renderer::theme::Theme;
use crate::vim::mode::Mode;
use eframe::egui;

pub struct StatusBar;

impl StatusBar {
    /// Renders the status bar at the bottom. Returns the height consumed.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        painter: &egui::Painter,
        rect: egui::Rect,
        theme: &Theme,
        font_id: &egui::FontId,
        line_height: f32,
        mode: Mode,
        file_path: Option<&str>,
        command_input: Option<&str>,
        status_message: Option<&str>,
        lsp_health: Option<&str>,
        error_count: usize,
        warning_count: usize,
    ) -> f32 {
        let bar_height = line_height + 4.0;
        let bar_rect = egui::Rect::from_min_size(
            egui::pos2(rect.min.x, rect.max.y - bar_height),
            egui::vec2(rect.width(), bar_height),
        );
        painter.rect_filled(bar_rect, 0.0, theme.status_bar_bg);

        let text_y = bar_rect.min.y + 2.0;

        // Mode label
        let mode_text = mode.status_text();
        painter.text(
            egui::pos2(bar_rect.min.x + 10.0, text_y),
            egui::Align2::LEFT_TOP,
            mode_text,
            font_id.clone(),
            theme.status_bar_fg,
        );

        // Status message (temporary) or file path
        if let Some(msg) = status_message {
            painter.text(
                egui::pos2(bar_rect.min.x + 100.0, text_y),
                egui::Align2::LEFT_TOP,
                msg,
                font_id.clone(),
                theme.foreground,
            );
        } else if let Some(path) = file_path {
            painter.text(
                egui::pos2(bar_rect.min.x + 100.0, text_y),
                egui::Align2::LEFT_TOP,
                path,
                font_id.clone(),
                theme.line_number,
            );
        }

        // Right-side status: diagnostics + LSP health.
        let mut x = bar_rect.max.x - 10.0;
        if error_count > 0 || warning_count > 0 {
            if warning_count > 0 {
                let w_text = format!("W:{}", warning_count);
                let w_galley =
                    painter.layout_no_wrap(w_text.clone(), font_id.clone(), theme.warning_fg);
                x -= w_galley.rect.width();
                painter.text(
                    egui::pos2(x, text_y),
                    egui::Align2::LEFT_TOP,
                    &w_text,
                    font_id.clone(),
                    theme.warning_fg,
                );
                x -= 8.0;
            }
            if error_count > 0 {
                let e_text = format!("E:{}", error_count);
                let e_galley =
                    painter.layout_no_wrap(e_text.clone(), font_id.clone(), theme.error_fg);
                x -= e_galley.rect.width();
                painter.text(
                    egui::pos2(x, text_y),
                    egui::Align2::LEFT_TOP,
                    &e_text,
                    font_id.clone(),
                    theme.error_fg,
                );
            }
        }
        if let Some(health) = lsp_health {
            x -= 14.0;
            let color = if health.contains("running") {
                theme.line_number_active
            } else if health.contains("missing") || health.contains("error") {
                theme.error_fg
            } else {
                theme.line_number
            };
            let galley = painter.layout_no_wrap(health.to_string(), font_id.clone(), color);
            x -= galley.rect.width();
            painter.text(
                egui::pos2(x.max(bar_rect.min.x + 260.0), text_y),
                egui::Align2::LEFT_TOP,
                health,
                font_id.clone(),
                color,
            );
        }

        // Command/search line input (already includes prefix like ":", "/" or "?")
        if let Some(input) = command_input {
            let cmd_rect = egui::Rect::from_min_size(
                egui::pos2(rect.min.x, bar_rect.min.y - line_height - 2.0),
                egui::vec2(rect.width(), line_height + 2.0),
            );
            painter.rect_filled(cmd_rect, 0.0, theme.background);
            painter.text(
                egui::pos2(cmd_rect.min.x + 10.0, cmd_rect.min.y),
                egui::Align2::LEFT_TOP,
                input,
                font_id.clone(),
                theme.foreground,
            );
        }

        bar_height
    }
}
