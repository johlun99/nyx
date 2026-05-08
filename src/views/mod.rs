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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PanelFocus {
    #[default]
    Editor,
    LeftPanel,
    BottomPanel,
    RightPanel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelSlot {
    Left,
    Bottom,
    Right,
}

impl PanelSlot {
    pub fn from_config(s: &str) -> Option<Self> {
        match s {
            "left" => Some(Self::Left),
            "bottom" => Some(Self::Bottom),
            "right" => Some(Self::Right),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn next(&self) -> Self {
        match self {
            Self::Left => Self::Bottom,
            Self::Bottom => Self::Right,
            Self::Right => Self::Left,
        }
    }

    #[allow(dead_code)]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Bottom => "bottom",
            Self::Right => "right",
        }
    }
}
