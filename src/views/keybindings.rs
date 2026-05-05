pub struct KeybindingsView {
    pub search: String,
}

impl KeybindingsView {
    pub fn new() -> Self {
        Self {
            search: String::new(),
        }
    }
}
