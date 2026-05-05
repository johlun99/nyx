use crate::vim::action::SearchDirection;

pub struct SearchState {
    pub pattern: String,
    pub direction: SearchDirection,
    pub matches: Vec<(usize, usize)>, // (start_offset, end_offset) in char indices
    pub current_match: Option<usize>, // index into matches vec
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            pattern: String::new(),
            direction: SearchDirection::Forward,
            matches: Vec::new(),
            current_match: None,
        }
    }

    /// Find all literal substring matches in the given text.
    /// Returns (start, end) pairs as char offsets.
    pub fn find_matches(&mut self, text: &str) {
        self.matches.clear();
        self.current_match = None;

        if self.pattern.is_empty() {
            return;
        }

        let pattern_chars: Vec<char> = self.pattern.chars().collect();
        let text_chars: Vec<char> = text.chars().collect();
        let pat_len = pattern_chars.len();

        if pat_len > text_chars.len() {
            return;
        }

        for i in 0..=(text_chars.len() - pat_len) {
            if text_chars[i..i + pat_len] == pattern_chars[..] {
                self.matches.push((i, i + pat_len));
            }
        }
    }

    /// Jump to the next match from the given cursor offset.
    pub fn next_match(&mut self, cursor_offset: usize) -> Option<usize> {
        if self.matches.is_empty() {
            return None;
        }
        // Find first match starting after cursor_offset
        for (idx, &(start, _)) in self.matches.iter().enumerate() {
            if start > cursor_offset {
                self.current_match = Some(idx);
                return Some(start);
            }
        }
        // Wrap to first match
        self.current_match = Some(0);
        Some(self.matches[0].0)
    }

    /// Jump to the previous match from the given cursor offset.
    pub fn prev_match(&mut self, cursor_offset: usize) -> Option<usize> {
        if self.matches.is_empty() {
            return None;
        }
        // Find last match starting before cursor_offset
        for (idx, &(start, _)) in self.matches.iter().enumerate().rev() {
            if start < cursor_offset {
                self.current_match = Some(idx);
                return Some(start);
            }
        }
        // Wrap to last match
        let last = self.matches.len() - 1;
        self.current_match = Some(last);
        Some(self.matches[last].0)
    }

    /// Jump to the nearest match in the current search direction.
    pub fn jump_to_nearest(&mut self, cursor_offset: usize) -> Option<usize> {
        match self.direction {
            SearchDirection::Forward => self.next_match(cursor_offset),
            SearchDirection::Backward => self.prev_match(cursor_offset),
        }
    }

    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    pub fn current_match_index(&self) -> Option<usize> {
        self.current_match
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_matches_simple() {
        let mut state = SearchState::new();
        state.pattern = "hello".into();
        state.find_matches("say hello world hello end");
        assert_eq!(state.matches.len(), 2);
        assert_eq!(state.matches[0], (4, 9));
        assert_eq!(state.matches[1], (16, 21));
    }

    #[test]
    fn find_matches_empty_pattern() {
        let mut state = SearchState::new();
        state.pattern = String::new();
        state.find_matches("hello");
        assert!(state.matches.is_empty());
    }

    #[test]
    fn find_matches_no_match() {
        let mut state = SearchState::new();
        state.pattern = "xyz".into();
        state.find_matches("hello world");
        assert!(state.matches.is_empty());
    }

    #[test]
    fn find_matches_unicode() {
        let mut state = SearchState::new();
        state.pattern = "på".into();
        state.find_matches("hej på dig");
        assert_eq!(state.matches.len(), 1);
        assert_eq!(state.matches[0], (4, 6)); // char indices
    }

    #[test]
    fn next_match_forward() {
        let mut state = SearchState::new();
        state.pattern = "x".into();
        state.direction = SearchDirection::Forward;
        state.find_matches("axbxcx");
        // cursor at 0, next should find index 1 ('x' at pos 1)
        let offset = state.next_match(0);
        assert_eq!(offset, Some(1));
        assert_eq!(state.current_match, Some(0));
    }

    #[test]
    fn next_match_wraps() {
        let mut state = SearchState::new();
        state.pattern = "a".into();
        state.find_matches("abc");
        // cursor past the only match, should wrap
        let offset = state.next_match(2);
        assert_eq!(offset, Some(0));
    }

    #[test]
    fn prev_match_backward() {
        let mut state = SearchState::new();
        state.pattern = "x".into();
        state.find_matches("axbxcx");
        // cursor at 5, prev should find the match at pos 3
        let offset = state.prev_match(5);
        assert_eq!(offset, Some(3));
    }

    #[test]
    fn prev_match_wraps() {
        let mut state = SearchState::new();
        state.pattern = "x".into();
        state.find_matches("axbx");
        // cursor at 0, prev should wrap to last match
        let offset = state.prev_match(0);
        assert_eq!(offset, Some(3));
    }
}
