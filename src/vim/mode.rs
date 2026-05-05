#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Command,
    Visual,
    VisualLine,
    VisualBlock,
}

impl Mode {
    pub fn status_text(&self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Command => "COMMAND",
            Mode::Visual => "VISUAL",
            Mode::VisualLine => "V-LINE",
            Mode::VisualBlock => "V-BLOCK",
        }
    }

    pub fn is_visual(&self) -> bool {
        matches!(self, Mode::Visual | Mode::VisualLine | Mode::VisualBlock)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_status_text() {
        assert_eq!(Mode::Normal.status_text(), "NORMAL");
        assert_eq!(Mode::Insert.status_text(), "INSERT");
        assert_eq!(Mode::Command.status_text(), "COMMAND");
        assert_eq!(Mode::Visual.status_text(), "VISUAL");
        assert_eq!(Mode::VisualLine.status_text(), "V-LINE");
        assert_eq!(Mode::VisualBlock.status_text(), "V-BLOCK");
    }

    #[test]
    fn is_visual() {
        assert!(!Mode::Normal.is_visual());
        assert!(!Mode::Insert.is_visual());
        assert!(Mode::Visual.is_visual());
        assert!(Mode::VisualLine.is_visual());
        assert!(Mode::VisualBlock.is_visual());
    }
}
