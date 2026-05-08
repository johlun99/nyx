mod command_palette;
mod filetree;

pub use command_palette::{CommandPalette, PaletteAction};
pub use filetree::FiletreeModule;

#[derive(PartialEq, Eq)]
pub enum ModuleAction {
    None,
    OpenFile(String),
}
