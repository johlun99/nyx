mod keybindings;
mod settings;

pub use keybindings::KeybindingsView;
pub use settings::{SettingsAction, SettingsView};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppView {
    Editor,
    Settings,
    Keybindings,
}

impl Default for AppView {
    fn default() -> Self {
        Self::Editor
    }
}
