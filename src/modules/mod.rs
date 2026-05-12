mod command_palette;
mod filetree;
mod git;
mod search_popup;
mod terminal;

pub use command_palette::{CommandPalette, PaletteAction};
pub use filetree::FiletreeModule;
pub use git::GitModule;
pub use search_popup::{SearchAction, SearchMode, SearchPopup};
pub use terminal::TerminalModule;

#[derive(PartialEq, Eq)]
pub enum ModuleAction {
    None,
    OpenFile(String),
    ViewDiff { path: String, staged: bool },
}
