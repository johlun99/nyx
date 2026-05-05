use crate::vim::mode::Mode;
use crate::vim::text_object::TextObject;

#[derive(Debug, Clone, PartialEq)]
pub enum VimAction {
    SwitchMode(Mode),
    Motion(MotionKind),
    InsertChar(char),
    DeleteCharBefore,
    Operator(OperatorAction),
    Yank(MotionKind),
    Paste,
    Undo,
    Redo,
    /// Enter insert mode, optionally with a preparatory motion/action
    EnterInsert(InsertEntry),
    EnterVisual(VisualKind),
    VisualOperator(VisualOperatorAction),
    SwapVisualAnchor,
    Noop,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VisualKind {
    Char,
    Line,
    Block,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VisualOperatorAction {
    Delete,
    Change,
    Yank,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InsertEntry {
    AtCursor,      // i
    AfterCursor,   // a
    EndOfLine,     // A
    FirstNonBlank, // I
    NewLineBelow,  // o
    NewLineAbove,  // O
}

#[derive(Debug, Clone, PartialEq)]
pub enum MotionKind {
    Left,
    Down,
    Up,
    Right,
    LineStart,
    FirstNonBlank,
    LineEnd,
    WordForward,
    WordBackward,
    WordEnd,
    FileTop,
    FileBottom,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OperatorAction {
    Delete(MotionKind),
    Change(MotionKind),
    DeleteLine,
    ChangeLine,
    YankLine,
    DeleteTextObject(TextObject),
    ChangeTextObject(TextObject),
    YankTextObject(TextObject),
}
