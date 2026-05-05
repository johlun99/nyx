use crate::vim::action::SearchDirection;
use crate::vim::action::*;
use crate::vim::mode::Mode;
use crate::vim::text_object::{TextObject, TextObjectKind};

pub struct KeyParser {
    mode: Mode,
    pending: String,
    count: Option<usize>,
    pending_register: Option<char>,
    awaiting_register: bool,
}

// Note: Visual, Visual-Line, and Visual-Block modes are planned for Phase 2.
// Dot-repeat (`.`) is planned for Phase 2 — will require storing last action in Editor.

impl KeyParser {
    pub fn new() -> Self {
        Self {
            mode: Mode::Normal,
            pending: String::new(),
            count: None,
            pending_register: None,
            awaiting_register: false,
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Returns the accumulated count, consuming it. Returns 1 if no count was set.
    pub fn take_count(&mut self) -> usize {
        self.count.take().unwrap_or(1)
    }

    /// Returns the pending register selection, consuming it.
    pub fn take_register(&mut self) -> Option<char> {
        self.pending_register.take()
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    pub fn handle_key(&mut self, ch: char) -> VimAction {
        match self.mode {
            Mode::Normal => self.handle_normal(ch),
            Mode::Insert => self.handle_insert(ch),
            Mode::Command => VimAction::Noop, // command input handled separately
            Mode::Visual | Mode::VisualLine | Mode::VisualBlock => self.handle_visual(ch),
        }
    }

    pub fn handle_escape(&mut self) -> VimAction {
        self.pending.clear();
        self.count = None;
        self.pending_register = None;
        self.awaiting_register = false;
        self.mode = Mode::Normal;
        VimAction::SwitchMode(Mode::Normal)
    }

    pub fn handle_backspace(&mut self) -> VimAction {
        match self.mode {
            Mode::Insert => VimAction::DeleteCharBefore,
            _ => VimAction::Noop,
        }
    }

    pub fn handle_ctrl_r(&mut self) -> VimAction {
        match self.mode {
            Mode::Normal => VimAction::Redo,
            _ => VimAction::Noop,
        }
    }

    fn handle_normal(&mut self, ch: char) -> VimAction {
        // Register prefix: " followed by a-z or +
        if self.awaiting_register {
            self.awaiting_register = false;
            match ch {
                'a'..='z' | '+' => {
                    self.pending_register = Some(ch);
                    return VimAction::Noop;
                }
                _ => {
                    return VimAction::Noop;
                }
            }
        }

        // Count prefix: digits 1-9 start a count, 0 after digits continues count.
        // Uses saturating arithmetic and caps at 99999 to prevent overflow.
        if ch.is_ascii_digit() && (ch != '0' || self.count.is_some()) {
            let digit = ch.to_digit(10).unwrap() as usize;
            let current = self.count.unwrap_or(0);
            self.count = Some(current.saturating_mul(10).saturating_add(digit).min(99_999));
            return VimAction::Noop;
        }

        // Pending multi-char sequences
        if !self.pending.is_empty() {
            let combined = format!("{}{}", self.pending, ch);

            // 3-char text object sequences: operator + i/a + object kind
            // Intercept "di", "da", "ci", "ca", "yi", "ya" to wait for one more char.
            if combined.len() == 2 {
                let first = combined.chars().next().unwrap();
                if (first == 'd' || first == 'c' || first == 'y') && (ch == 'i' || ch == 'a') {
                    self.pending = combined;
                    return VimAction::Noop;
                }
            }

            if combined.len() == 3 {
                // Text object: e.g. "diw", "ci\"", "ya("
                let mut chars_iter = combined.chars();
                let op = chars_iter.next().unwrap();
                let scope = chars_iter.next().unwrap();
                let kind_ch = chars_iter.next().unwrap();

                self.pending.clear();

                if let Some(kind) = Self::char_to_text_object_kind(kind_ch) {
                    let text_obj = match scope {
                        'i' => TextObject::Inner(kind),
                        'a' => TextObject::Around(kind),
                        _ => return VimAction::Noop,
                    };
                    return match op {
                        'd' => VimAction::Operator(OperatorAction::DeleteTextObject(text_obj)),
                        'c' => {
                            self.mode = Mode::Insert;
                            VimAction::Operator(OperatorAction::ChangeTextObject(text_obj))
                        }
                        'y' => VimAction::Operator(OperatorAction::YankTextObject(text_obj)),
                        _ => VimAction::Noop,
                    };
                }
                return VimAction::Noop;
            }

            self.pending.clear();
            return match combined.as_str() {
                "gg" => VimAction::Motion(MotionKind::FileTop),
                "dd" => VimAction::Operator(OperatorAction::DeleteLine),
                "cc" => {
                    self.mode = Mode::Insert;
                    VimAction::Operator(OperatorAction::ChangeLine)
                }
                "yy" => VimAction::Operator(OperatorAction::YankLine),
                s if s.starts_with('d') => {
                    if let Some(motion) = Self::char_to_motion(ch) {
                        VimAction::Operator(OperatorAction::Delete(motion))
                    } else {
                        VimAction::Noop
                    }
                }
                s if s.starts_with('c') => {
                    if let Some(motion) = Self::char_to_motion(ch) {
                        self.mode = Mode::Insert;
                        VimAction::Operator(OperatorAction::Change(motion))
                    } else {
                        VimAction::Noop
                    }
                }
                s if s.starts_with('y') => {
                    if let Some(motion) = Self::char_to_motion(ch) {
                        VimAction::Yank(motion)
                    } else {
                        VimAction::Noop
                    }
                }
                _ => VimAction::Noop,
            };
        }

        match ch {
            // Motions
            'h' => VimAction::Motion(MotionKind::Left),
            'j' => VimAction::Motion(MotionKind::Down),
            'k' => VimAction::Motion(MotionKind::Up),
            'l' => VimAction::Motion(MotionKind::Right),
            '0' => VimAction::Motion(MotionKind::LineStart),
            '^' => VimAction::Motion(MotionKind::FirstNonBlank),
            '$' => VimAction::Motion(MotionKind::LineEnd),
            'w' => VimAction::Motion(MotionKind::WordForward),
            'b' => VimAction::Motion(MotionKind::WordBackward),
            'e' => VimAction::Motion(MotionKind::WordEnd),
            'G' => VimAction::Motion(MotionKind::FileBottom),

            // Pending sequences
            'g' | 'd' | 'c' | 'y' => {
                self.pending.push(ch);
                VimAction::Noop
            }

            // Operators
            'p' => VimAction::Paste,
            'x' => VimAction::Operator(OperatorAction::Delete(MotionKind::Right)),
            'u' => VimAction::Undo,

            // Insert entry points
            'i' => {
                self.mode = Mode::Insert;
                VimAction::EnterInsert(InsertEntry::AtCursor)
            }
            'a' => {
                self.mode = Mode::Insert;
                VimAction::EnterInsert(InsertEntry::AfterCursor)
            }
            'A' => {
                self.mode = Mode::Insert;
                VimAction::EnterInsert(InsertEntry::EndOfLine)
            }
            'I' => {
                self.mode = Mode::Insert;
                VimAction::EnterInsert(InsertEntry::FirstNonBlank)
            }
            'o' => {
                self.mode = Mode::Insert;
                VimAction::EnterInsert(InsertEntry::NewLineBelow)
            }
            'O' => {
                self.mode = Mode::Insert;
                VimAction::EnterInsert(InsertEntry::NewLineAbove)
            }

            // Visual mode entry
            'v' => {
                self.mode = Mode::Visual;
                VimAction::EnterVisual(VisualKind::Char)
            }
            'V' => {
                self.mode = Mode::VisualLine;
                VimAction::EnterVisual(VisualKind::Line)
            }

            // Command mode
            ':' => {
                self.mode = Mode::Command;
                VimAction::SwitchMode(Mode::Command)
            }

            // Register prefix
            '"' => {
                self.awaiting_register = true;
                VimAction::Noop
            }

            // Dot-repeat
            '.' => VimAction::DotRepeat,

            // Search
            '/' => VimAction::EnterSearch(SearchDirection::Forward),
            '?' => VimAction::EnterSearch(SearchDirection::Backward),
            'n' => VimAction::SearchNext,
            'N' => VimAction::SearchPrev,

            _ => VimAction::Noop,
        }
    }

    fn handle_visual(&mut self, ch: char) -> VimAction {
        // Handle pending sequences (e.g. gg)
        if !self.pending.is_empty() {
            let combined = format!("{}{}", self.pending, ch);
            self.pending.clear();
            return match combined.as_str() {
                "gg" => VimAction::Motion(MotionKind::FileTop),
                _ => VimAction::Noop,
            };
        }

        match ch {
            // Motions work in visual mode (move cursor, selection follows)
            'h' => VimAction::Motion(MotionKind::Left),
            'j' => VimAction::Motion(MotionKind::Down),
            'k' => VimAction::Motion(MotionKind::Up),
            'l' => VimAction::Motion(MotionKind::Right),
            '0' => VimAction::Motion(MotionKind::LineStart),
            '^' => VimAction::Motion(MotionKind::FirstNonBlank),
            '$' => VimAction::Motion(MotionKind::LineEnd),
            'w' => VimAction::Motion(MotionKind::WordForward),
            'b' => VimAction::Motion(MotionKind::WordBackward),
            'e' => VimAction::Motion(MotionKind::WordEnd),
            'G' => VimAction::Motion(MotionKind::FileBottom),
            'g' => {
                self.pending.push(ch);
                VimAction::Noop
            }

            // Operators on selection
            'd' => {
                self.mode = Mode::Normal;
                VimAction::VisualOperator(VisualOperatorAction::Delete)
            }
            'c' => {
                self.mode = Mode::Insert;
                VimAction::VisualOperator(VisualOperatorAction::Change)
            }
            'y' => {
                self.mode = Mode::Normal;
                VimAction::VisualOperator(VisualOperatorAction::Yank)
            }
            'x' => {
                self.mode = Mode::Normal;
                VimAction::VisualOperator(VisualOperatorAction::Delete)
            }

            // Swap anchor/cursor
            'o' => VimAction::SwapVisualAnchor,

            // Switch between visual sub-modes
            'v' => {
                if self.mode == Mode::Visual {
                    self.mode = Mode::Normal;
                    VimAction::SwitchMode(Mode::Normal)
                } else {
                    self.mode = Mode::Visual;
                    VimAction::SwitchMode(Mode::Visual)
                }
            }
            'V' => {
                if self.mode == Mode::VisualLine {
                    self.mode = Mode::Normal;
                    VimAction::SwitchMode(Mode::Normal)
                } else {
                    self.mode = Mode::VisualLine;
                    VimAction::SwitchMode(Mode::VisualLine)
                }
            }

            _ => VimAction::Noop,
        }
    }

    fn handle_insert(&mut self, ch: char) -> VimAction {
        VimAction::InsertChar(ch)
    }

    fn char_to_motion(ch: char) -> Option<MotionKind> {
        match ch {
            'h' => Some(MotionKind::Left),
            'j' => Some(MotionKind::Down),
            'k' => Some(MotionKind::Up),
            'l' => Some(MotionKind::Right),
            'w' => Some(MotionKind::WordForward),
            'b' => Some(MotionKind::WordBackward),
            'e' => Some(MotionKind::WordEnd),
            '0' => Some(MotionKind::LineStart),
            '^' => Some(MotionKind::FirstNonBlank),
            '$' => Some(MotionKind::LineEnd),
            'G' => Some(MotionKind::FileBottom),
            _ => None,
        }
    }

    fn char_to_text_object_kind(ch: char) -> Option<TextObjectKind> {
        match ch {
            'w' => Some(TextObjectKind::Word),
            'W' => Some(TextObjectKind::BigWord),
            '"' => Some(TextObjectKind::DoubleQuote),
            '\'' => Some(TextObjectKind::SingleQuote),
            '(' | ')' => Some(TextObjectKind::Paren),
            '[' | ']' => Some(TextObjectKind::Bracket),
            '{' | '}' => Some(TextObjectKind::Brace),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vim::action::SearchDirection;
    use crate::vim::text_object::{TextObject, TextObjectKind};

    #[test]
    fn normal_mode_i_enters_insert() {
        let mut parser = KeyParser::new();
        let action = parser.handle_key('i');
        assert_eq!(action, VimAction::EnterInsert(InsertEntry::AtCursor));
        assert_eq!(parser.mode(), Mode::Insert);
    }

    #[test]
    fn normal_mode_a_enters_insert_after() {
        let mut parser = KeyParser::new();
        let action = parser.handle_key('a');
        assert_eq!(action, VimAction::EnterInsert(InsertEntry::AfterCursor));
        assert_eq!(parser.mode(), Mode::Insert);
    }

    #[test]
    fn normal_mode_shift_a_enters_insert_eol() {
        let mut parser = KeyParser::new();
        let action = parser.handle_key('A');
        assert_eq!(action, VimAction::EnterInsert(InsertEntry::EndOfLine));
        assert_eq!(parser.mode(), Mode::Insert);
    }

    #[test]
    fn normal_mode_shift_i_enters_insert_first_nonblank() {
        let mut parser = KeyParser::new();
        let action = parser.handle_key('I');
        assert_eq!(action, VimAction::EnterInsert(InsertEntry::FirstNonBlank));
        assert_eq!(parser.mode(), Mode::Insert);
    }

    #[test]
    fn normal_mode_o_opens_line_below() {
        let mut parser = KeyParser::new();
        let action = parser.handle_key('o');
        assert_eq!(action, VimAction::EnterInsert(InsertEntry::NewLineBelow));
        assert_eq!(parser.mode(), Mode::Insert);
    }

    #[test]
    fn normal_mode_shift_o_opens_line_above() {
        let mut parser = KeyParser::new();
        let action = parser.handle_key('O');
        assert_eq!(action, VimAction::EnterInsert(InsertEntry::NewLineAbove));
        assert_eq!(parser.mode(), Mode::Insert);
    }

    #[test]
    fn escape_returns_to_normal() {
        let mut parser = KeyParser::new();
        parser.set_mode(Mode::Insert);
        let action = parser.handle_escape();
        assert_eq!(action, VimAction::SwitchMode(Mode::Normal));
    }

    #[test]
    fn normal_mode_hjkl_motions() {
        let mut parser = KeyParser::new();
        assert_eq!(parser.handle_key('h'), VimAction::Motion(MotionKind::Left));
        assert_eq!(parser.handle_key('j'), VimAction::Motion(MotionKind::Down));
        assert_eq!(parser.handle_key('k'), VimAction::Motion(MotionKind::Up));
        assert_eq!(parser.handle_key('l'), VimAction::Motion(MotionKind::Right));
    }

    #[test]
    fn normal_mode_word_motions() {
        let mut parser = KeyParser::new();
        assert_eq!(
            parser.handle_key('w'),
            VimAction::Motion(MotionKind::WordForward)
        );
        assert_eq!(
            parser.handle_key('b'),
            VimAction::Motion(MotionKind::WordBackward)
        );
        assert_eq!(
            parser.handle_key('e'),
            VimAction::Motion(MotionKind::WordEnd)
        );
    }

    #[test]
    fn normal_mode_line_motions() {
        let mut parser = KeyParser::new();
        assert_eq!(
            parser.handle_key('0'),
            VimAction::Motion(MotionKind::LineStart)
        );
        assert_eq!(
            parser.handle_key('$'),
            VimAction::Motion(MotionKind::LineEnd)
        );
        assert_eq!(
            parser.handle_key('^'),
            VimAction::Motion(MotionKind::FirstNonBlank)
        );
    }

    #[test]
    fn normal_mode_gg_and_big_g() {
        let mut parser = KeyParser::new();
        assert_eq!(parser.handle_key('g'), VimAction::Noop);
        assert_eq!(
            parser.handle_key('g'),
            VimAction::Motion(MotionKind::FileTop)
        );
        assert_eq!(
            parser.handle_key('G'),
            VimAction::Motion(MotionKind::FileBottom)
        );
    }

    #[test]
    fn normal_mode_undo_redo() {
        let mut parser = KeyParser::new();
        assert_eq!(parser.handle_key('u'), VimAction::Undo);
    }

    #[test]
    fn insert_mode_regular_char() {
        let mut parser = KeyParser::new();
        parser.set_mode(Mode::Insert);
        assert_eq!(parser.handle_key('a'), VimAction::InsertChar('a'));
    }

    #[test]
    fn colon_enters_command() {
        let mut parser = KeyParser::new();
        assert_eq!(parser.handle_key(':'), VimAction::SwitchMode(Mode::Command));
    }

    #[test]
    fn count_prefix_with_motion() {
        let mut parser = KeyParser::new();
        assert_eq!(parser.handle_key('5'), VimAction::Noop);
        let action = parser.handle_key('j');
        assert_eq!(action, VimAction::Motion(MotionKind::Down));
        // count should have been 5, consumed by handle_key
        // The count is read via take_count() after producing the action
    }

    #[test]
    fn count_prefix_parsing() {
        let mut parser = KeyParser::new();
        parser.handle_key('1');
        parser.handle_key('2');
        assert_eq!(parser.take_count(), 12);
    }

    #[test]
    fn no_count_returns_1() {
        let mut parser = KeyParser::new();
        assert_eq!(parser.take_count(), 1);
    }

    #[test]
    fn unrecognized_operator_target_resets_pending() {
        let mut parser = KeyParser::new();
        assert_eq!(parser.handle_key('d'), VimAction::Noop); // pending 'd'
        assert_eq!(parser.handle_key('z'), VimAction::Noop); // unrecognized, clears pending
                                                             // Parser should be ready for new input
        assert_eq!(parser.handle_key('j'), VimAction::Motion(MotionKind::Down));
    }

    #[test]
    fn count_prefix_consumed_with_action() {
        let mut parser = KeyParser::new();
        parser.handle_key('5');
        parser.handle_key('j');
        // After action dispatch, count should be available via take_count
        assert_eq!(parser.take_count(), 5);
        // Second take_count returns default 1
        assert_eq!(parser.take_count(), 1);
    }

    #[test]
    fn register_prefix_sets_register() {
        let mut parser = KeyParser::new();
        assert_eq!(parser.handle_key('"'), VimAction::Noop);
        assert_eq!(parser.handle_key('a'), VimAction::Noop);
        assert_eq!(parser.take_register(), Some('a'));
    }

    #[test]
    fn register_prefix_plus() {
        let mut parser = KeyParser::new();
        parser.handle_key('"');
        parser.handle_key('+');
        assert_eq!(parser.take_register(), Some('+'));
    }

    #[test]
    fn register_consumed_after_take() {
        let mut parser = KeyParser::new();
        parser.handle_key('"');
        parser.handle_key('a');
        assert_eq!(parser.take_register(), Some('a'));
        assert_eq!(parser.take_register(), None);
    }

    #[test]
    fn register_prefix_then_operator() {
        let mut parser = KeyParser::new();
        parser.handle_key('"');
        parser.handle_key('a');
        let action = parser.handle_key('p');
        assert_eq!(action, VimAction::Paste);
        assert_eq!(parser.take_register(), Some('a'));
    }

    #[test]
    fn text_object_diw() {
        let mut parser = KeyParser::new();
        assert_eq!(parser.handle_key('d'), VimAction::Noop);
        assert_eq!(parser.handle_key('i'), VimAction::Noop);
        assert_eq!(
            parser.handle_key('w'),
            VimAction::Operator(OperatorAction::DeleteTextObject(TextObject::Inner(
                TextObjectKind::Word
            )))
        );
    }

    #[test]
    fn text_object_ci_double_quote() {
        let mut parser = KeyParser::new();
        parser.handle_key('c');
        parser.handle_key('i');
        let action = parser.handle_key('"');
        assert_eq!(
            action,
            VimAction::Operator(OperatorAction::ChangeTextObject(TextObject::Inner(
                TextObjectKind::DoubleQuote
            )))
        );
        assert_eq!(parser.mode(), Mode::Insert);
    }

    #[test]
    fn text_object_ya_paren() {
        let mut parser = KeyParser::new();
        parser.handle_key('y');
        parser.handle_key('a');
        let action = parser.handle_key('(');
        assert_eq!(
            action,
            VimAction::Operator(OperatorAction::YankTextObject(TextObject::Around(
                TextObjectKind::Paren
            )))
        );
    }

    #[test]
    fn text_object_invalid_resets() {
        let mut parser = KeyParser::new();
        parser.handle_key('d');
        parser.handle_key('i');
        assert_eq!(parser.handle_key('z'), VimAction::Noop); // invalid object
                                                             // Parser should be clean
        assert_eq!(parser.handle_key('j'), VimAction::Motion(MotionKind::Down));
    }

    #[test]
    fn v_enters_visual_mode() {
        let mut parser = KeyParser::new();
        let action = parser.handle_key('v');
        assert_eq!(action, VimAction::EnterVisual(VisualKind::Char));
        assert_eq!(parser.mode(), Mode::Visual);
    }

    #[test]
    fn visual_v_enters_visual_line() {
        let mut parser = KeyParser::new();
        let action = parser.handle_key('V');
        assert_eq!(action, VimAction::EnterVisual(VisualKind::Line));
        assert_eq!(parser.mode(), Mode::VisualLine);
    }

    #[test]
    fn visual_mode_d_deletes_selection() {
        let mut parser = KeyParser::new();
        parser.handle_key('v');
        let action = parser.handle_key('d');
        assert_eq!(
            action,
            VimAction::VisualOperator(VisualOperatorAction::Delete)
        );
        assert_eq!(parser.mode(), Mode::Normal);
    }

    #[test]
    fn visual_mode_y_yanks_selection() {
        let mut parser = KeyParser::new();
        parser.handle_key('v');
        let action = parser.handle_key('y');
        assert_eq!(
            action,
            VimAction::VisualOperator(VisualOperatorAction::Yank)
        );
        assert_eq!(parser.mode(), Mode::Normal);
    }

    #[test]
    fn visual_mode_c_changes_selection() {
        let mut parser = KeyParser::new();
        parser.handle_key('v');
        let action = parser.handle_key('c');
        assert_eq!(
            action,
            VimAction::VisualOperator(VisualOperatorAction::Change)
        );
        assert_eq!(parser.mode(), Mode::Insert);
    }

    #[test]
    fn visual_mode_o_swaps_anchor() {
        let mut parser = KeyParser::new();
        parser.handle_key('v');
        let action = parser.handle_key('o');
        assert_eq!(action, VimAction::SwapVisualAnchor);
    }

    #[test]
    fn visual_mode_escape_returns_normal() {
        let mut parser = KeyParser::new();
        parser.handle_key('v');
        let action = parser.handle_escape();
        assert_eq!(action, VimAction::SwitchMode(Mode::Normal));
        assert_eq!(parser.mode(), Mode::Normal);
    }

    #[test]
    fn visual_mode_motions_work() {
        let mut parser = KeyParser::new();
        parser.handle_key('v');
        assert_eq!(parser.handle_key('j'), VimAction::Motion(MotionKind::Down));
        assert_eq!(
            parser.handle_key('w'),
            VimAction::Motion(MotionKind::WordForward)
        );
    }

    #[test]
    fn dot_emits_dot_repeat() {
        let mut parser = KeyParser::new();
        assert_eq!(parser.handle_key('.'), VimAction::DotRepeat);
    }

    #[test]
    fn forward_search_key() {
        let mut parser = KeyParser::new();
        assert_eq!(
            parser.handle_key('/'),
            VimAction::EnterSearch(SearchDirection::Forward)
        );
    }

    #[test]
    fn backward_search_key() {
        let mut parser = KeyParser::new();
        assert_eq!(
            parser.handle_key('?'),
            VimAction::EnterSearch(SearchDirection::Backward)
        );
    }

    #[test]
    fn search_next_key() {
        let mut parser = KeyParser::new();
        assert_eq!(parser.handle_key('n'), VimAction::SearchNext);
    }

    #[test]
    fn search_prev_key() {
        let mut parser = KeyParser::new();
        assert_eq!(parser.handle_key('N'), VimAction::SearchPrev);
    }
}
