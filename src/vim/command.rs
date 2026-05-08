// src/vim/command.rs

#[derive(Debug, Clone, PartialEq)]
pub enum CommandResult {
    Write,
    Quit,
    WriteQuit,
    ForceQuit,
    Rename(String),
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
        let trimmed = self.input.trim();
        if let Some(name) = trimmed.strip_prefix("rename ") {
            let new_name = name.trim().to_string();
            if new_name.is_empty() {
                return CommandResult::Unknown(trimmed.to_string());
            }
            return CommandResult::Rename(new_name);
        }
        match trimmed {
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

    #[test]
    fn parse_rename() {
        let mut parser = CommandParser::new();
        parser.input = "rename new_name".into();
        assert_eq!(parser.execute(), CommandResult::Rename("new_name".into()));
    }

    #[test]
    fn parse_rename_empty_name() {
        let mut parser = CommandParser::new();
        parser.input = "rename ".into();
        assert_eq!(parser.execute(), CommandResult::Unknown("rename".into()));
    }
}
