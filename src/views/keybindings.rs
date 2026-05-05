pub struct KeybindingEntry {
    pub category: &'static str,
    pub action: &'static str,
    pub key: &'static str,
}

pub struct KeybindingsView {
    pub search: String,
    pub entries: Vec<KeybindingEntry>,
}

impl KeybindingsView {
    pub fn new() -> Self {
        Self {
            search: String::new(),
            entries: Self::all_entries(),
        }
    }

    fn all_entries() -> Vec<KeybindingEntry> {
        vec![
            // Navigation
            KeybindingEntry { category: "Navigation", action: "Move left", key: "h" },
            KeybindingEntry { category: "Navigation", action: "Move down", key: "j" },
            KeybindingEntry { category: "Navigation", action: "Move up", key: "k" },
            KeybindingEntry { category: "Navigation", action: "Move right", key: "l" },
            KeybindingEntry { category: "Navigation", action: "Word forward", key: "w" },
            KeybindingEntry { category: "Navigation", action: "Word backward", key: "b" },
            KeybindingEntry { category: "Navigation", action: "Word end", key: "e" },
            KeybindingEntry { category: "Navigation", action: "Line start", key: "0" },
            KeybindingEntry { category: "Navigation", action: "Line end", key: "$" },
            KeybindingEntry { category: "Navigation", action: "First non-blank", key: "^" },
            KeybindingEntry { category: "Navigation", action: "File top", key: "gg" },
            KeybindingEntry { category: "Navigation", action: "File bottom", key: "G" },
            // Editing
            KeybindingEntry { category: "Editing", action: "Delete char", key: "x" },
            KeybindingEntry { category: "Editing", action: "Delete line", key: "dd" },
            KeybindingEntry { category: "Editing", action: "Change line", key: "cc" },
            KeybindingEntry { category: "Editing", action: "Yank line", key: "yy" },
            KeybindingEntry { category: "Editing", action: "Paste", key: "p" },
            KeybindingEntry { category: "Editing", action: "Undo", key: "u" },
            KeybindingEntry { category: "Editing", action: "Redo", key: "Ctrl+R" },
            KeybindingEntry { category: "Editing", action: "Repeat last", key: "." },
            KeybindingEntry { category: "Editing", action: "Search forward", key: "/" },
            KeybindingEntry { category: "Editing", action: "Search backward", key: "?" },
            KeybindingEntry { category: "Editing", action: "Next match", key: "n" },
            KeybindingEntry { category: "Editing", action: "Previous match", key: "N" },
            // Modes
            KeybindingEntry { category: "Modes", action: "Insert", key: "i" },
            KeybindingEntry { category: "Modes", action: "Append", key: "a" },
            KeybindingEntry { category: "Modes", action: "Append end of line", key: "A" },
            KeybindingEntry { category: "Modes", action: "Insert first non-blank", key: "I" },
            KeybindingEntry { category: "Modes", action: "Open below", key: "o" },
            KeybindingEntry { category: "Modes", action: "Open above", key: "O" },
            KeybindingEntry { category: "Modes", action: "Visual", key: "v" },
            KeybindingEntry { category: "Modes", action: "Visual line", key: "V" },
            KeybindingEntry { category: "Modes", action: "Visual block", key: "Ctrl+V" },
            KeybindingEntry { category: "Modes", action: "Command", key: ":" },
            // App
            KeybindingEntry { category: "App", action: "Settings", key: "\u{2318}," },
            KeybindingEntry { category: "App", action: "Keybindings", key: "\u{2318}K" },
            KeybindingEntry { category: "App", action: "Save", key: ":w" },
            KeybindingEntry { category: "App", action: "Quit", key: ":q" },
            KeybindingEntry { category: "App", action: "Force quit", key: ":q!" },
        ]
    }

    pub fn filtered_entries(&self) -> Vec<&KeybindingEntry> {
        if self.search.is_empty() {
            return self.entries.iter().collect();
        }
        let query = self.search.to_lowercase();
        self.entries
            .iter()
            .filter(|e| {
                e.action.to_lowercase().contains(&query)
                    || e.key.to_lowercase().contains(&query)
            })
            .collect()
    }

    pub fn categories_for_entries(entries: &[&KeybindingEntry]) -> Vec<&'static str> {
        let mut cats: Vec<&'static str> = Vec::new();
        for e in entries {
            if !cats.contains(&e.category) {
                cats.push(e.category);
            }
        }
        cats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_entries_have_non_empty_fields() {
        let view = KeybindingsView::new();
        for entry in &view.entries {
            assert!(!entry.category.is_empty(), "empty category");
            assert!(!entry.action.is_empty(), "empty action");
            assert!(!entry.key.is_empty(), "empty key");
        }
    }

    #[test]
    fn all_entries_have_valid_categories() {
        let valid = ["Navigation", "Editing", "Modes", "App"];
        let view = KeybindingsView::new();
        for entry in &view.entries {
            assert!(
                valid.contains(&entry.category),
                "invalid category: {}",
                entry.category
            );
        }
    }

    #[test]
    fn empty_search_returns_all() {
        let view = KeybindingsView::new();
        let total = view.entries.len();
        let filtered = view.filtered_entries();
        assert_eq!(filtered.len(), total);
    }

    #[test]
    fn search_matches_action_case_insensitive() {
        let mut view = KeybindingsView::new();
        view.search = "Delete".to_string();
        let filtered = view.filtered_entries();
        assert!(filtered.len() >= 2); // at least "Delete char" and "Delete line"
        for entry in &filtered {
            let action_lower = entry.action.to_lowercase();
            let key_lower = entry.key.to_lowercase();
            assert!(
                action_lower.contains("delete") || key_lower.contains("delete"),
                "entry '{}' / '{}' doesn't match 'delete'",
                entry.action,
                entry.key
            );
        }
    }

    #[test]
    fn search_matches_key() {
        let mut view = KeybindingsView::new();
        view.search = "dd".to_string();
        let filtered = view.filtered_entries();
        assert!(filtered.len() >= 1);
        assert!(filtered.iter().any(|e| e.key == "dd"));
    }

    #[test]
    fn search_no_matches_returns_empty() {
        let mut view = KeybindingsView::new();
        view.search = "xyznonexistent".to_string();
        let filtered = view.filtered_entries();
        assert!(filtered.is_empty());
    }

    #[test]
    fn categories_preserves_order_and_deduplicates() {
        let view = KeybindingsView::new();
        let entries = view.filtered_entries();
        let cats = KeybindingsView::categories_for_entries(&entries);
        assert_eq!(cats[0], "Navigation");
        assert_eq!(cats[1], "Editing");
        assert_eq!(cats[2], "Modes");
        assert_eq!(cats[3], "App");
        // No duplicates
        let mut seen = std::collections::HashSet::new();
        for cat in &cats {
            assert!(seen.insert(cat), "duplicate category: {}", cat);
        }
    }

    #[test]
    fn search_hides_empty_categories() {
        let mut view = KeybindingsView::new();
        // "settings" should only match the App category
        view.search = "settings".to_string();
        let filtered = view.filtered_entries();
        let cats = KeybindingsView::categories_for_entries(&filtered);
        assert_eq!(cats.len(), 1);
        assert_eq!(cats[0], "App");
    }
}
