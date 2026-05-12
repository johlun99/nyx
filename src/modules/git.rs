use crate::modules::ModuleAction;
use crate::renderer::Theme;
use eframe::egui;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

const AUTO_REFRESH_SECS: u64 = 5;
const SUCCESS_DISMISS_SECS: u64 = 3;
const ERROR_DISMISS_SECS: u64 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    Untracked,
}

impl FileStatus {
    fn prefix(&self) -> &'static str {
        match self {
            FileStatus::Modified => "M",
            FileStatus::Added => "A",
            FileStatus::Deleted => "D",
            FileStatus::Renamed => "R",
            FileStatus::Copied => "C",
            FileStatus::Untracked => "?",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GitFileEntry {
    pub status: FileStatus,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Row {
    StagedHeader,
    StagedFile(usize),
    ChangesHeader,
    ChangesFile(usize),
}

struct CommitInput {
    message: String,
    active: bool,
    result_message: Option<(String, Instant)>,
}

pub struct GitModule {
    root: Option<PathBuf>,
    staged: Vec<GitFileEntry>,
    unstaged: Vec<GitFileEntry>,
    rows: Vec<Row>,
    selected: usize,
    commit_input: CommitInput,
    last_refresh: Option<Instant>,
    last_error: Option<(String, Instant)>,
}

impl GitModule {
    pub fn new(root: Option<PathBuf>) -> Self {
        let mut module = Self {
            root,
            staged: Vec::new(),
            unstaged: Vec::new(),
            rows: Vec::new(),
            selected: 0,
            commit_input: CommitInput {
                message: String::new(),
                active: false,
                result_message: None,
            },
            last_refresh: None,
            last_error: None,
        };
        module.refresh();
        module
    }

    fn run_git(&self, args: &[&str]) -> Result<String, String> {
        let root = self
            .root
            .as_ref()
            .ok_or_else(|| "no git root".to_string())?;
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    }

    pub fn refresh(&mut self) {
        self.staged.clear();
        self.unstaged.clear();

        if let Ok(output) = self.run_git(&["status", "--porcelain=v1"]) {
            parse_status(&output, &mut self.staged, &mut self.unstaged);
        }

        self.rebuild_rows();
        self.last_refresh = Some(Instant::now());
    }

    fn rebuild_rows(&mut self) {
        self.rows.clear();

        if !self.staged.is_empty() {
            self.rows.push(Row::StagedHeader);
            for i in 0..self.staged.len() {
                self.rows.push(Row::StagedFile(i));
            }
        }

        if !self.unstaged.is_empty() {
            self.rows.push(Row::ChangesHeader);
            for i in 0..self.unstaged.len() {
                self.rows.push(Row::ChangesFile(i));
            }
        }

        // Clamp selection
        if !self.rows.is_empty() && self.selected >= self.rows.len() {
            self.selected = self.rows.len() - 1;
        }
    }

    fn stage_file(&mut self, path: &str) {
        if let Err(e) = self.run_git(&["add", "--", path]) {
            self.last_error = Some((e, Instant::now()));
        }
        self.refresh();
    }

    fn unstage_file(&mut self, path: &str) {
        if let Err(e) = self.run_git(&["reset", "HEAD", "--", path]) {
            self.last_error = Some((e, Instant::now()));
        }
        self.refresh();
    }

    fn stage_all(&mut self) {
        if let Err(e) = self.run_git(&["add", "-A"]) {
            self.last_error = Some((e, Instant::now()));
        }
        self.refresh();
    }

    fn unstage_all(&mut self) {
        if let Err(e) = self.run_git(&["reset", "HEAD"]) {
            self.last_error = Some((e, Instant::now()));
        }
        self.refresh();
    }

    fn commit(&mut self) {
        let msg = self.commit_input.message.trim().to_string();
        if msg.is_empty() {
            return;
        }
        match self.run_git(&["commit", "-m", &msg]) {
            Ok(output) => {
                let summary = output.lines().next().unwrap_or("committed").to_string();
                self.commit_input.result_message = Some((summary, Instant::now()));
                self.commit_input.message.clear();
                self.commit_input.active = false;
            }
            Err(e) => {
                self.last_error = Some((e, Instant::now()));
            }
        }
        self.refresh();
    }

    pub fn handle_input(&mut self, ctx: &egui::Context) -> ModuleAction {
        let mut action = ModuleAction::None;
        let mut want_stage: Option<String> = None;
        let mut want_unstage: Option<String> = None;
        let mut want_stage_all = false;
        let mut want_unstage_all = false;
        let mut want_commit = false;
        let mut want_refresh = false;
        let mut commit_text: Option<String> = None;

        ctx.input(|input| {
            if self.commit_input.active {
                // Commit input mode
                if input.key_pressed(egui::Key::Escape) {
                    self.commit_input.active = false;
                    self.commit_input.message.clear();
                    return;
                }
                if input.key_pressed(egui::Key::Enter) {
                    want_commit = true;
                    return;
                }
                if input.key_pressed(egui::Key::Backspace) {
                    self.commit_input.message.pop();
                    return;
                }
                for event in &input.events {
                    if let egui::Event::Text(text) = event {
                        commit_text = Some(text.clone());
                    }
                }
                return;
            }

            // Normal navigation mode
            if input.key_pressed(egui::Key::Escape) {
                return;
            }

            if input.key_pressed(egui::Key::ArrowDown) || input.key_pressed(egui::Key::J) {
                if !self.rows.is_empty() && self.selected < self.rows.len() - 1 {
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

            if input.key_pressed(egui::Key::S) {
                if let Some(row) = self.rows.get(self.selected) {
                    match row {
                        Row::ChangesFile(i) => {
                            want_stage = Some(self.unstaged[*i].path.clone());
                        }
                        Row::ChangesHeader => {
                            want_stage_all = true;
                        }
                        Row::StagedFile(i) => {
                            want_unstage = Some(self.staged[*i].path.clone());
                        }
                        Row::StagedHeader => {
                            want_unstage_all = true;
                        }
                    }
                }
                return;
            }

            if input.key_pressed(egui::Key::C) {
                self.commit_input.active = true;
                self.commit_input.message.clear();
                return;
            }

            if input.key_pressed(egui::Key::Enter) {
                if let Some(row) = self.rows.get(self.selected) {
                    match row {
                        Row::ChangesFile(i) => {
                            action = ModuleAction::ViewDiff {
                                path: self.unstaged[*i].path.clone(),
                                staged: false,
                            };
                        }
                        Row::StagedFile(i) => {
                            action = ModuleAction::ViewDiff {
                                path: self.staged[*i].path.clone(),
                                staged: true,
                            };
                        }
                        _ => {}
                    }
                }
                return;
            }

            if input.key_pressed(egui::Key::R) {
                want_refresh = true;
            }
        });

        // Apply deferred actions outside input closure
        if let Some(text) = commit_text {
            self.commit_input.message.push_str(&text);
        }
        if want_commit {
            self.commit();
        }
        if let Some(path) = want_stage {
            self.stage_file(&path);
        }
        if let Some(path) = want_unstage {
            self.unstage_file(&path);
        }
        if want_stage_all {
            self.stage_all();
        }
        if want_unstage_all {
            self.unstage_all();
        }
        if want_refresh {
            self.refresh();
        }

        action
    }

    pub fn render(&mut self, ui: &mut egui::Ui, theme: &Theme, focused: bool) -> ModuleAction {
        let action = ModuleAction::None;

        // Auto-refresh
        if let Some(last) = self.last_refresh {
            if last.elapsed().as_secs() >= AUTO_REFRESH_SECS {
                self.refresh();
            }
        }

        // Dismiss stale messages
        if let Some((_, when)) = &self.last_error {
            if when.elapsed().as_secs() >= ERROR_DISMISS_SECS {
                self.last_error = None;
            }
        }
        if let Some((_, when)) = &self.commit_input.result_message {
            if when.elapsed().as_secs() >= SUCCESS_DISMISS_SECS {
                self.commit_input.result_message = None;
            }
        }

        // Header
        let header_color = if focused {
            theme.foreground
        } else {
            theme.line_number
        };
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("GIT")
                    .color(header_color)
                    .size(11.0)
                    .strong(),
            );
        });
        ui.add_space(4.0);

        // Empty state
        if self.rows.is_empty() {
            ui.label(
                egui::RichText::new("  nothing to commit, working tree clean")
                    .color(theme.line_number)
                    .monospace()
                    .size(12.0),
            );
        }

        // Scroll area with rows
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (row_idx, row) in self.rows.iter().enumerate() {
                    let is_selected = row_idx == self.selected && focused;

                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 20.0),
                        egui::Sense::click(),
                    );

                    if response.hovered() && !is_selected {
                        ui.painter()
                            .rect_filled(rect, 0.0, theme.selection.linear_multiply(0.5));
                    }
                    if is_selected {
                        ui.painter().rect_filled(rect, 0.0, theme.selection);
                    }

                    if response.clicked() {
                        self.selected = row_idx;
                    }

                    let (text, color) = match row {
                        Row::StagedHeader => (
                            format!("Staged ({})", self.staged.len()),
                            theme.syntax.keyword,
                        ),
                        Row::StagedFile(i) => {
                            let entry = &self.staged[*i];
                            (
                                format!("  {} {}", entry.status.prefix(), entry.path),
                                theme.syntax.string,
                            )
                        }
                        Row::ChangesHeader => (
                            format!("Changes ({})", self.unstaged.len()),
                            theme.syntax.keyword,
                        ),
                        Row::ChangesFile(i) => {
                            let entry = &self.unstaged[*i];
                            let color = match entry.status {
                                FileStatus::Modified => theme.warning_fg,
                                FileStatus::Deleted => theme.error_fg,
                                FileStatus::Untracked => theme.line_number,
                                _ => theme.foreground,
                            };
                            (format!("  {} {}", entry.status.prefix(), entry.path), color)
                        }
                    };

                    let text_pos = egui::pos2(rect.min.x + 4.0, rect.min.y + 2.0);
                    ui.painter().text(
                        text_pos,
                        egui::Align2::LEFT_TOP,
                        text,
                        egui::FontId::monospace(13.0),
                        color,
                    );
                }
            });

        // Commit input / status line at bottom
        ui.separator();
        if self.commit_input.active {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                let display = format!("> {}_", self.commit_input.message);
                ui.label(
                    egui::RichText::new(display)
                        .color(theme.foreground)
                        .monospace()
                        .size(12.0),
                );
            });
        } else if let Some((msg, _)) = &self.commit_input.result_message {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(msg)
                        .color(theme.syntax.string)
                        .monospace()
                        .size(12.0),
                );
            });
        } else if let Some((err, _)) = &self.last_error {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(err)
                        .color(theme.error_fg)
                        .monospace()
                        .size(12.0),
                );
            });
        } else {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("press c to commit")
                        .color(theme.line_number)
                        .monospace()
                        .size(12.0),
                );
            });
        }

        action
    }
}

/// Parse `git status --porcelain=v1` output into staged and unstaged entries.
fn parse_status(output: &str, staged: &mut Vec<GitFileEntry>, unstaged: &mut Vec<GitFileEntry>) {
    for line in output.lines() {
        if line.len() < 3 {
            continue;
        }
        let x = line.as_bytes()[0];
        let y = line.as_bytes()[1];
        let raw_path = &line[3..];

        // Handle renames: "R  old -> new"
        let path = if raw_path.contains(" -> ") {
            raw_path.rsplit(" -> ").next().unwrap_or(raw_path)
        } else {
            raw_path
        }
        .to_string();

        // Index status (staged)
        let staged_status = match x {
            b'M' => Some(FileStatus::Modified),
            b'A' => Some(FileStatus::Added),
            b'D' => Some(FileStatus::Deleted),
            b'R' => Some(FileStatus::Renamed),
            b'C' => Some(FileStatus::Copied),
            _ => None,
        };
        if let Some(status) = staged_status {
            staged.push(GitFileEntry {
                status,
                path: path.clone(),
            });
        }

        // Worktree status (unstaged)
        let unstaged_status = match y {
            b'M' => Some(FileStatus::Modified),
            b'D' => Some(FileStatus::Deleted),
            b'?' if x == b'?' => Some(FileStatus::Untracked),
            _ => None,
        };
        if let Some(status) = unstaged_status {
            unstaged.push(GitFileEntry { status, path });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_status_empty() {
        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        parse_status("", &mut staged, &mut unstaged);
        assert!(staged.is_empty());
        assert!(unstaged.is_empty());
    }

    #[test]
    fn parse_status_staged_modified() {
        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        parse_status("M  src/app.rs\n", &mut staged, &mut unstaged);
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].status, FileStatus::Modified);
        assert_eq!(staged[0].path, "src/app.rs");
        assert!(unstaged.is_empty());
    }

    #[test]
    fn parse_status_unstaged_modified() {
        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        parse_status(" M src/app.rs\n", &mut staged, &mut unstaged);
        assert!(staged.is_empty());
        assert_eq!(unstaged.len(), 1);
        assert_eq!(unstaged[0].status, FileStatus::Modified);
        assert_eq!(unstaged[0].path, "src/app.rs");
    }

    #[test]
    fn parse_status_both_modified() {
        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        parse_status("MM src/app.rs\n", &mut staged, &mut unstaged);
        assert_eq!(staged.len(), 1);
        assert_eq!(unstaged.len(), 1);
        assert_eq!(staged[0].status, FileStatus::Modified);
        assert_eq!(unstaged[0].status, FileStatus::Modified);
    }

    #[test]
    fn parse_status_added() {
        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        parse_status("A  new_file.rs\n", &mut staged, &mut unstaged);
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].status, FileStatus::Added);
        assert_eq!(staged[0].path, "new_file.rs");
    }

    #[test]
    fn parse_status_untracked() {
        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        parse_status("?? test.py\n", &mut staged, &mut unstaged);
        assert!(staged.is_empty());
        assert_eq!(unstaged.len(), 1);
        assert_eq!(unstaged[0].status, FileStatus::Untracked);
        assert_eq!(unstaged[0].path, "test.py");
    }

    #[test]
    fn parse_status_deleted_staged() {
        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        parse_status("D  old.rs\n", &mut staged, &mut unstaged);
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].status, FileStatus::Deleted);
    }

    #[test]
    fn parse_status_deleted_unstaged() {
        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        parse_status(" D old.rs\n", &mut staged, &mut unstaged);
        assert!(staged.is_empty());
        assert_eq!(unstaged.len(), 1);
        assert_eq!(unstaged[0].status, FileStatus::Deleted);
    }

    #[test]
    fn parse_status_renamed() {
        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        parse_status("R  old.rs -> new.rs\n", &mut staged, &mut unstaged);
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].status, FileStatus::Renamed);
        assert_eq!(staged[0].path, "new.rs");
    }

    #[test]
    fn parse_status_multiple_entries() {
        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        let output = "M  src/app.rs\n M src/main.rs\n?? test.py\nA  new.rs\n";
        parse_status(output, &mut staged, &mut unstaged);
        assert_eq!(staged.len(), 2); // M staged + A
        assert_eq!(unstaged.len(), 2); // M unstaged + ??
    }

    #[test]
    fn parse_status_short_lines_ignored() {
        let mut staged = Vec::new();
        let mut unstaged = Vec::new();
        parse_status("M\n \nAB\n", &mut staged, &mut unstaged);
        assert!(staged.is_empty());
        assert!(unstaged.is_empty());
    }

    #[test]
    fn rebuild_rows_empty() {
        let mut module = GitModule::new(None);
        module.staged.clear();
        module.unstaged.clear();
        module.rebuild_rows();
        assert!(module.rows.is_empty());
    }

    #[test]
    fn rebuild_rows_staged_only() {
        let mut module = GitModule::new(None);
        module.staged = vec![GitFileEntry {
            status: FileStatus::Modified,
            path: "a.rs".to_string(),
        }];
        module.unstaged.clear();
        module.rebuild_rows();
        assert_eq!(module.rows.len(), 2); // header + 1 file
        assert_eq!(module.rows[0], Row::StagedHeader);
        assert_eq!(module.rows[1], Row::StagedFile(0));
    }

    #[test]
    fn rebuild_rows_both_sections() {
        let mut module = GitModule::new(None);
        module.staged = vec![GitFileEntry {
            status: FileStatus::Added,
            path: "a.rs".to_string(),
        }];
        module.unstaged = vec![
            GitFileEntry {
                status: FileStatus::Modified,
                path: "b.rs".to_string(),
            },
            GitFileEntry {
                status: FileStatus::Untracked,
                path: "c.rs".to_string(),
            },
        ];
        module.selected = 999;
        module.rebuild_rows();
        // header + 1 staged + header + 2 unstaged = 5
        assert_eq!(module.rows.len(), 5);
        // Selection should be clamped
        assert_eq!(module.selected, 4);
    }

    #[test]
    fn status_prefix_all_variants() {
        assert_eq!(FileStatus::Modified.prefix(), "M");
        assert_eq!(FileStatus::Added.prefix(), "A");
        assert_eq!(FileStatus::Deleted.prefix(), "D");
        assert_eq!(FileStatus::Renamed.prefix(), "R");
        assert_eq!(FileStatus::Copied.prefix(), "C");
        assert_eq!(FileStatus::Untracked.prefix(), "?");
    }
}
