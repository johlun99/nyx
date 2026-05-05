// src/app.rs
use crate::config::NyxConfig;
use crate::editor::Editor;
use crate::renderer::{EditorView, Theme};
use crate::vim::Mode;
use eframe::egui;

pub struct NyxApp {
    editor: Editor,
    editor_view: EditorView,
    theme: Theme,
    config: NyxConfig,
}

impl NyxApp {
    pub fn new(file_path: Option<String>, config: NyxConfig) -> Self {
        Self {
            editor: Editor::new(file_path),
            editor_view: EditorView::new(),
            theme: Theme::default_dark(),
            config,
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        ctx.input(|input| {
            // Command mode intercepts all input
            if self.editor.mode() == Mode::Command {
                if input.key_pressed(egui::Key::Enter) {
                    self.editor.execute_command();
                    return;
                }
                if input.key_pressed(egui::Key::Backspace) {
                    self.editor.handle_command_backspace();
                    return;
                }
                if input.key_pressed(egui::Key::Escape)
                    || (input.modifiers.ctrl && input.key_pressed(egui::Key::OpenBracket))
                {
                    self.editor.command_parser.clear();
                    let action = self.editor.key_parser.handle_escape();
                    self.editor.apply_action(action);
                    return;
                }
                for event in &input.events {
                    if let egui::Event::Text(text) = event {
                        for ch in text.chars() {
                            self.editor.handle_command_char(ch);
                        }
                    }
                }
                return;
            }

            // Escape and Ctrl+[ (both exit to Normal mode)
            if input.key_pressed(egui::Key::Escape)
                || (input.modifiers.ctrl && input.key_pressed(egui::Key::OpenBracket))
            {
                let action = self.editor.key_parser.handle_escape();
                self.editor.apply_action(action);
                return;
            }

            // Ctrl+R for redo — only in Normal mode
            if self.editor.mode() == Mode::Normal
                && input.modifiers.ctrl
                && input.key_pressed(egui::Key::R)
            {
                let action = self.editor.key_parser.handle_ctrl_r();
                self.editor.apply_action(action);
                return;
            }

            // Backspace
            if input.key_pressed(egui::Key::Backspace) {
                let action = self.editor.key_parser.handle_backspace();
                self.editor.apply_action(action);
                return;
            }

            // Enter
            if input.key_pressed(egui::Key::Enter) {
                if self.editor.mode() == Mode::Insert {
                    let action = self.editor.key_parser.handle_key('\n');
                    self.editor.apply_action(action);
                } else if self.editor.mode() == Mode::Normal {
                    let action = self.editor.key_parser.handle_key('j');
                    self.editor.apply_action(action);
                }
                return;
            }

            // Text input
            for event in &input.events {
                if let egui::Event::Text(text) = event {
                    for ch in text.chars() {
                        let action = self.editor.key_parser.handle_key(ch);
                        self.editor.apply_action(action);
                    }
                }
            }
        });
    }
}

impl eframe::App for NyxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.editor.should_quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        self.handle_input(ctx);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let command_input = self.editor.command_input();
                self.editor_view.render(
                    ui,
                    &self.editor.buffer,
                    &self.theme,
                    self.editor.mode(),
                    self.config.editor.font_size,
                    self.editor.file_path.as_deref(),
                    command_input,
                    self.editor.status_message.as_deref(),
                );
            });
    }
}
