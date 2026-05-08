// src/renderer/editor_view.rs
use crate::config::{format_line_number, LineNumberMode};
use crate::editor::Editor;
use crate::lsp::{CodeActionState, CompletionState, HoverState, LspManager, NyxDiagnostic};
use crate::renderer::status_bar::StatusBar;
use crate::renderer::theme::Theme;
use crate::vim::mode::Mode;
use eframe::egui::{self, Rect, Sense, Vec2};

use crate::lsp::protocol::DiagnosticSeverity;

/// Result of a mouse click in the editor area.
pub struct EditorClick {
    pub line: usize,
    pub col: usize,
}

pub struct EditorView {
    pub scroll_offset: usize,
}

impl EditorView {
    pub fn new() -> Self {
        Self { scroll_offset: 0 }
    }

    /// Render the editor and return click position if the user clicked in the text area.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        editor: &Editor,
        theme: &Theme,
        font_size: f32,
        line_number_mode: LineNumberMode,
        lsp_manager: &LspManager,
    ) -> Option<EditorClick> {
        let buffer = &editor.buffer;
        let mode = editor.mode();
        let file_path = editor.file_path.as_deref();
        let command_display = editor.command_input().map(|s| format!(":{}", s));
        let search_display = editor.search_input_display();
        let bottom_input = search_display.or(command_display);
        let status_message = editor.status_message.as_deref();
        let lsp_health = file_path.and_then(|p| lsp_manager.health_summary_for_file(p));

        let diagnostics: &[NyxDiagnostic] = file_path
            .map(|p| lsp_manager.diagnostics_for_file(p))
            .unwrap_or(&[]);
        let (error_count, warning_count) = file_path
            .map(|p| lsp_manager.diagnostic_counts(p))
            .unwrap_or((0, 0));

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
        let click_pos = if response.clicked() {
            response.interact_pointer_pos()
        } else {
            None
        };
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
            lsp_health.as_deref(),
            error_count,
            warning_count,
        );

        let editor_height = available.y - status_height;
        let visible_lines = ((editor_height / line_height).ceil() as usize).max(1);

        // Diagnostic gutter width (always reserve space to avoid layout shift)
        let diag_gutter_width = char_width * 3.0;

        // Dynamic gutter width based on line number mode
        let (gutter_width, text_x) = if line_number_mode == LineNumberMode::Off {
            (0.0_f32, rect.min.x + diag_gutter_width + 4.0)
        } else {
            let max_line = buffer.line_count();
            let digits = max_line.to_string().len().max(3);
            let width = (digits + 2) as f32 * char_width;
            (width, rect.min.x + diag_gutter_width + width + 4.0)
        };

        let end_line = (self.scroll_offset + visible_lines).min(buffer.line_count());

        for i in self.scroll_offset..end_line {
            let y = rect.min.y + ((i - self.scroll_offset) as f32) * line_height;
            let line_slice = buffer.line_slice(i);
            let line_str = line_slice.to_string();
            let display = line_str.trim_end_matches('\n');

            // Diagnostic gutter icon
            if let Some(severity) = line_max_severity(diagnostics, i) {
                let (icon, color) = if severity == DiagnosticSeverity::ERROR {
                    ("\u{25cf}", theme.error_fg) // filled circle
                } else {
                    ("\u{25b2}", theme.warning_fg) // triangle
                };
                painter.text(
                    egui::pos2(rect.min.x + 2.0, y),
                    egui::Align2::LEFT_TOP,
                    icon,
                    font_id.clone(),
                    color,
                );
            }

            // Line number (skip entirely when Off)
            if line_number_mode != LineNumberMode::Off {
                let raw = format_line_number(line_number_mode, i, buffer.cursor_line());
                let gutter_chars = (gutter_width / char_width).floor() as usize;
                let padded = format!("{:>width$}", raw, width = gutter_chars.saturating_sub(1));
                let num_color = if i == buffer.cursor_line() {
                    theme.line_number_active
                } else {
                    theme.line_number
                };
                painter.text(
                    egui::pos2(rect.min.x + diag_gutter_width, y),
                    egui::Align2::LEFT_TOP,
                    &padded,
                    font_id.clone(),
                    num_color,
                );
            }

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

            // Diagnostic underlines (drawn before text, after selection)
            for diag in diagnostics
                .iter()
                .filter(|d| d.start_line <= i && d.end_line >= i)
            {
                let col_start = if i == diag.start_line {
                    diag.start_col
                } else {
                    0
                };
                let col_end = if i == diag.end_line {
                    diag.end_col
                } else {
                    display.chars().count()
                };
                if col_start >= col_end {
                    continue;
                }

                let underline_color = if diag.severity == DiagnosticSeverity::ERROR {
                    theme.error_underline
                } else {
                    theme.warning_underline
                };

                let x_start = if col_start == 0 {
                    0.0
                } else {
                    let prefix: String = display.chars().take(col_start).collect();
                    painter
                        .layout_no_wrap(prefix, font_id.clone(), theme.foreground)
                        .rect
                        .width()
                };
                let x_end = {
                    let prefix: String = display.chars().take(col_end).collect();
                    painter
                        .layout_no_wrap(prefix, font_id.clone(), theme.foreground)
                        .rect
                        .width()
                };

                // Draw wavy underline as small segments
                let underline_y = y + line_height - 2.0;
                let wave_height = 2.0;
                let wave_period = 4.0;
                let mut wx = text_x + x_start;
                let end_wx = text_x + x_end;
                while wx < end_wx {
                    let next_wx = (wx + wave_period).min(end_wx);
                    let mid = (wx + next_wx) / 2.0;
                    let peak = underline_y
                        + if ((wx - text_x) / wave_period) as i32 % 2 == 0 {
                            -wave_height
                        } else {
                            wave_height
                        };
                    painter.line_segment(
                        [egui::pos2(wx, underline_y), egui::pos2(mid, peak)],
                        egui::Stroke::new(1.0, underline_color),
                    );
                    painter.line_segment(
                        [egui::pos2(mid, peak), egui::pos2(next_wx, underline_y)],
                        egui::Stroke::new(1.0, underline_color),
                    );
                    wx = next_wx;
                }
            }

            // Text content — with syntax highlighting if available
            let syntax_spans = editor.syntax_highlights_for_line(i, theme);
            if syntax_spans.is_empty() {
                // Monochrome fallback
                painter.text(
                    egui::pos2(text_x, y),
                    egui::Align2::LEFT_TOP,
                    display,
                    font_id.clone(),
                    theme.foreground,
                );
            } else {
                // Per-span colored rendering
                let display_chars: Vec<char> = display.chars().collect();
                let mut last_end: usize = 0;

                for (col_start, col_end, color) in &syntax_spans {
                    let col_start = *col_start;
                    let col_end = (*col_end).min(display_chars.len());
                    if col_start > display_chars.len() {
                        continue;
                    }

                    // Draw unhighlighted gap before this span
                    if col_start > last_end {
                        let gap_text: String = display_chars[last_end..col_start].iter().collect();
                        let gap_x = text_x
                            + if last_end == 0 {
                                0.0
                            } else {
                                let prefix: String = display_chars[..last_end].iter().collect();
                                painter
                                    .layout_no_wrap(prefix, font_id.clone(), theme.foreground)
                                    .rect
                                    .width()
                            };
                        painter.text(
                            egui::pos2(gap_x, y),
                            egui::Align2::LEFT_TOP,
                            &gap_text,
                            font_id.clone(),
                            theme.foreground,
                        );
                    }

                    // Draw highlighted span
                    let span_text: String = display_chars[col_start..col_end].iter().collect();
                    let span_x = text_x
                        + if col_start == 0 {
                            0.0
                        } else {
                            let prefix: String = display_chars[..col_start].iter().collect();
                            painter
                                .layout_no_wrap(prefix, font_id.clone(), theme.foreground)
                                .rect
                                .width()
                        };
                    painter.text(
                        egui::pos2(span_x, y),
                        egui::Align2::LEFT_TOP,
                        &span_text,
                        font_id.clone(),
                        *color,
                    );

                    last_end = col_end;
                }

                // Draw remaining text after last span
                if last_end < display_chars.len() {
                    let remaining: String = display_chars[last_end..].iter().collect();
                    let rem_x = text_x
                        + if last_end == 0 {
                            0.0
                        } else {
                            let prefix: String = display_chars[..last_end].iter().collect();
                            painter
                                .layout_no_wrap(prefix, font_id.clone(), theme.foreground)
                                .rect
                                .width()
                        };
                    painter.text(
                        egui::pos2(rem_x, y),
                        egui::Align2::LEFT_TOP,
                        &remaining,
                        font_id.clone(),
                        theme.foreground,
                    );
                }
            }

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

        // Completion popup
        if let Some(ref completion) = lsp_manager.completion {
            self.render_completion_popup(
                &painter,
                completion,
                theme,
                &font_id,
                text_x,
                rect,
                line_height,
                char_width,
                buffer.cursor_line(),
                buffer.cursor_col(),
            );
        }

        // Hover popup
        if let Some(ref hover) = lsp_manager.hover_result {
            self.render_hover_popup(
                &painter,
                hover,
                theme,
                &font_id,
                text_x,
                rect,
                line_height,
                char_width,
                buffer.cursor_line(),
                buffer.cursor_col(),
            );
        }

        // Code action popup
        if let Some(ref actions) = lsp_manager.code_actions {
            self.render_code_action_popup(
                &painter,
                actions,
                theme,
                &font_id,
                text_x,
                rect,
                line_height,
                char_width,
                buffer.cursor_line(),
                buffer.cursor_col(),
            );
        }

        // Resolve click to line/col
        let editor_click = click_pos.and_then(|pos| {
            let rel_y = pos.y - rect.min.y;
            if rel_y < 0.0 || rel_y >= editor_height {
                return None;
            }
            let line = self.scroll_offset + (rel_y / line_height) as usize;
            let line = line.min(buffer.line_count().saturating_sub(1));

            let rel_x = pos.x - text_x;
            let col = if rel_x <= 0.0 {
                0
            } else {
                // Walk characters to find the clicked column
                let line_slice = buffer.line_slice(line);
                let line_str = line_slice.to_string();
                let display_str = line_str.trim_end_matches('\n');
                let mut best_col = display_str.chars().count();
                let mut prev_width = 0.0_f32;
                for (ci, _) in display_str.char_indices().enumerate() {
                    let prefix: String = display_str.chars().take(ci + 1).collect();
                    let w = painter
                        .layout_no_wrap(prefix, font_id.clone(), theme.foreground)
                        .rect
                        .width();
                    let midpoint = (prev_width + w) / 2.0;
                    if rel_x < midpoint {
                        best_col = ci;
                        break;
                    }
                    prev_width = w;
                }
                best_col
            };
            Some(EditorClick { line, col })
        });

        // Scroll follow
        if buffer.cursor_line() < self.scroll_offset {
            self.scroll_offset = buffer.cursor_line();
        } else if buffer.cursor_line() >= self.scroll_offset + visible_lines {
            self.scroll_offset = buffer.cursor_line() - visible_lines + 1;
        }

        editor_click
    }

    #[allow(clippy::too_many_arguments)]
    fn render_completion_popup(
        &self,
        painter: &egui::Painter,
        completion: &CompletionState,
        theme: &Theme,
        font_id: &egui::FontId,
        text_x: f32,
        editor_rect: Rect,
        line_height: f32,
        char_width: f32,
        cursor_line: usize,
        cursor_col: usize,
    ) {
        let filtered = completion.filtered_items();
        if filtered.is_empty() {
            return;
        }

        let max_visible = 10.min(filtered.len());
        let popup_height = max_visible as f32 * line_height + 4.0;
        let popup_width = 300.0_f32.min(editor_rect.width() * 0.6);

        // Position: below cursor if room, above if near bottom
        let cursor_screen_line = cursor_line.saturating_sub(self.scroll_offset);
        let cursor_y = editor_rect.min.y + cursor_screen_line as f32 * line_height;
        let cursor_x = text_x + cursor_col as f32 * char_width;

        let below_y = cursor_y + line_height;
        let popup_y = if below_y + popup_height < editor_rect.max.y - line_height * 2.0 {
            below_y
        } else {
            cursor_y - popup_height
        };

        let popup_x = cursor_x.min(editor_rect.max.x - popup_width);
        let popup_rect = Rect::from_min_size(
            egui::pos2(popup_x, popup_y),
            Vec2::new(popup_width, popup_height),
        );

        // Background
        painter.rect_filled(popup_rect, 4.0, theme.status_bar_bg);
        painter.rect_stroke(
            popup_rect,
            4.0,
            egui::Stroke::new(1.0, theme.line_number),
            egui::StrokeKind::Outside,
        );

        // Items
        let scroll_start = if completion.selected >= max_visible {
            completion.selected - max_visible + 1
        } else {
            0
        };

        for (vi, idx) in (scroll_start..scroll_start + max_visible).enumerate() {
            if idx >= filtered.len() {
                break;
            }
            let item = filtered[idx];
            let item_y = popup_rect.min.y + 2.0 + vi as f32 * line_height;
            let is_selected = idx == completion.selected;

            if is_selected {
                let sel_rect = Rect::from_min_size(
                    egui::pos2(popup_rect.min.x + 1.0, item_y),
                    Vec2::new(popup_width - 2.0, line_height),
                );
                painter.rect_filled(sel_rect, 2.0, theme.selection);
            }

            // Kind icon
            let kind_text = item.kind.map(|k| k.icon()).unwrap_or("  ");
            painter.text(
                egui::pos2(popup_rect.min.x + 4.0, item_y),
                egui::Align2::LEFT_TOP,
                kind_text,
                font_id.clone(),
                theme.syntax.keyword,
            );

            // Label
            painter.text(
                egui::pos2(popup_rect.min.x + 4.0 + char_width * 3.0, item_y),
                egui::Align2::LEFT_TOP,
                &item.label,
                font_id.clone(),
                theme.foreground,
            );
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn render_hover_popup(
        &self,
        painter: &egui::Painter,
        hover: &HoverState,
        theme: &Theme,
        font_id: &egui::FontId,
        text_x: f32,
        editor_rect: Rect,
        line_height: f32,
        char_width: f32,
        cursor_line: usize,
        cursor_col: usize,
    ) {
        if hover.text.is_empty() {
            return;
        }

        // Limit to 5 lines
        let lines: Vec<&str> = hover.text.lines().take(5).collect();
        let truncated = hover.text.lines().count() > 5;
        let line_count = lines.len() + if truncated { 1 } else { 0 };

        let popup_height = line_count as f32 * line_height + 8.0;
        let popup_width = 400.0_f32.min(editor_rect.width() * 0.7);

        // Position above cursor
        let cursor_screen_line = cursor_line.saturating_sub(self.scroll_offset);
        let cursor_y = editor_rect.min.y + cursor_screen_line as f32 * line_height;
        let cursor_x = text_x + cursor_col as f32 * char_width;

        let popup_y = if cursor_y - popup_height > editor_rect.min.y {
            cursor_y - popup_height
        } else {
            cursor_y + line_height
        };
        let popup_x = cursor_x.min(editor_rect.max.x - popup_width);

        let popup_rect = Rect::from_min_size(
            egui::pos2(popup_x, popup_y),
            Vec2::new(popup_width, popup_height),
        );

        painter.rect_filled(popup_rect, 4.0, theme.status_bar_bg);
        painter.rect_stroke(
            popup_rect,
            4.0,
            egui::Stroke::new(1.0, theme.line_number),
            egui::StrokeKind::Outside,
        );

        for (i, line) in lines.iter().enumerate() {
            painter.text(
                egui::pos2(
                    popup_rect.min.x + 6.0,
                    popup_rect.min.y + 4.0 + i as f32 * line_height,
                ),
                egui::Align2::LEFT_TOP,
                line,
                font_id.clone(),
                theme.foreground,
            );
        }
        if truncated {
            painter.text(
                egui::pos2(
                    popup_rect.min.x + 6.0,
                    popup_rect.min.y + 4.0 + lines.len() as f32 * line_height,
                ),
                egui::Align2::LEFT_TOP,
                "...",
                font_id.clone(),
                theme.line_number,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_code_action_popup(
        &self,
        painter: &egui::Painter,
        actions: &CodeActionState,
        theme: &Theme,
        font_id: &egui::FontId,
        text_x: f32,
        editor_rect: Rect,
        line_height: f32,
        char_width: f32,
        cursor_line: usize,
        cursor_col: usize,
    ) {
        if actions.actions.is_empty() {
            return;
        }

        let max_visible = 8.min(actions.actions.len());
        let popup_height = max_visible as f32 * line_height + 4.0;
        let popup_width = 350.0_f32.min(editor_rect.width() * 0.6);

        let cursor_screen_line = cursor_line.saturating_sub(self.scroll_offset);
        let cursor_y = editor_rect.min.y + cursor_screen_line as f32 * line_height;
        let cursor_x = text_x + cursor_col as f32 * char_width;

        let below_y = cursor_y + line_height;
        let popup_y = if below_y + popup_height < editor_rect.max.y - line_height * 2.0 {
            below_y
        } else {
            cursor_y - popup_height
        };
        let popup_x = cursor_x.min(editor_rect.max.x - popup_width);

        let popup_rect = Rect::from_min_size(
            egui::pos2(popup_x, popup_y),
            Vec2::new(popup_width, popup_height),
        );

        painter.rect_filled(popup_rect, 4.0, theme.status_bar_bg);
        painter.rect_stroke(
            popup_rect,
            4.0,
            egui::Stroke::new(1.0, theme.line_number),
            egui::StrokeKind::Outside,
        );

        let scroll_start = if actions.selected >= max_visible {
            actions.selected - max_visible + 1
        } else {
            0
        };

        for (vi, idx) in (scroll_start..scroll_start + max_visible).enumerate() {
            if idx >= actions.actions.len() {
                break;
            }
            let action = &actions.actions[idx];
            let item_y = popup_rect.min.y + 2.0 + vi as f32 * line_height;
            let is_selected = idx == actions.selected;

            if is_selected {
                let sel_rect = Rect::from_min_size(
                    egui::pos2(popup_rect.min.x + 1.0, item_y),
                    Vec2::new(popup_width - 2.0, line_height),
                );
                painter.rect_filled(sel_rect, 2.0, theme.selection);
            }

            painter.text(
                egui::pos2(popup_rect.min.x + 6.0, item_y),
                egui::Align2::LEFT_TOP,
                &action.title,
                font_id.clone(),
                theme.foreground,
            );
        }
    }
}

/// Find the maximum severity diagnostic on a given line.
fn line_max_severity(diagnostics: &[NyxDiagnostic], line: usize) -> Option<DiagnosticSeverity> {
    let mut max: Option<DiagnosticSeverity> = None;
    for d in diagnostics {
        if d.start_line <= line && d.end_line >= line {
            match max {
                None => max = Some(d.severity),
                Some(current) => {
                    // Lower number = higher severity (ERROR=1 > WARNING=2)
                    if d.severity.0 < current.0 {
                        max = Some(d.severity);
                    }
                }
            }
        }
    }
    max
}
