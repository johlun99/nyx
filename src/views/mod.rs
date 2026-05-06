mod keybindings;
mod settings;

pub use keybindings::KeybindingsView;
pub use settings::{SettingsAction, SettingsView};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AppView {
    #[default]
    Editor,
    Settings,
    Keybindings,
}
