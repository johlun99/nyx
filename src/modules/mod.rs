mod command_palette;
mod filetree;

pub use command_palette::{CommandPalette, PaletteAction};
pub use filetree::FiletreeModule;

pub enum ModuleAction {
    None,
    OpenFile(String),
}
