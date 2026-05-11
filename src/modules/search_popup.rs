use crate::renderer::Theme;
use eframe::egui;
use std::fs;
use std::path::{Path, PathBuf};

/// Directories to skip when walking the file tree.
const HIDDEN_DIRS: &[&str] = &[".git", "target", "node_modules"];

/// Maximum number of files to cache.
const MAX_FILES: usize = 10_000;

/// Maximum file size (bytes) for content search.
const MAX_FILE_SIZE: u64 = 1_024 * 1_024;

/// Maximum content search results.
const MAX_CONTENT_RESULTS: usize = 50;

/// Maximum visible height as fraction of screen.
const MAX_HEIGHT_FRACTION: f32 = 0.55;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Files,
    Content,
}

#[derive(Debug, Clone)]
pub enum SearchResult {
    File {
        path: PathBuf,
        display: String,
        score: i32,
        positions: Vec<usize>,
    },
    Content {
        path: PathBuf,
        display: String,
        line: usize,
        preview: String,
        col: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchAction {
    None,
    OpenFile(String),
    OpenFileAtLine(String, usize, usize),
}

pub struct SearchPopup {
    query: String,
    mode: SearchMode,
    selected: usize,
    results: Vec<SearchResult>,
    file_cache: Vec<(PathBuf, String)>,
    /// Cached file contents: (path, display_name, content)
    content_cache: Vec<(PathBuf, String, String)>,
    root: PathBuf,
    last_content_query: String,
}

// --- Fuzzy matcher ---

/// Subsequence fuzzy match with scoring.
/// Returns `Some((score, matched_positions))` or `None`.
fn fuzzy_match(pattern: &str, text: &str) -> Option<(i32, Vec<usize>)> {
    if pattern.is_empty() {
        return Some((0, vec![]));
    }

    let pattern_lower: Vec<char> = pattern.chars().flat_map(|c| c.to_lowercase()).collect();
    let text_chars: Vec<char> = text.chars().collect();
    let text_lower: Vec<char> = text.chars().flat_map(|c| c.to_lowercase()).collect();

    let mut positions = Vec::with_capacity(pattern_lower.len());
    let mut pi = 0;
    let mut score: i32 = 0;
    let mut last_match: Option<usize> = None;

    for (ti, &tc) in text_lower.iter().enumerate() {
        if pi < pattern_lower.len() && tc == pattern_lower[pi] {
            // Start-of-string bonus
            if ti == 0 {
                score += 10;
            }
            // After separator bonus
            if ti > 0 {
                let prev = text_chars[ti - 1];
                if prev == '/' || prev == '.' || prev == '_' || prev == '-' {
                    score += 5;
                }
            }
            // Consecutive match bonus
            if let Some(last) = last_match {
                if ti == last + 1 {
                    score += 3;
                }
            }
            score += 1; // per-match point
            positions.push(ti);
            last_match = Some(ti);
            pi += 1;
        } else if last_match.is_some() {
            score -= 1; // gap penalty
        }
    }

    if pi == pattern_lower.len() {
        Some((score, positions))
    } else {
        None
    }
}

// --- File walker ---

fn walk_files(root: &Path, max: usize) -> Vec<(PathBuf, String)> {
    let mut result = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            if result.len() >= max {
                return result;
            }

            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if path.is_dir() {
                if !HIDDEN_DIRS.contains(&name.as_str()) && !name.starts_with('.') {
                    stack.push(path);
                }
            } else if let Ok(rel) = path.strip_prefix(root) {
                result.push((path.clone(), rel.to_string_lossy().to_string()));
            }
        }
    }

    result
}

// --- Content search ---

fn is_binary(path: &Path) -> bool {
    let Ok(file) = fs::File::open(path) else {
        return true;
    };
    use std::io::Read;
    let mut buf = [0u8; 512];
    let Ok(n) = (&file).read(&mut buf) else {
        return true;
    };
    buf[..n].contains(&0)
}

/// Build content cache: read all eligible files into memory.
fn build_content_cache(files: &[(PathBuf, String)]) -> Vec<(PathBuf, String, String)> {
    let mut cache = Vec::new();
    for (path, display) in files {
        // Skip large files
        if let Ok(meta) = fs::metadata(path) {
            if meta.len() > MAX_FILE_SIZE {
                continue;
            }
        }
        // Skip binary files
        if is_binary(path) {
            continue;
        }
        if let Ok(content) = fs::read_to_string(path) {
            cache.push((path.clone(), display.clone(), content));
        }
    }
    cache
}

fn search_content(
    content_cache: &[(PathBuf, String, String)],
    query: &str,
    max_results: usize,
) -> Vec<SearchResult> {
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    for (path, _display, content) in content_cache {
        if results.len() >= max_results {
            break;
        }

        let file_name = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        for (line_idx, line) in content.lines().enumerate() {
            if results.len() >= max_results {
                break;
            }
            let line_lower = line.to_lowercase();
            if let Some(col) = line_lower.find(&query_lower) {
                results.push(SearchResult::Content {
                    path: path.clone(),
                    display: format!("{}:{}", file_name, line_idx + 1),
                    line: line_idx + 1,
                    preview: line.trim().to_string(),
                    col,
                });
            }
        }
    }

    results
}

impl SearchPopup {
    pub fn new(root: Option<PathBuf>) -> Self {
        let root = root.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        Self {
            query: String::new(),
            mode: SearchMode::Files,
            selected: 0,
            results: Vec::new(),
            file_cache: Vec::new(),
            content_cache: Vec::new(),
            root,
            last_content_query: String::new(),
        }
    }

    pub fn reset(&mut self) {
        self.query.clear();
        self.selected = 0;
        self.results.clear();
        self.last_content_query.clear();
    }

    pub fn set_mode(&mut self, mode: SearchMode) {
        self.mode = mode;
    }

    pub fn refresh_cache(&mut self) {
        self.file_cache = walk_files(&self.root, MAX_FILES);
        self.content_cache = build_content_cache(&self.file_cache);
    }

    fn run_file_search(&mut self) {
        if self.query.is_empty() {
            self.results.clear();
            return;
        }

        let mut scored: Vec<SearchResult> = self
            .file_cache
            .iter()
            .filter_map(|(path, display)| {
                fuzzy_match(&self.query, display).map(|(score, positions)| SearchResult::File {
                    path: path.clone(),
                    display: display.clone(),
                    score,
                    positions,
                })
            })
            .collect();

        scored.sort_by(|a, b| {
            let sa = match a {
                SearchResult::File { score, .. } => *score,
                _ => 0,
            };
            let sb = match b {
                SearchResult::File { score, .. } => *score,
                _ => 0,
            };
            sb.cmp(&sa)
        });

        scored.truncate(100);
        self.results = scored;
    }

    fn run_content_search(&mut self) {
        if self.query.is_empty() {
            self.results.clear();
            self.last_content_query.clear();
            return;
        }

        // Don't re-run if query hasn't changed
        if self.query == self.last_content_query {
            return;
        }

        self.last_content_query = self.query.clone();
        self.results = search_content(&self.content_cache, &self.query, MAX_CONTENT_RESULTS);
    }

    /// Handle input. Returns `(should_close, action)`.
    pub fn handle_input(&mut self, ctx: &egui::Context) -> (bool, SearchAction) {
        let mut should_close = false;
        let mut action = SearchAction::None;
        let mut query_changed = false;
        let mut mode_toggled = false;

        ctx.input(|input| {
            if input.key_pressed(egui::Key::Escape) {
                should_close = true;
                return;
            }
            if input.key_pressed(egui::Key::ArrowDown) {
                let count = self.results.len();
                if count > 0 && self.selected < count - 1 {
                    self.selected += 1;
                }
                return;
            }
            if input.key_pressed(egui::Key::ArrowUp) {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                return;
            }
            if input.key_pressed(egui::Key::Enter) {
                if let Some(result) = self.results.get(self.selected) {
                    action = match result {
                        SearchResult::File { path, .. } => {
                            SearchAction::OpenFile(path.to_string_lossy().to_string())
                        }
                        SearchResult::Content {
                            path, line, col, ..
                        } => SearchAction::OpenFileAtLine(
                            path.to_string_lossy().to_string(),
                            *line,
                            *col,
                        ),
                    };
                }
                should_close = true;
                return;
            }
            if input.key_pressed(egui::Key::Tab) {
                mode_toggled = true;
                return;
            }
            if input.key_pressed(egui::Key::Backspace) {
                self.query.pop();
                self.selected = 0;
                query_changed = true;
                return;
            }
            // Text input
            for event in &input.events {
                if let egui::Event::Text(text) = event {
                    if !input.modifiers.command && !input.modifiers.ctrl {
                        self.query.push_str(text);
                        self.selected = 0;
                        query_changed = true;
                    }
                }
            }
        });

        if mode_toggled {
            self.mode = match self.mode {
                SearchMode::Files => SearchMode::Content,
                SearchMode::Content => SearchMode::Files,
            };
            self.selected = 0;
            self.last_content_query.clear();
            query_changed = true;
        }

        if query_changed {
            match self.mode {
                SearchMode::Files => self.run_file_search(),
                SearchMode::Content => self.run_content_search(),
            }
        }

        (should_close, action)
    }

    /// Render the search popup overlay.
    pub fn render(&self, ctx: &egui::Context, theme: &Theme) {
        let screen = ctx.screen_rect();

        // Dim layer
        egui::Area::new(egui::Id::new("search_dim"))
            .fixed_pos(screen.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.painter()
                    .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(160));
            });

        // Search overlay — top-center
        let popup_width = (screen.width() * 0.6).clamp(400.0, 800.0);
        let popup_x = (screen.width() - popup_width) / 2.0;
        let popup_y = screen.height() * 0.12;

        egui::Area::new(egui::Id::new("search_overlay"))
            .fixed_pos(egui::pos2(popup_x, popup_y))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::default()
                    .fill(theme.status_bar_bg)
                    .stroke(egui::Stroke::new(1.0, theme.line_number))
                    .corner_radius(8.0)
                    .inner_margin(12.0)
                    .show(ui, |ui| {
                        ui.set_width(popup_width);
                        ui.set_min_height(screen.height() * MAX_HEIGHT_FRACTION);

                        // Top row: query + mode indicator
                        ui.horizontal(|ui| {
                            // Query display
                            let query_display = if self.query.is_empty() {
                                let placeholder = match self.mode {
                                    SearchMode::Files => "Search files...",
                                    SearchMode::Content => "Search in files...",
                                };
                                egui::RichText::new(placeholder)
                                    .color(theme.line_number)
                                    .italics()
                                    .size(14.0)
                            } else {
                                egui::RichText::new(format!("> {}", self.query))
                                    .color(theme.foreground)
                                    .size(14.0)
                            };
                            ui.label(query_display);

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let content_color = if self.mode == SearchMode::Content {
                                        theme.syntax.keyword
                                    } else {
                                        theme.line_number
                                    };
                                    ui.label(
                                        egui::RichText::new("Content")
                                            .color(content_color)
                                            .size(12.0),
                                    );
                                    ui.label(
                                        egui::RichText::new("|")
                                            .color(theme.line_number)
                                            .size(12.0),
                                    );
                                    let files_color = if self.mode == SearchMode::Files {
                                        theme.syntax.keyword
                                    } else {
                                        theme.line_number
                                    };
                                    ui.label(
                                        egui::RichText::new("Files").color(files_color).size(12.0),
                                    );
                                },
                            );
                        });

                        ui.separator();

                        // Results
                        if self.results.is_empty() {
                            let msg = if self.query.is_empty() {
                                "Type to search"
                            } else {
                                "No results"
                            };
                            ui.label(egui::RichText::new(msg).color(theme.line_number).size(13.0));
                        } else {
                            let row_height = match self.mode {
                                SearchMode::Files => 28.0,
                                SearchMode::Content => 40.0,
                            };
                            let max_height = screen.height() * MAX_HEIGHT_FRACTION;

                            egui::ScrollArea::vertical()
                                .max_height(max_height)
                                .show(ui, |ui| {
                                    for (idx, result) in self.results.iter().enumerate() {
                                        let is_selected = idx == self.selected;
                                        let (rect, response) = ui.allocate_exact_size(
                                            egui::vec2(ui.available_width(), row_height),
                                            egui::Sense::hover(),
                                        );

                                        if is_selected {
                                            response.scroll_to_me(None);
                                            ui.painter().rect_filled(rect, 4.0, theme.selection);
                                        }

                                        match result {
                                            SearchResult::File {
                                                display, positions, ..
                                            } => {
                                                self.render_fuzzy_text(
                                                    ui, &rect, display, positions, theme,
                                                );
                                            }
                                            SearchResult::Content {
                                                display, preview, ..
                                            } => {
                                                // Line 1: filename:line
                                                ui.painter().text(
                                                    egui::pos2(rect.min.x + 8.0, rect.min.y + 3.0),
                                                    egui::Align2::LEFT_TOP,
                                                    display,
                                                    egui::FontId::monospace(12.0),
                                                    theme.syntax.keyword,
                                                );

                                                // Line 2: preview text
                                                let truncated: String =
                                                    preview.chars().take(80).collect();
                                                ui.painter().text(
                                                    egui::pos2(
                                                        rect.min.x + 16.0,
                                                        rect.min.y + 19.0,
                                                    ),
                                                    egui::Align2::LEFT_TOP,
                                                    &truncated,
                                                    egui::FontId::monospace(12.0),
                                                    theme.foreground,
                                                );
                                            }
                                        }
                                    }
                                });
                        }
                    });
            });
    }

    fn render_fuzzy_text(
        &self,
        ui: &egui::Ui,
        rect: &egui::Rect,
        text: &str,
        positions: &[usize],
        theme: &Theme,
    ) {
        let font = egui::FontId::monospace(13.0);
        let mut x = rect.min.x + 8.0;
        let y = rect.min.y + 3.0;

        for (i, ch) in text.chars().enumerate() {
            let color = if positions.contains(&i) {
                theme.syntax.keyword
            } else {
                theme.foreground
            };
            let galley = ui
                .painter()
                .layout_no_wrap(ch.to_string(), font.clone(), color);
            ui.painter().galley(egui::pos2(x, y), galley.clone(), color);
            x += galley.size().x;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Fuzzy match tests ---

    #[test]
    fn fuzzy_exact_prefix() {
        let (score, positions) = fuzzy_match("src", "src/main.rs").unwrap();
        assert!(score > 0);
        assert_eq!(positions, vec![0, 1, 2]);
    }

    #[test]
    fn fuzzy_mid_string() {
        let (score, positions) = fuzzy_match("main", "src/main.rs").unwrap();
        assert!(score > 0);
        assert!(!positions.is_empty());
        // 'm' is at index 4
        assert_eq!(positions[0], 4);
    }

    #[test]
    fn fuzzy_case_insensitive() {
        let result = fuzzy_match("SRC", "src/main.rs");
        assert!(result.is_some());
    }

    #[test]
    fn fuzzy_no_match() {
        let result = fuzzy_match("xyz", "src/main.rs");
        assert!(result.is_none());
    }

    #[test]
    fn fuzzy_empty_pattern() {
        let (score, positions) = fuzzy_match("", "anything").unwrap();
        assert_eq!(score, 0);
        assert!(positions.is_empty());
    }

    #[test]
    fn fuzzy_unicode() {
        let result = fuzzy_match("åäö", "test_åäö_file.rs");
        assert!(result.is_some());
        let (_, positions) = result.unwrap();
        assert_eq!(positions.len(), 3);
    }

    #[test]
    fn fuzzy_separator_bonus() {
        // Match after separator should score higher
        let (score_sep, _) = fuzzy_match("m", "src/main.rs").unwrap();
        let (score_mid, _) = fuzzy_match("a", "src/main.rs").unwrap();
        // 'm' after '/' gets separator bonus, 'a' mid-word doesn't
        assert!(score_sep > score_mid);
    }

    // --- File walker tests ---

    #[test]
    fn walker_respects_hidden_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join(".git");
        fs::create_dir(&git_dir).unwrap();
        fs::write(git_dir.join("config"), "test").unwrap();

        let src_dir = dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();
        fs::write(src_dir.join("main.rs"), "fn main() {}").unwrap();

        let files = walk_files(dir.path(), MAX_FILES);
        assert!(files.iter().any(|(_, d)| d.contains("main.rs")));
        assert!(!files.iter().any(|(_, d)| d.contains(".git")));
    }

    #[test]
    fn walker_respects_max_cap() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..20 {
            fs::write(dir.path().join(format!("file{}.txt", i)), "content").unwrap();
        }

        let files = walk_files(dir.path(), 5);
        assert_eq!(files.len(), 5);
    }

    // --- Content search tests ---

    #[test]
    fn content_search_finds_matches() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.rs");
        fs::write(&file_path, "fn hello_world() {\n    println!(\"hi\");\n}\n").unwrap();

        let files = walk_files(dir.path(), MAX_FILES);
        let cache = build_content_cache(&files);
        let results = search_content(&cache, "hello", MAX_CONTENT_RESULTS);
        assert_eq!(results.len(), 1);
        match &results[0] {
            SearchResult::Content { line, col, .. } => {
                assert_eq!(*line, 1);
                assert_eq!(*col, 3); // "fn hello..." — 'h' at col 3
            }
            _ => panic!("Expected Content result"),
        }
    }

    #[test]
    fn content_search_respects_limit() {
        let dir = tempfile::tempdir().unwrap();
        let mut content = String::new();
        for i in 0..100 {
            content.push_str(&format!("match line {}\n", i));
        }
        fs::write(dir.path().join("big.txt"), &content).unwrap();

        let files = walk_files(dir.path(), MAX_FILES);
        let cache = build_content_cache(&files);
        let results = search_content(&cache, "match", 5);
        assert_eq!(results.len(), 5);
    }

    // --- SearchPopup state tests ---

    #[test]
    fn reset_clears_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut popup = SearchPopup::new(Some(dir.path().to_path_buf()));
        popup.query = "test".to_string();
        popup.selected = 3;
        popup.mode = SearchMode::Content;
        popup.reset();
        assert!(popup.query.is_empty());
        assert_eq!(popup.selected, 0);
        assert!(popup.results.is_empty());
    }

    #[test]
    fn mode_toggle() {
        let dir = tempfile::tempdir().unwrap();
        let mut popup = SearchPopup::new(Some(dir.path().to_path_buf()));
        assert_eq!(popup.mode, SearchMode::Files);
        popup.mode = match popup.mode {
            SearchMode::Files => SearchMode::Content,
            SearchMode::Content => SearchMode::Files,
        };
        assert_eq!(popup.mode, SearchMode::Content);
    }

    #[test]
    fn file_search_returns_results() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir(&src).unwrap();
        fs::write(src.join("main.rs"), "fn main() {}").unwrap();
        fs::write(src.join("lib.rs"), "pub mod lib;").unwrap();

        let mut popup = SearchPopup::new(Some(dir.path().to_path_buf()));
        popup.refresh_cache();
        popup.query = "main".to_string();
        popup.run_file_search();
        assert!(!popup.results.is_empty());
        match &popup.results[0] {
            SearchResult::File { display, .. } => {
                assert!(display.contains("main"));
            }
            _ => panic!("Expected File result"),
        }
    }
}
