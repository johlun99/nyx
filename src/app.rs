// src/app.rs
use crate::config::NyxConfig;
use crate::editor::Editor;
use crate::renderer::{EditorView, Theme};
use crate::views::{AppView, KeybindingsView, SettingsAction, SettingsView};
use crate::vim::{Mode, VimAction, VisualKind};
use eframe::egui;

pub struct NyxApp {
    editor: Editor,
    editor_view: EditorView,
    theme: Theme,
    config: NyxConfig,
    active_view: AppView,
    keybindings_view: KeybindingsView,
    settings_view: SettingsView,
}

impl NyxApp {
    pub fn new(file_path: Option<String>, config: NyxConfig) -> Self {
        let mut editor = Editor::new(file_path);
        editor.set_tab_size(config.editor.tab_size);
        Self {
            editor,
            editor_view: EditorView::new(),
            theme: Theme::default_dark(),
            config,
            active_view: AppView::default(),
            keybindings_view: KeybindingsView::new(),
            settings_view: SettingsView::new(),
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        // --- App-level shortcuts (work from any view) ---
        let mut view_switch: Option<AppView> = None;
        ctx.input(|input| {
            if input.modifiers.command && input.key_pressed(egui::Key::Comma) {
                view_switch = Some(match self.active_view {
                    AppView::Settings => AppView::Editor,
                    _ => AppView::Settings,
                });
            }
            if input.modifiers.command && input.key_pressed(egui::Key::K) {
                view_switch = Some(match self.active_view {
                    AppView::Keybindings => AppView::Editor,
                    _ => AppView::Keybindings,
                });
            }
        });
        if let Some(new_view) = view_switch {
            if new_view == AppView::Keybindings {
                self.keybindings_view.search.clear();
            }
            self.active_view = new_view;
            return;
        }

        // --- Non-editor view input ---
        match self.active_view {
            AppView::Keybindings => {
                let should_close = self.keybindings_view.handle_input(ctx);
                if should_close {
                    self.active_view = AppView::Editor;
                }
                return;
            }
            AppView::Settings => {
                let action = self.settings_view.handle_input(ctx, &mut self.config);
                match action {
                    SettingsAction::Close => {
                        self.active_view = AppView::Editor;
                    }
                    SettingsAction::ConfigChanged => {
                        self.editor.set_tab_size(self.config.editor.tab_size);
                        let _ = self.config.save(&NyxConfig::config_path());
                    }
                    SettingsAction::None => {}
                }
                return;
            }
            AppView::Editor => {}
        }

        // --- Editor input (unchanged from here down) ---
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

            // Search input mode
            if self.editor.search_input.is_some() {
                if input.key_pressed(egui::Key::Enter) {
                    self.editor.execute_search();
                    return;
                }
                if input.key_pressed(egui::Key::Escape)
                    || (input.modifiers.ctrl && input.key_pressed(egui::Key::OpenBracket))
                {
                    self.editor.search_input = None;
                    let action = self.editor.key_parser.handle_escape();
                    self.editor.apply_action(action);
                    return;
                }
                if input.key_pressed(egui::Key::Backspace) {
                    self.editor.handle_search_backspace();
                    return;
                }
                for event in &input.events {
                    if let egui::Event::Text(text) = event {
                        for ch in text.chars() {
                            self.editor.handle_search_char(ch);
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

            // Ctrl+V for visual block mode — only in Normal mode
            if self.editor.mode() == Mode::Normal
                && input.modifiers.ctrl
                && input.key_pressed(egui::Key::V)
            {
                self.editor.key_parser.set_mode(Mode::VisualBlock);
                let action = VimAction::EnterVisual(VisualKind::Block);
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

        match self.active_view {
            AppView::Editor => {
                self.editor.ensure_syntax_parsed();
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        self.editor_view.render(
                            ui,
                            &self.editor,
                            &self.theme,
                            self.config.editor.font_size,
                            self.config.editor.line_numbers,
                        );
                    });
            }
            AppView::Settings => {
                let changed = self.settings_view.render(
                    ctx,
                    &mut self.config,
                    &self.theme,
                );
                if changed {
                    self.editor.set_tab_size(self.config.editor.tab_size);
                    let _ = self.config.save(&NyxConfig::config_path());
                }
            }
            AppView::Keybindings => {
                // Render editor behind (it will be dimmed by the overlay)
                self.editor.ensure_syntax_parsed();
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        self.editor_view.render(
                            ui,
                            &self.editor,
                            &self.theme,
                            self.config.editor.font_size,
                            self.config.editor.line_numbers,
                        );
                    });
                // Overlay on top
                self.keybindings_view.render(ctx, &self.theme);
            }
        }
    }
}
