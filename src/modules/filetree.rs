use crate::modules::ModuleAction;
use crate::renderer::Theme;
use eframe::egui;
use std::path::PathBuf;

/// Directories hidden by default in the filetree.
const HIDDEN_DIRS: &[&str] = &[".git", "target", "node_modules"];

pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub depth: usize,
    pub expanded: bool,
}

pub struct FiletreeModule {
    root: Option<PathBuf>,
    entries: Vec<FileEntry>,
    selected: usize,
    needs_refresh: bool,
    /// Tracks which directories are expanded (by canonical path).
    expanded_dirs: std::collections::HashSet<PathBuf>,
}

impl FiletreeModule {
    pub fn new(root: Option<PathBuf>) -> Self {
        let mut module = Self {
            root,
            entries: Vec::new(),
            selected: 0,
            needs_refresh: true,
            expanded_dirs: std::collections::HashSet::new(),
        };
        module.refresh();
        module
    }

    pub fn refresh(&mut self) {
        self.entries.clear();
        if let Some(root) = self.root.clone() {
            self.read_dir(&root, 0);
        }
        self.needs_refresh = false;
        // Clamp selection
        if !self.entries.is_empty() && self.selected >= self.entries.len() {
            self.selected = self.entries.len() - 1;
        }
    }

    fn read_dir(&mut self, dir: &std::path::Path, depth: usize) {
        let Ok(read_dir) = std::fs::read_dir(dir) else {
            return;
        };

        let mut children: Vec<(String, PathBuf, bool)> = Vec::new();
        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path();
            let is_dir = path.is_dir();

            // Skip hidden dirs
            if is_dir && HIDDEN_DIRS.contains(&name.as_str()) {
                continue;
            }

            children.push((name, path, is_dir));
        }

        // Sort: dirs first, then alphabetical. Dotfiles sorted last within each group.
        children.sort_by(|a, b| {
            let a_dir = a.2;
            let b_dir = b.2;
            let a_dot = a.0.starts_with('.');
            let b_dot = b.0.starts_with('.');

            // Dirs before files
            if a_dir != b_dir {
                return b_dir.cmp(&a_dir);
            }
            // Non-dotfiles before dotfiles
            if a_dot != b_dot {
                return a_dot.cmp(&b_dot);
            }
            // Alphabetical (case-insensitive)
            a.0.to_lowercase().cmp(&b.0.to_lowercase())
        });

        for (name, path, is_dir) in children {
            let expanded = is_dir && self.expanded_dirs.contains(&path);
            self.entries.push(FileEntry {
                name,
                path: path.clone(),
                is_dir,
                depth,
                expanded,
            });
            if expanded {
                self.read_dir(&path, depth + 1);
            }
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if !self.entries.is_empty() && self.selected < self.entries.len() - 1 {
            self.selected += 1;
        }
    }

    fn toggle_expand(&mut self) {
        if self.selected >= self.entries.len() {
            return;
        }
        let entry = &self.entries[self.selected];
        if !entry.is_dir {
            return;
        }
        let path = entry.path.clone();
        if self.expanded_dirs.contains(&path) {
            self.expanded_dirs.remove(&path);
        } else {
            self.expanded_dirs.insert(path);
        }
        self.refresh();
    }

    /// Handle input when the filetree panel is focused.
    /// Returns a `ModuleAction` indicating what the app should do.
    pub fn handle_input(&mut self, ctx: &egui::Context) -> ModuleAction {
        let mut action = ModuleAction::None;
        let mut want_toggle = false;

        ctx.input(|input| {
            if input.key_pressed(egui::Key::J) || input.key_pressed(egui::Key::ArrowDown) {
                self.move_down();
                return;
            }
            if input.key_pressed(egui::Key::K) || input.key_pressed(egui::Key::ArrowUp) {
                self.move_up();
                return;
            }
            if input.key_pressed(egui::Key::Enter) || input.key_pressed(egui::Key::L) {
                if let Some(entry) = self.entries.get(self.selected) {
                    if entry.is_dir {
                        want_toggle = true;
                    } else {
                        action = ModuleAction::OpenFile(entry.path.to_string_lossy().to_string());
                    }
                }
                return;
            }
            if input.key_pressed(egui::Key::H) {
                if let Some(entry) = self.entries.get(self.selected) {
                    if entry.is_dir && entry.expanded {
                        want_toggle = true;
                    }
                }
            }
        });

        if want_toggle {
            self.toggle_expand();
        }

        action
    }

    /// Render the filetree in the given UI region.
    /// Returns a `ModuleAction` if a file was clicked to open.
    pub fn render(&mut self, ui: &mut egui::Ui, theme: &Theme, focused: bool) -> ModuleAction {
        let mut action = ModuleAction::None;

        // Header
        let header_color = if focused {
            theme.foreground
        } else {
            theme.line_number
        };
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("EXPLORER")
                    .color(header_color)
                    .size(11.0)
                    .strong(),
            );
        });
        ui.add_space(4.0);

        // Entries
        let mut clicked_idx: Option<usize> = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (idx, entry) in self.entries.iter().enumerate() {
                    let is_selected = idx == self.selected && focused;
                    let indent = entry.depth as f32 * 16.0;

                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 20.0),
                        egui::Sense::click(),
                    );

                    // Hover highlight
                    if response.hovered() && !is_selected {
                        ui.painter()
                            .rect_filled(rect, 0.0, theme.selection.linear_multiply(0.5));
                    }

                    // Selected row background
                    if is_selected {
                        ui.painter().rect_filled(rect, 0.0, theme.selection);
                    }

                    if response.clicked() {
                        clicked_idx = Some(idx);
                    }

                    // Build label text
                    let prefix = if entry.is_dir {
                        if entry.expanded {
                            "\u{25be} "
                        } else {
                            "\u{25b8} "
                        }
                    } else {
                        "  "
                    };

                    let text_color = if entry.is_dir {
                        theme.syntax.keyword
                    } else {
                        theme.foreground
                    };

                    let text_pos = egui::pos2(rect.min.x + indent + 4.0, rect.min.y + 2.0);
                    ui.painter().text(
                        text_pos,
                        egui::Align2::LEFT_TOP,
                        format!("{}{}", prefix, entry.name),
                        egui::FontId::monospace(13.0),
                        text_color,
                    );
                }
            });

        // Handle click outside the iteration (needs &mut self)
        if let Some(idx) = clicked_idx {
            self.selected = idx;
            if let Some(entry) = self.entries.get(idx) {
                if entry.is_dir {
                    self.toggle_expand();
                } else {
                    action = ModuleAction::OpenFile(entry.path.to_string_lossy().to_string());
                }
            }
        }

        action
    }

    #[cfg(test)]
    pub fn entries(&self) -> &[FileEntry] {
        &self.entries
    }

    #[cfg(test)]
    pub fn selected(&self) -> usize {
        self.selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_dir() -> TempDir {
        let tmp = TempDir::new().unwrap();
        // Create some files and dirs
        std::fs::create_dir(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src").join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(tmp.path().join("Cargo.toml"), "[package]").unwrap();
        std::fs::write(tmp.path().join(".gitignore"), "target/").unwrap();
        std::fs::create_dir(tmp.path().join("target")).unwrap();
        std::fs::write(tmp.path().join("target").join("debug"), "").unwrap();
        tmp
    }

    #[test]
    fn refresh_reads_directory() {
        let tmp = create_test_dir();
        let ft = FiletreeModule::new(Some(tmp.path().to_path_buf()));
        let entries = ft.entries();
        // Should have: src/ dir, Cargo.toml, .gitignore (target/ is hidden)
        assert!(entries.len() >= 2);
        // First entry should be the dir (dirs come first)
        assert!(entries[0].is_dir);
        assert_eq!(entries[0].name, "src");
    }

    #[test]
    fn hidden_dirs_excluded() {
        let tmp = create_test_dir();
        let ft = FiletreeModule::new(Some(tmp.path().to_path_buf()));
        let names: Vec<&str> = ft.entries().iter().map(|e| e.name.as_str()).collect();
        assert!(!names.contains(&"target"));
    }

    #[test]
    fn navigation_clamps() {
        let tmp = create_test_dir();
        let mut ft = FiletreeModule::new(Some(tmp.path().to_path_buf()));
        let len = ft.entries().len();

        // Move up at top stays at 0
        ft.move_up();
        assert_eq!(ft.selected(), 0);

        // Move to end
        for _ in 0..len + 5 {
            ft.move_down();
        }
        assert_eq!(ft.selected(), len - 1);

        // Move down at bottom stays at end
        ft.move_down();
        assert_eq!(ft.selected(), len - 1);
    }

    #[test]
    fn toggle_expand_adds_children() {
        let tmp = create_test_dir();
        let mut ft = FiletreeModule::new(Some(tmp.path().to_path_buf()));

        // First entry should be src/ dir
        assert!(ft.entries()[0].is_dir);
        assert!(!ft.entries()[0].expanded);
        let initial_count = ft.entries().len();

        // Toggle expand on dir
        ft.toggle_expand();
        assert!(ft.entries()[0].expanded);
        assert!(ft.entries().len() > initial_count);

        // Toggle collapse
        ft.toggle_expand();
        assert!(!ft.entries()[0].expanded);
        assert_eq!(ft.entries().len(), initial_count);
    }

    #[test]
    fn dotfiles_sorted_last() {
        let tmp = create_test_dir();
        let ft = FiletreeModule::new(Some(tmp.path().to_path_buf()));
        // Files: Cargo.toml should come before .gitignore
        let file_entries: Vec<&str> = ft
            .entries()
            .iter()
            .filter(|e| !e.is_dir)
            .map(|e| e.name.as_str())
            .collect();
        if file_entries.len() >= 2 {
            let cargo_idx = file_entries.iter().position(|n| *n == "Cargo.toml");
            let gitignore_idx = file_entries.iter().position(|n| *n == ".gitignore");
            if let (Some(ci), Some(gi)) = (cargo_idx, gitignore_idx) {
                assert!(ci < gi, "Cargo.toml should sort before .gitignore");
            }
        }
    }

    #[test]
    fn empty_root_produces_empty_entries() {
        let ft = FiletreeModule::new(None);
        assert!(ft.entries().is_empty());
    }
}
