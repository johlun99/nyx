// src/vim/command.rs

#[derive(Debug, Clone, PartialEq)]
pub enum CommandResult {
    Write,
    Quit,
    WriteQuit,
    ForceQuit,
    Unknown(String),
}

pub struct CommandParser {
    pub input: String,
}

impl CommandParser {
    pub fn new() -> Self {
        Self {
            input: String::new(),
        }
    }

    pub fn execute(&self) -> CommandResult {
        match self.input.trim() {
            "w" => CommandResult::Write,
            "q" => CommandResult::Quit,
            "wq" | "x" => CommandResult::WriteQuit,
            "q!" => CommandResult::ForceQuit,
            other => CommandResult::Unknown(other.to_string()),
        }
    }

    pub fn clear(&mut self) {
        self.input.clear();
    }

    pub fn push_char(&mut self, ch: char) {
        self.input.push(ch);
    }

    pub fn pop_char(&mut self) {
        self.input.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_write() {
        let mut parser = CommandParser::new();
        parser.input = "w".into();
        assert_eq!(parser.execute(), CommandResult::Write);
    }

    #[test]
    fn parse_quit() {
        let mut parser = CommandParser::new();
        parser.input = "q".into();
        assert_eq!(parser.execute(), CommandResult::Quit);
    }

    #[test]
    fn parse_write_quit() {
        let mut parser = CommandParser::new();
        parser.input = "wq".into();
        assert_eq!(parser.execute(), CommandResult::WriteQuit);
    }

    #[test]
    fn parse_force_quit() {
        let mut parser = CommandParser::new();
        parser.input = "q!".into();
        assert_eq!(parser.execute(), CommandResult::ForceQuit);
    }

    #[test]
    fn parse_unknown() {
        let mut parser = CommandParser::new();
        parser.input = "foo".into();
        assert_eq!(parser.execute(), CommandResult::Unknown("foo".into()));
    }
}
