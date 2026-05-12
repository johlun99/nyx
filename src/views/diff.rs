use crate::renderer::Theme;
use eframe::egui;
use std::path::PathBuf;
use std::process::Command;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum DiffMode {
    WorkingTree,
    Staged,
}

#[derive(Debug, PartialEq, Eq)]
enum LineKind {
    Context,
    Added,
    Removed,
    HunkHeader,
}

#[derive(Debug)]
struct SideBySideLine {
    left: Option<(usize, String)>,
    right: Option<(usize, String)>,
    kind: LineKind,
}

pub enum DiffInputResult {
    None,
    Close,
}

pub struct DiffView {
    path: String,
    root: PathBuf,
    mode: DiffMode,
    lines: Vec<SideBySideLine>,
    scroll_offset: usize,
    first_change: usize,
}

impl DiffView {
    pub fn new(root: PathBuf, path: String, mode: DiffMode) -> Self {
        let mut view = Self {
            path,
            root,
            mode,
            lines: Vec::new(),
            scroll_offset: 0,
            first_change: 0,
        };
        view.reload();
        view
    }

    fn reload(&mut self) {
        let output = self.run_diff();
        self.lines = parse_unified_diff(&output);
        self.first_change = self
            .lines
            .iter()
            .position(|l| l.kind != LineKind::Context && l.kind != LineKind::HunkHeader)
            .unwrap_or(0);
        // Scroll to first change, a few lines from the top
        self.scroll_offset = self.first_change.saturating_sub(3);
    }

    fn run_diff(&self) -> String {
        let args = match self.mode {
            DiffMode::WorkingTree => vec!["diff", "--", &self.path],
            DiffMode::Staged => vec!["diff", "--cached", "--", &self.path],
        };
        let output = Command::new("git")
            .args(&args)
            .current_dir(&self.root)
            .output();
        match output {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
            Ok(o) => String::from_utf8_lossy(&o.stderr).to_string(),
            Err(e) => format!("Failed to run git diff: {}", e),
        }
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            DiffMode::WorkingTree => DiffMode::Staged,
            DiffMode::Staged => DiffMode::WorkingTree,
        };
        self.reload();
    }

    pub fn handle_input(&mut self, ctx: &egui::Context) -> DiffInputResult {
        let mut result = DiffInputResult::None;
        ctx.input(|input| {
            if input.key_pressed(egui::Key::Escape) {
                result = DiffInputResult::Close;
                return;
            }
            if input.key_pressed(egui::Key::D) {
                self.toggle_mode();
                return;
            }
            if input.key_pressed(egui::Key::J) || input.key_pressed(egui::Key::ArrowDown) {
                if self.scroll_offset < self.lines.len().saturating_sub(1) {
                    self.scroll_offset += 1;
                }
                return;
            }
            if input.key_pressed(egui::Key::K) || input.key_pressed(egui::Key::ArrowUp) {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
        });
        result
    }

    pub fn render(&self, ui: &mut egui::Ui, theme: &Theme) {
        let mode_label = match self.mode {
            DiffMode::WorkingTree => "Working Tree Diff",
            DiffMode::Staged => "Staged Diff",
        };

        // Header bar
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("{} — {}", self.path, mode_label))
                    .color(theme.foreground)
                    .monospace()
                    .size(13.0)
                    .strong(),
            );
        });
        ui.add_space(2.0);
        ui.separator();
        ui.add_space(2.0);

        if self.lines.is_empty() {
            ui.label(
                egui::RichText::new("  No changes")
                    .color(theme.line_number)
                    .monospace()
                    .size(13.0),
            );
            return;
        }

        let available = ui.available_rect_before_wrap();
        let row_height = 18.0;
        let line_num_width = 50.0;
        let divider_width = 1.0;
        let half_width = (available.width() - divider_width) / 2.0;

        let visible_rows = (available.height() / row_height).floor() as usize;
        let end = (self.scroll_offset + visible_rows).min(self.lines.len());

        // Colors — blend highlight into background at ~12% to keep text readable
        let blend = |highlight: egui::Color32, t: f32| -> egui::Color32 {
            let bg = theme.background;
            let mix = |a: u8, b: u8| -> u8 { ((a as f32) * (1.0 - t) + (b as f32) * t) as u8 };
            egui::Color32::from_rgb(
                mix(bg.r(), highlight.r()),
                mix(bg.g(), highlight.g()),
                mix(bg.b(), highlight.b()),
            )
        };
        let added_bg = blend(theme.syntax.string, 0.12);
        let removed_bg = blend(theme.error_fg, 0.12);

        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(available.width(), available.height()),
            egui::Sense::hover(),
        );
        let painter = ui.painter_at(rect);

        // Draw vertical divider
        let divider_x = rect.min.x + half_width;
        painter.line_segment(
            [
                egui::pos2(divider_x, rect.min.y),
                egui::pos2(divider_x, rect.max.y),
            ],
            egui::Stroke::new(1.0, theme.selection),
        );

        for (vis_idx, line_idx) in (self.scroll_offset..end).enumerate() {
            let line = &self.lines[line_idx];
            let y = rect.min.y + vis_idx as f32 * row_height;

            if line.kind == LineKind::HunkHeader {
                // Hunk header spans full width
                let row_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.min.x, y),
                    egui::vec2(available.width(), row_height),
                );
                painter.rect_filled(row_rect, 0.0, theme.selection.linear_multiply(0.3));
                let text = line.left.as_ref().map(|(_, s)| s.as_str()).unwrap_or("");
                painter.text(
                    egui::pos2(rect.min.x + 4.0, y + 1.0),
                    egui::Align2::LEFT_TOP,
                    text,
                    egui::FontId::monospace(12.0),
                    theme.line_number,
                );
                continue;
            }

            // Background for added/removed
            let (left_bg, right_bg) = match line.kind {
                LineKind::Added => (None, Some(added_bg)),
                LineKind::Removed => (Some(removed_bg), None),
                _ => (None, None),
            };

            // Left side
            let left_rect = egui::Rect::from_min_size(
                egui::pos2(rect.min.x, y),
                egui::vec2(half_width, row_height),
            );
            if let Some(bg) = left_bg {
                painter.rect_filled(left_rect, 0.0, bg);
            }
            if let Some((num, text)) = &line.left {
                painter.text(
                    egui::pos2(rect.min.x + line_num_width - 8.0, y + 1.0),
                    egui::Align2::RIGHT_TOP,
                    format!("{}", num),
                    egui::FontId::monospace(12.0),
                    theme.line_number,
                );
                let text_color = match line.kind {
                    LineKind::Removed => theme.error_fg,
                    _ => theme.foreground,
                };
                painter.text(
                    egui::pos2(rect.min.x + line_num_width + 4.0, y + 1.0),
                    egui::Align2::LEFT_TOP,
                    text,
                    egui::FontId::monospace(12.0),
                    text_color,
                );
            }

            // Right side
            let right_x = divider_x + divider_width;
            let right_rect = egui::Rect::from_min_size(
                egui::pos2(right_x, y),
                egui::vec2(half_width, row_height),
            );
            if let Some(bg) = right_bg {
                painter.rect_filled(right_rect, 0.0, bg);
            }
            if let Some((num, text)) = &line.right {
                painter.text(
                    egui::pos2(right_x + line_num_width - 8.0, y + 1.0),
                    egui::Align2::RIGHT_TOP,
                    format!("{}", num),
                    egui::FontId::monospace(12.0),
                    theme.line_number,
                );
                let text_color = match line.kind {
                    LineKind::Added => theme.syntax.string,
                    _ => theme.foreground,
                };
                painter.text(
                    egui::pos2(right_x + line_num_width + 4.0, y + 1.0),
                    egui::Align2::LEFT_TOP,
                    text,
                    egui::FontId::monospace(12.0),
                    text_color,
                );
            }
        }

        // Hint bar at bottom
        let hint_y = rect.max.y - row_height;
        painter.text(
            egui::pos2(rect.min.x + 4.0, hint_y + 1.0),
            egui::Align2::LEFT_TOP,
            "j/k: scroll  d: toggle staged/working  Esc: close",
            egui::FontId::monospace(11.0),
            theme.line_number,
        );
    }
}

fn parse_unified_diff(output: &str) -> Vec<SideBySideLine> {
    let mut lines = Vec::new();
    let mut left_num: usize = 0;
    let mut right_num: usize = 0;
    let mut in_hunk = false;

    for raw_line in output.lines() {
        if raw_line.starts_with("@@") {
            // Parse hunk header: @@ -L,S +R,S @@
            if let Some((l, r)) = parse_hunk_header(raw_line) {
                left_num = l;
                right_num = r;
            }
            lines.push(SideBySideLine {
                left: Some((0, raw_line.to_string())),
                right: None,
                kind: LineKind::HunkHeader,
            });
            in_hunk = true;
            continue;
        }

        if !in_hunk {
            continue;
        }

        if let Some(text) = raw_line.strip_prefix('+') {
            lines.push(SideBySideLine {
                left: None,
                right: Some((right_num, text.to_string())),
                kind: LineKind::Added,
            });
            right_num += 1;
        } else if let Some(text) = raw_line.strip_prefix('-') {
            lines.push(SideBySideLine {
                left: Some((left_num, text.to_string())),
                right: None,
                kind: LineKind::Removed,
            });
            left_num += 1;
        } else if let Some(text) = raw_line.strip_prefix(' ') {
            lines.push(SideBySideLine {
                left: Some((left_num, text.to_string())),
                right: Some((right_num, text.to_string())),
                kind: LineKind::Context,
            });
            left_num += 1;
            right_num += 1;
        } else if raw_line == "\\ No newline at end of file" {
            // Skip this marker
        } else {
            // Treat as context (e.g. empty context lines)
            lines.push(SideBySideLine {
                left: Some((left_num, raw_line.to_string())),
                right: Some((right_num, raw_line.to_string())),
                kind: LineKind::Context,
            });
            left_num += 1;
            right_num += 1;
        }
    }

    lines
}

fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    // @@ -L,S +R,S @@ optional context
    let trimmed = line.strip_prefix("@@ ")?;
    let end = trimmed.find(" @@")?;
    let range_part = &trimmed[..end]; // e.g. "-10,5 +20,7"
    let mut parts = range_part.split_whitespace();
    let left_range = parts.next()?.strip_prefix('-')?;
    let right_range = parts.next()?.strip_prefix('+')?;

    let left_start: usize = left_range.split(',').next()?.parse().ok()?;
    let right_start: usize = right_range.split(',').next()?.parse().ok()?;

    Some((left_start, right_start))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_diff() {
        let lines = parse_unified_diff("");
        assert!(lines.is_empty());
    }

    #[test]
    fn parse_context_only() {
        let diff = "@@ -1,3 +1,3 @@\n line1\n line2\n line3\n";
        let lines = parse_unified_diff(diff);
        assert_eq!(lines.len(), 4); // hunk header + 3 context
        assert_eq!(lines[0].kind, LineKind::HunkHeader);
        assert_eq!(lines[1].kind, LineKind::Context);
        assert_eq!(lines[1].left, Some((1, "line1".to_string())));
        assert_eq!(lines[1].right, Some((1, "line1".to_string())));
    }

    #[test]
    fn parse_add_remove() {
        let diff = "@@ -1,2 +1,2 @@\n-old line\n+new line\n";
        let lines = parse_unified_diff(diff);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1].kind, LineKind::Removed);
        assert_eq!(lines[1].left, Some((1, "old line".to_string())));
        assert!(lines[1].right.is_none());
        assert_eq!(lines[2].kind, LineKind::Added);
        assert!(lines[2].left.is_none());
        assert_eq!(lines[2].right, Some((1, "new line".to_string())));
    }

    #[test]
    fn parse_mixed_hunks() {
        let diff = "\
@@ -5,4 +5,5 @@
 context
-removed
+added1
+added2
 context2
";
        let lines = parse_unified_diff(diff);
        assert_eq!(lines.len(), 6); // hunk + context + removed + added1 + added2 + context2
        assert_eq!(lines[0].kind, LineKind::HunkHeader);
        assert_eq!(lines[1].kind, LineKind::Context);
        assert_eq!(lines[1].left.as_ref().unwrap().0, 5);
        assert_eq!(lines[2].kind, LineKind::Removed);
        assert_eq!(lines[2].left.as_ref().unwrap().0, 6);
        assert_eq!(lines[3].kind, LineKind::Added);
        assert_eq!(lines[3].right.as_ref().unwrap().0, 6);
        assert_eq!(lines[4].kind, LineKind::Added);
        assert_eq!(lines[4].right.as_ref().unwrap().0, 7);
        assert_eq!(lines[5].kind, LineKind::Context);
    }

    #[test]
    fn parse_hunk_header_extraction() {
        assert_eq!(
            parse_hunk_header("@@ -10,5 +20,7 @@ fn foo()"),
            Some((10, 20))
        );
        assert_eq!(parse_hunk_header("@@ -1 +1 @@"), Some((1, 1)));
        assert_eq!(parse_hunk_header("@@ -0,0 +1,3 @@"), Some((0, 1)));
        assert!(parse_hunk_header("not a header").is_none());
    }

    #[test]
    fn side_by_side_alignment() {
        let diff = "@@ -1,3 +1,3 @@\n-removed\n+added\n context\n";
        let lines = parse_unified_diff(diff);
        // removed: left only
        assert!(lines[1].left.is_some());
        assert!(lines[1].right.is_none());
        // added: right only
        assert!(lines[2].left.is_none());
        assert!(lines[2].right.is_some());
        // context: both
        assert!(lines[3].left.is_some());
        assert!(lines[3].right.is_some());
    }

    #[test]
    fn first_change_detection() {
        let diff = "@@ -1,4 +1,4 @@\n context\n-removed\n+added\n context\n";
        let lines = parse_unified_diff(diff);
        let first_change = lines
            .iter()
            .position(|l| l.kind != LineKind::Context && l.kind != LineKind::HunkHeader)
            .unwrap_or(0);
        assert_eq!(first_change, 2); // index 0=hunk header, 1=context, 2=removed
    }
}
