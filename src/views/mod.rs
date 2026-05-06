mod keybindings;
mod lsp_servers;
mod settings;

pub use keybindings::KeybindingsView;
pub use lsp_servers::LspServersView;
pub use settings::{SettingsAction, SettingsTab, SettingsView};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AppView {
    #[default]
    Editor,
    Settings,
    Keybindings,
}
