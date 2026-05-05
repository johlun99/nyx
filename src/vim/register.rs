use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct RegisterEntry {
    pub content: String,
    pub linewise: bool,
}

pub struct RegisterFile {
    unnamed: RegisterEntry,
    named: HashMap<char, RegisterEntry>,
    system_clipboard: Option<arboard::Clipboard>,
}

impl RegisterFile {
    pub fn new() -> Self {
        let system_clipboard = arboard::Clipboard::new().ok();
        Self {
            unnamed: RegisterEntry::default(),
            named: HashMap::new(),
            system_clipboard,
        }
    }

    pub fn get(&self, name: Option<char>) -> RegisterEntry {
        match name {
            None => self.unnamed.clone(),
            Some('+') => {
                // System clipboard needs &mut self (arboard requirement)
                // Use get_mut() for system clipboard reads
                self.unnamed.clone()
            }
            Some(c @ 'a'..='z') => self.named.get(&c).cloned().unwrap_or_default(),
            Some(_) => RegisterEntry::default(),
        }
    }

    /// Get from system clipboard. Needs &mut self because arboard requires it.
    #[allow(dead_code)]
    pub fn get_mut(&mut self, name: Option<char>) -> RegisterEntry {
        match name {
            Some('+') => {
                if let Some(ref mut clip) = self.system_clipboard {
                    if let Ok(text) = clip.get_text() {
                        return RegisterEntry {
                            content: text,
                            linewise: false,
                        };
                    }
                }
                self.unnamed.clone()
            }
            other => self.get(other),
        }
    }

    pub fn set(&mut self, name: Option<char>, content: String, linewise: bool) {
        let entry = RegisterEntry { content, linewise };
        match name {
            None => {
                self.unnamed = entry;
            }
            Some('+') => {
                if let Some(ref mut clip) = self.system_clipboard {
                    let _ = clip.set_text(entry.content.clone());
                }
                self.unnamed = entry;
            }
            Some(c @ 'a'..='z') => {
                self.unnamed = entry.clone();
                self.named.insert(c, entry);
            }
            Some(_) => {}
        }
    }
}

impl Default for RegisterFile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_unnamed_default_is_empty() {
        let reg = RegisterFile::new();
        let entry = reg.get(None);
        assert_eq!(entry.content, "");
        assert!(!entry.linewise);
    }

    #[test]
    fn set_and_get_unnamed() {
        let mut reg = RegisterFile::new();
        reg.set(None, "hello".into(), false);
        let entry = reg.get(None);
        assert_eq!(entry.content, "hello");
        assert!(!entry.linewise);
    }

    #[test]
    fn set_and_get_named() {
        let mut reg = RegisterFile::new();
        reg.set(Some('a'), "test".into(), true);
        let entry = reg.get(Some('a'));
        assert_eq!(entry.content, "test");
        assert!(entry.linewise);
    }

    #[test]
    fn named_register_also_sets_unnamed() {
        let mut reg = RegisterFile::new();
        reg.set(Some('b'), "line\n".into(), true);
        let unnamed = reg.get(None);
        assert_eq!(unnamed.content, "line\n");
        assert!(unnamed.linewise);
    }

    #[test]
    fn get_unset_named_returns_empty() {
        let reg = RegisterFile::new();
        let entry = reg.get(Some('z'));
        assert_eq!(entry.content, "");
    }

    #[test]
    fn linewise_flag_preserved() {
        let mut reg = RegisterFile::new();
        reg.set(None, "hello\n".into(), true);
        assert!(reg.get(None).linewise);
        reg.set(None, "world".into(), false);
        assert!(!reg.get(None).linewise);
    }
}
