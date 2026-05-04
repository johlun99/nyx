// src/app.rs
use crate::buffer::TextBuffer;
use crate::renderer::{EditorView, Theme};
use crate::vim::mode::Mode;

pub struct NyxApp {
    buffer: TextBuffer,
    editor_view: EditorView,
    theme: Theme,
    mode: Mode,
}

impl NyxApp {
    pub fn new() -> Self {
        Self {
            buffer: TextBuffer::from_text(
                "Welcome to Nyx!\n\nPress i to enter insert mode.\nPress : for commands.\nPress :q to quit.\n"
            ),
            editor_view: EditorView::new(),
            theme: Theme::default_dark(),
            mode: Mode::Normal,
        }
    }
}

impl eframe::App for NyxApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        eframe::egui::CentralPanel::default()
            .frame(eframe::egui::Frame::NONE)
            .show(ctx, |ui| {
                self.editor_view.render(
                    ui, &self.buffer, &self.theme, self.mode, 14.0, None, None, None,
                );
            });
    }
}
