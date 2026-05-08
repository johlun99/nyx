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
    /// Whether search mode is active.
    searching: bool,
    /// Current search/filter query.
    search_query: String,
    /// Indices into `entries` that match the current search query.
    filtered: Vec<usize>,
    /// Index into `filtered` for the currently selected match.
    filtered_selected: usize,
    /// Last directory opened via search — its children are prioritized in next search.
    search_context: Option<PathBuf>,
}

impl FiletreeModule {
    pub fn new(root: Option<PathBuf>) -> Self {
        let mut module = Self {
            root,
            entries: Vec::new(),
            selected: 0,
            needs_refresh: true,
            expanded_dirs: std::collections::HashSet::new(),
            searching: false,
            search_query: String::new(),
            filtered: Vec::new(),
            filtered_selected: 0,
            search_context: None,
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
        // Re-apply filter if search is active
        if self.searching && !self.search_query.is_empty() {
            self.update_filter();
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

    /// Whether search mode is active.
    fn is_searching(&self) -> bool {
        self.searching
    }

    /// Rebuild the filtered indices from the current search query.
    /// Entries under `search_context` are sorted first.
    fn update_filter(&mut self) {
        let query = self.search_query.to_lowercase();
        let mut context_matches: Vec<usize> = Vec::new();
        let mut other_matches: Vec<usize> = Vec::new();

        for (i, entry) in self.entries.iter().enumerate() {
            if entry.name.to_lowercase().contains(&query) {
                if let Some(ctx) = &self.search_context {
                    if entry.path.starts_with(ctx) {
                        context_matches.push(i);
                        continue;
                    }
                }
                other_matches.push(i);
            }
        }

        self.filtered = context_matches;
        self.filtered.extend(other_matches);

        // Clamp selection and sync
        if self.filtered.is_empty() {
            self.filtered_selected = 0;
        } else {
            if self.filtered_selected >= self.filtered.len() {
                self.filtered_selected = self.filtered.len() - 1;
            }
            self.selected = self.filtered[self.filtered_selected];
        }
    }

    /// Clear search state and return to normal navigation.
    fn clear_search(&mut self) {
        self.searching = false;
        self.search_query.clear();
        self.filtered.clear();
        self.filtered_selected = 0;
    }

    pub fn move_up(&mut self) {
        if self.is_searching() {
            if self.filtered_selected > 0 {
                self.filtered_selected -= 1;
                self.selected = self.filtered[self.filtered_selected];
            }
        } else if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.is_searching() {
            if !self.filtered.is_empty() && self.filtered_selected < self.filtered.len() - 1 {
                self.filtered_selected += 1;
                self.selected = self.filtered[self.filtered_selected];
            }
        } else if !self.entries.is_empty() && self.selected < self.entries.len() - 1 {
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
        let mut search_text: Option<String> = None;

        ctx.input(|input| {
            // Escape: clear search or let app handle
            if input.key_pressed(egui::Key::Escape) {
                if self.is_searching() {
                    self.clear_search();
                }
                // If not searching, escape is handled by app (unfocus panel)
                return;
            }

            // Arrow keys always navigate (both in search and normal mode)
            if input.key_pressed(egui::Key::ArrowDown) {
                self.move_down();
                return;
            }
            if input.key_pressed(egui::Key::ArrowUp) {
                self.move_up();
                return;
            }

            if self.is_searching() {
                // In search mode: special key handling
                if input.key_pressed(egui::Key::Backspace) {
                    self.search_query.pop();
                    if self.search_query.is_empty() {
                        self.clear_search();
                    } else {
                        self.update_filter();
                    }
                    return;
                }

                if input.key_pressed(egui::Key::Enter) {
                    // Open selected entry and clear search
                    if let Some(entry) = self.entries.get(self.selected) {
                        if entry.is_dir {
                            // Remember this dir as search context for next search
                            self.search_context = Some(entry.path.clone());
                            want_toggle = true;
                        } else {
                            action =
                                ModuleAction::OpenFile(entry.path.to_string_lossy().to_string());
                        }
                    }
                    self.clear_search();
                    return;
                }

                // Collect text input for search
                for event in &input.events {
                    if let egui::Event::Text(text) = event {
                        search_text = Some(text.clone());
                    }
                }
            } else {
                // Normal mode: vim-style navigation
                if input.key_pressed(egui::Key::J) {
                    self.move_down();
                    return;
                }
                if input.key_pressed(egui::Key::K) {
                    self.move_up();
                    return;
                }
                if input.key_pressed(egui::Key::Enter) || input.key_pressed(egui::Key::L) {
                    if let Some(entry) = self.entries.get(self.selected) {
                        if entry.is_dir {
                            want_toggle = true;
                        } else {
                            action =
                                ModuleAction::OpenFile(entry.path.to_string_lossy().to_string());
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
                    return;
                }

                // / activates search mode (don't add / to the query)
                if input.key_pressed(egui::Key::Slash) {
                    self.searching = true;
                    self.search_query.clear();
                    return;
                }

                // Any other text input starts search (speed-search)
                for event in &input.events {
                    if let egui::Event::Text(text) = event {
                        // Skip nav keys that are already handled above
                        if text.len() == 1 {
                            let ch = text.chars().next().unwrap_or(' ');
                            if matches!(ch, 'j' | 'k' | 'h' | 'l') {
                                continue;
                            }
                        }
                        self.searching = true;
                        search_text = Some(text.clone());
                    }
                }
            }
        });

        // Apply search text outside the input closure
        if let Some(text) = search_text {
            self.search_query.push_str(&text);
            self.filtered_selected = 0;
            self.update_filter();
        }

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

        // Search bar (when active)
        if self.is_searching() {
            ui.horizontal(|ui| {
                ui.add_space(4.0);
                let search_display = format!("/{}", self.search_query);
                ui.label(
                    egui::RichText::new(&search_display)
                        .color(theme.syntax.string)
                        .monospace()
                        .size(12.0),
                );
            });
            ui.add_space(2.0);
        } else {
            ui.add_space(4.0);
        }

        // Determine which entries to show
        let visible_indices: Vec<usize> = if self.is_searching() && !self.search_query.is_empty() {
            self.filtered.clone()
        } else {
            (0..self.entries.len()).collect()
        };

        // Entries
        let mut clicked_idx: Option<usize> = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for &entry_idx in &visible_indices {
                    let entry = &self.entries[entry_idx];
                    let is_selected = entry_idx == self.selected && focused;
                    let indent = if self.is_searching() && !self.search_query.is_empty() {
                        0.0 // Flat list when filtering
                    } else {
                        entry.depth as f32 * 16.0
                    };

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
                        clicked_idx = Some(entry_idx);
                    }

                    // Build label text
                    let (prefix, display_name) =
                        if self.is_searching() && !self.search_query.is_empty() {
                            // In search mode, show relative path for context
                            let rel = self
                                .root
                                .as_ref()
                                .and_then(|r| entry.path.strip_prefix(r).ok())
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_else(|| entry.name.clone());
                            let prefix = if entry.is_dir { "\u{25b8} " } else { "  " };
                            (prefix, rel)
                        } else {
                            let prefix = if entry.is_dir {
                                if entry.expanded {
                                    "\u{25be} "
                                } else {
                                    "\u{25b8} "
                                }
                            } else {
                                "  "
                            };
                            (prefix, entry.name.clone())
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
                        format!("{}{}", prefix, display_name),
                        egui::FontId::monospace(13.0),
                        text_color,
                    );
                }
            });

        // Handle click outside the iteration (needs &mut self)
        if let Some(idx) = clicked_idx {
            self.selected = idx;
            if self.is_searching() {
                // Update filtered_selected to match
                if let Some(pos) = self.filtered.iter().position(|&i| i == idx) {
                    self.filtered_selected = pos;
                }
            }
            let entry_info = self.entries.get(idx).map(|e| (e.is_dir, e.path.clone()));
            if let Some((is_dir, path)) = entry_info {
                if is_dir {
                    if self.is_searching() {
                        self.search_context = Some(path);
                        self.clear_search();
                    }
                    self.toggle_expand();
                } else {
                    if self.is_searching() {
                        self.clear_search();
                    }
                    action = ModuleAction::OpenFile(path.to_string_lossy().to_string());
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

    #[cfg(test)]
    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    #[cfg(test)]
    pub fn filtered_indices(&self) -> &[usize] {
        &self.filtered
    }

    #[cfg(test)]
    pub fn set_search(&mut self, query: &str) {
        self.searching = true;
        self.search_query = query.to_string();
        self.filtered_selected = 0;
        self.update_filter();
    }

    #[cfg(test)]
    pub fn set_search_context(&mut self, path: PathBuf) {
        self.search_context = Some(path);
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

    #[test]
    fn search_filters_entries() {
        let tmp = create_test_dir();
        let mut ft = FiletreeModule::new(Some(tmp.path().to_path_buf()));
        // Entries: src/, Cargo.toml, .gitignore
        ft.set_search("cargo");
        assert_eq!(ft.filtered_indices().len(), 1);
        let matched = &ft.entries()[ft.filtered_indices()[0]];
        assert_eq!(matched.name, "Cargo.toml");
    }

    #[test]
    fn search_is_case_insensitive() {
        let tmp = create_test_dir();
        let mut ft = FiletreeModule::new(Some(tmp.path().to_path_buf()));
        ft.set_search("CARGO");
        assert_eq!(ft.filtered_indices().len(), 1);
        assert_eq!(ft.entries()[ft.filtered_indices()[0]].name, "Cargo.toml");
    }

    #[test]
    fn search_no_match_returns_empty() {
        let tmp = create_test_dir();
        let mut ft = FiletreeModule::new(Some(tmp.path().to_path_buf()));
        ft.set_search("zzzzz");
        assert!(ft.filtered_indices().is_empty());
    }

    #[test]
    fn search_context_prioritizes_dir_children() {
        let tmp = create_test_dir();
        // Expand src/ so its children are visible
        let mut ft = FiletreeModule::new(Some(tmp.path().to_path_buf()));
        ft.toggle_expand(); // expands src/
                            // Now entries: src/, src/main.rs, Cargo.toml, .gitignore

        // Set search context to src/
        ft.set_search_context(tmp.path().join("src"));

        // Search for something that matches both inside and outside src/
        // "main" matches src/main.rs
        ft.set_search("main");
        assert!(!ft.filtered_indices().is_empty());
        // The first match should be src/main.rs (inside context dir)
        let first_match = &ft.entries()[ft.filtered_indices()[0]];
        assert_eq!(first_match.name, "main.rs");
    }

    #[test]
    fn search_navigate_up_down() {
        let tmp = create_test_dir();
        let mut ft = FiletreeModule::new(Some(tmp.path().to_path_buf()));
        // Expand src/
        ft.toggle_expand();
        // Entries: src/, src/main.rs, Cargo.toml, .gitignore

        // Search for entries with "." — matches .gitignore, Cargo.toml (has .)
        // Actually let's search for something broader
        ft.set_search("s"); // matches src/, src/main.rs, .gitignore (has 's'... no)
                            // "s" matches: "src" (has s), "main.rs" (has s)... actually just check
        let count = ft.filtered_indices().len();
        if count > 1 {
            let first_selected = ft.selected();
            ft.move_down();
            assert_ne!(ft.selected(), first_selected);
            ft.move_up();
            assert_eq!(ft.selected(), first_selected);
        }
    }

    #[test]
    fn clear_search_resets_state() {
        let tmp = create_test_dir();
        let mut ft = FiletreeModule::new(Some(tmp.path().to_path_buf()));
        ft.set_search("cargo");
        assert!(!ft.search_query().is_empty());
        assert!(!ft.filtered_indices().is_empty());

        ft.clear_search();
        assert!(ft.search_query().is_empty());
        assert!(ft.filtered_indices().is_empty());
    }
}
