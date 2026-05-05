pub struct SettingsView {
    pub selected_row: usize,
}

impl SettingsView {
    pub fn new() -> Self {
        Self {
            selected_row: 0,
        }
    }
}
