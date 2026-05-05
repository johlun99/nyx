// src/renderer/editor_view.rs
use crate::editor::Editor;
use crate::renderer::status_bar::StatusBar;
use crate::renderer::theme::Theme;
use crate::vim::mode::Mode;
use eframe::egui::{self, Rect, Sense, Vec2};

pub struct EditorView {
    pub scroll_offset: usize,
}

impl EditorView {
    pub fn new() -> Self {
        Self { scroll_offset: 0 }
    }

    pub fn render(&mut self, ui: &mut egui::Ui, editor: &Editor, theme: &Theme, font_size: f32) {
        let buffer = &editor.buffer;
        let mode = editor.mode();
        let file_path = editor.file_path.as_deref();
        let command_display = editor.command_input().map(|s| format!(":{}", s));
        let search_display = editor.search_input_display();
        let bottom_input = search_display.or(command_display);
        let status_message = editor.status_message.as_deref();

        let font_id = egui::FontId::monospace(font_size);
        let line_height = ui.fonts(|f| f.row_height(&font_id));
        let char_width = ui.fonts(|f| {
            f.layout_no_wrap("m".to_string(), font_id.clone(), theme.foreground)
                .rect
                .width()
        });
        let available = ui.available_size();

        // Allocate painter
        let (response, painter) = ui.allocate_painter(available, Sense::click_and_drag());
        let rect = response.rect;
        painter.rect_filled(rect, 0.0, theme.background);

        // Status bar at bottom
        let status_height = StatusBar::render(
            &painter,
            rect,
            theme,
            &font_id,
            line_height,
            mode,
            file_path,
            bottom_input.as_deref(),
            status_message,
        );

        let editor_height = available.y - status_height;
        let visible_lines = ((editor_height / line_height).ceil() as usize).max(1);
        let gutter_width = 50.0;
        let text_x = rect.min.x + gutter_width + 10.0;

        let end_line = (self.scroll_offset + visible_lines).min(buffer.line_count());

        for i in self.scroll_offset..end_line {
            let y = rect.min.y + ((i - self.scroll_offset) as f32) * line_height;
            let line_slice = buffer.line_slice(i);
            let line_str = line_slice.to_string();
            let display = line_str.trim_end_matches('\n');

            // Line number
            let line_num = format!("{:>4}", i + 1);
            let num_color = if i == buffer.cursor_line() {
                theme.line_number_active
            } else {
                theme.line_number
            };
            painter.text(
                egui::pos2(rect.min.x + 5.0, y),
                egui::Align2::LEFT_TOP,
                &line_num,
                font_id.clone(),
                num_color,
            );

            // Search match highlights (drawn before text so text is visible on top)
            for (match_start_col, match_end_col, is_current) in editor.search_highlights_for_line(i)
            {
                let match_x_start: f32 = if match_start_col == 0 {
                    0.0
                } else {
                    let prefix: String = display.chars().take(match_start_col).collect();
                    painter
                        .layout_no_wrap(prefix, font_id.clone(), theme.foreground)
                        .rect
                        .width()
                };
                let match_x_end: f32 = {
                    let prefix: String = display.chars().take(match_end_col).collect();
                    painter
                        .layout_no_wrap(prefix, font_id.clone(), theme.foreground)
                        .rect
                        .width()
                };
                let match_rect = Rect::from_min_size(
                    egui::pos2(text_x + match_x_start, y),
                    Vec2::new(match_x_end - match_x_start, line_height),
                );
                let color = if is_current {
                    theme.search_current
                } else {
                    theme.search_match
                };
                painter.rect_filled(match_rect, 0.0, color);
            }

            // Selection highlight (visual modes, drawn before text so text is visible on top)
            if let Some((sel_start_col, sel_end_col)) = editor.visual_highlights_for_line(i) {
                let sel_x_start: f32 = if sel_start_col == 0 {
                    0.0
                } else {
                    let prefix: String = display.chars().take(sel_start_col).collect();
                    painter
                        .layout_no_wrap(prefix, font_id.clone(), theme.foreground)
                        .rect
                        .width()
                };
                let sel_x_end: f32 = {
                    let prefix: String = display.chars().take(sel_end_col).collect();
                    painter
                        .layout_no_wrap(prefix, font_id.clone(), theme.foreground)
                        .rect
                        .width()
                };
                let sel_rect = Rect::from_min_size(
                    egui::pos2(text_x + sel_x_start, y),
                    Vec2::new(sel_x_end - sel_x_start, line_height),
                );
                painter.rect_filled(sel_rect, 0.0, theme.selection);
            }

            // Text content (drawn after highlights so text is visible)
            painter.text(
                egui::pos2(text_x, y),
                egui::Align2::LEFT_TOP,
                display,
                font_id.clone(),
                theme.foreground,
            );

            // Cursor (only on cursor line)
            if i == buffer.cursor_line() {
                let cursor_col = buffer.cursor_col();

                // Calculate cursor x position using char_indices (char-safe)
                let cursor_x: f32 = if cursor_col == 0 {
                    0.0
                } else {
                    let prefix: String = display.chars().take(cursor_col).collect();
                    painter
                        .layout_no_wrap(prefix, font_id.clone(), theme.foreground)
                        .rect
                        .width()
                };

                match mode {
                    Mode::Normal | Mode::Command => {
                        // Block cursor
                        let cursor_rect = Rect::from_min_size(
                            egui::pos2(text_x + cursor_x, y),
                            Vec2::new(char_width, line_height),
                        );
                        painter.rect_filled(cursor_rect, 0.0, theme.cursor);

                        // Draw character under cursor with inverted color
                        if let Some(ch) = display.chars().nth(cursor_col) {
                            painter.text(
                                egui::pos2(text_x + cursor_x, y),
                                egui::Align2::LEFT_TOP,
                                ch.to_string(),
                                font_id.clone(),
                                theme.background,
                            );
                        }
                    }
                    Mode::Insert => {
                        // Thin line cursor (2px wide)
                        let cursor_rect = Rect::from_min_size(
                            egui::pos2(text_x + cursor_x, y),
                            Vec2::new(2.0, line_height),
                        );
                        painter.rect_filled(cursor_rect, 0.0, theme.cursor_insert);
                    }
                    Mode::Visual | Mode::VisualLine | Mode::VisualBlock => {
                        // Block cursor for visual modes (same as normal)
                        let cursor_rect = Rect::from_min_size(
                            egui::pos2(text_x + cursor_x, y),
                            Vec2::new(char_width, line_height),
                        );
                        painter.rect_filled(cursor_rect, 0.0, theme.cursor);

                        // Draw character under cursor with inverted color
                        if let Some(ch) = display.chars().nth(cursor_col) {
                            painter.text(
                                egui::pos2(text_x + cursor_x, y),
                                egui::Align2::LEFT_TOP,
                                ch.to_string(),
                                font_id.clone(),
                                theme.background,
                            );
                        }
                    }
                }
            }
        }

        // Scroll follow
        if buffer.cursor_line() < self.scroll_offset {
            self.scroll_offset = buffer.cursor_line();
        } else if buffer.cursor_line() >= self.scroll_offset + visible_lines {
            self.scroll_offset = buffer.cursor_line() - visible_lines + 1;
        }
    }
}
