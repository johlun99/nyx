#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Command,
}

impl Mode {
    pub fn status_text(&self) -> &'static str {
        match self {
            Mode::Normal => "NORMAL",
            Mode::Insert => "INSERT",
            Mode::Command => "COMMAND",
        }
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
    }
}
