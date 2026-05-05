// src/buffer/history.rs

#[derive(Clone, Debug)]
pub(crate) enum EditAction {
    Insert { offset: usize, text: String },
    Delete { offset: usize, text: String },
}

/// An undo entry is either a single edit or a group of edits (e.g., an entire Insert session).
/// `undo()` always pops one UndoEntry, so a group is undone atomically.
#[derive(Clone, Debug)]
pub(crate) enum UndoEntry {
    Single(EditAction),
    Group(Vec<EditAction>),
}

pub(crate) struct History {
    undo_stack: std::collections::VecDeque<UndoEntry>,
    redo_stack: Vec<UndoEntry>,
    /// Accumulates actions while an undo group is open (e.g., during Insert mode).
    /// When the group is closed, this becomes a single UndoEntry::Group.
    current_group: Option<Vec<EditAction>>,
    max_entries: usize,
    recording: bool,
}

impl History {
    pub fn new() -> Self {
        Self::with_max_entries(10_000)
    }

    pub fn with_max_entries(max: usize) -> Self {
        Self {
            undo_stack: std::collections::VecDeque::new(),
            redo_stack: Vec::new(),
            current_group: None,
            max_entries: max,
            recording: true,
        }
    }

    pub fn set_recording(&mut self, recording: bool) {
        self.recording = recording;
    }

    /// Begin an undo group. All subsequent push() calls accumulate into this group
    /// until end_group() is called. Used when entering Insert mode.
    pub fn begin_group(&mut self) {
        debug_assert!(
            self.current_group.is_none(),
            "begin_group called while a group is already open"
        );
        if self.current_group.is_none() {
            self.current_group = Some(Vec::new());
        }
    }

    /// Close the current undo group and push it as a single UndoEntry.
    /// Used when leaving Insert mode (Escape).
    pub fn end_group(&mut self) {
        if let Some(actions) = self.current_group.take() {
            if !actions.is_empty() {
                self.push_entry(UndoEntry::Group(actions));
            }
        }
    }

    pub fn push(&mut self, action: EditAction) {
        if !self.recording {
            return;
        }
        if let Some(ref mut group) = self.current_group {
            group.push(action);
            self.redo_stack.clear();
        } else {
            self.push_entry(UndoEntry::Single(action));
        }
    }

    fn push_entry(&mut self, entry: UndoEntry) {
        self.undo_stack.push_back(entry);
        self.redo_stack.clear();
        if self.undo_stack.len() > self.max_entries {
            self.undo_stack.pop_front(); // O(1) eviction
        }
    }

    pub fn undo(&mut self) -> Option<UndoEntry> {
        let entry = self.undo_stack.pop_back()?;
        self.redo_stack.push(entry.clone());
        Some(entry)
    }

    pub fn redo(&mut self) -> Option<UndoEntry> {
        let entry = self.redo_stack.pop()?;
        self.undo_stack.push_back(entry.clone());
        Some(entry)
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_undo() {
        let mut hist = History::new();
        hist.push(EditAction::Insert {
            offset: 0,
            text: "hello".into(),
        });
        let entry = hist.undo();
        assert!(entry.is_some());
        match entry.unwrap() {
            UndoEntry::Single(EditAction::Insert { offset, text }) => {
                assert_eq!(offset, 0);
                assert_eq!(text, "hello");
            }
            _ => panic!("expected Single(Insert)"),
        }
    }

    #[test]
    fn undo_returns_none_when_empty() {
        let mut hist = History::new();
        assert!(hist.undo().is_none());
    }

    #[test]
    fn redo_after_undo() {
        let mut hist = History::new();
        hist.push(EditAction::Insert {
            offset: 0,
            text: "hello".into(),
        });
        let _ = hist.undo();
        let entry = hist.redo();
        assert!(entry.is_some());
    }

    #[test]
    fn new_edit_clears_redo_stack() {
        let mut hist = History::new();
        hist.push(EditAction::Insert {
            offset: 0,
            text: "a".into(),
        });
        let _ = hist.undo();
        hist.push(EditAction::Insert {
            offset: 0,
            text: "b".into(),
        });
        assert!(hist.redo().is_none());
    }

    #[test]
    fn respects_max_entries() {
        let mut hist = History::with_max_entries(3);
        for i in 0..5 {
            hist.push(EditAction::Insert {
                offset: i,
                text: "x".into(),
            });
        }
        // Only 3 most recent should survive
        assert!(hist.undo().is_some());
        assert!(hist.undo().is_some());
        assert!(hist.undo().is_some());
        assert!(hist.undo().is_none());
    }

    #[test]
    fn pause_and_resume_recording() {
        let mut hist = History::new();
        hist.set_recording(false);
        hist.push(EditAction::Insert {
            offset: 0,
            text: "ignored".into(),
        });
        assert!(hist.undo().is_none());
        hist.set_recording(true);
        hist.push(EditAction::Insert {
            offset: 0,
            text: "recorded".into(),
        });
        assert!(hist.undo().is_some());
    }

    #[test]
    fn group_undo_is_atomic() {
        let mut hist = History::new();
        hist.begin_group();
        hist.push(EditAction::Insert {
            offset: 0,
            text: "a".into(),
        });
        hist.push(EditAction::Insert {
            offset: 1,
            text: "b".into(),
        });
        hist.push(EditAction::Insert {
            offset: 2,
            text: "c".into(),
        });
        hist.end_group();

        // Single undo should remove the entire group
        let entry = hist.undo();
        assert!(entry.is_some());
        match entry.unwrap() {
            UndoEntry::Group(actions) => assert_eq!(actions.len(), 3),
            _ => panic!("expected Group"),
        }
        // No more entries
        assert!(hist.undo().is_none());
    }

    #[test]
    fn empty_group_produces_no_entry() {
        let mut hist = History::new();
        hist.begin_group();
        hist.end_group();
        assert!(hist.undo().is_none());
    }
}
