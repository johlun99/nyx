mod command_palette;
mod filetree;
mod terminal;

pub use command_palette::{CommandPalette, PaletteAction};
pub use filetree::FiletreeModule;
pub use terminal::TerminalModule;

#[derive(PartialEq, Eq)]
pub enum ModuleAction {
    None,
    OpenFile(String),
}
