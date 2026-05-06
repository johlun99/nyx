// src/app.rs
use crate::config::lsp_config::LspConfig;
use crate::config::NyxConfig;
use crate::editor::Editor;
use crate::lsp::LspManager;
use crate::renderer::{EditorView, Theme};
use crate::syntax::languages::language_for_extension;
use crate::views::{
    AppView, KeybindingsView, LspServersView, SettingsAction, SettingsTab, SettingsView,
};
use crate::vim::{Mode, VimAction, VisualKind};
use eframe::egui;
use std::time::{Duration, Instant};

pub struct NyxApp {
    editor: Editor,
    editor_view: EditorView,
    theme: Theme,
    config: NyxConfig,
    active_view: AppView,
    keybindings_view: KeybindingsView,
    settings_view: SettingsView,
    lsp_manager: LspManager,
    lsp_view: LspServersView,
    /// Track if we've sent the initial didOpen
    lsp_document_opened: bool,
    /// Track the last typed character for `::` completion trigger
    last_typed_char: Option<char>,
    /// Debounce auto completion while typing identifiers
    last_completion_request: Option<Instant>,
    /// Last LSP error surfaced in editor status line
    last_lsp_error_shown: Option<String>,
}

impl NyxApp {
    pub fn new(file_path: Option<String>, config: NyxConfig) -> Self {
        let mut editor = Editor::new(file_path);
        editor.set_tab_size(config.editor.tab_size);

        let lsp_config = LspConfig::load_or_create(&LspConfig::config_path());
        let lsp_manager = LspManager::new(lsp_config);

        Self {
            editor,
            editor_view: EditorView::new(),
            theme: Theme::default_dark(),
            config,
            active_view: AppView::default(),
            keybindings_view: KeybindingsView::new(),
            settings_view: SettingsView::new(),
            lsp_manager,
            lsp_view: LspServersView::new(),
            lsp_document_opened: false,
            last_typed_char: None,
            last_completion_request: None,
            last_lsp_error_shown: None,
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        // --- Completion input interception ---
        if self.lsp_manager.completion.is_some() && self.active_view == AppView::Editor {
            let has_visible_completion_items = self
                .lsp_manager
                .completion
                .as_ref()
                .map(|c| !c.filtered_items().is_empty())
                .unwrap_or(false);
            let completion_has_filter = self
                .lsp_manager
                .completion
                .as_ref()
                .map(|c| !c.filter_text.is_empty())
                .unwrap_or(false);
            let mut handled = false;
            let mut accept_completion = false;
            ctx.input(|input| {
                if input.key_pressed(egui::Key::Escape) {
                    self.lsp_manager.dismiss_completion();
                    // Let the same Esc also propagate to normal input handling,
                    // so Insert mode can exit immediately.
                    return;
                }
                if has_visible_completion_items && input.key_pressed(egui::Key::ArrowDown) {
                    if let Some(ref mut state) = self.lsp_manager.completion {
                        state.move_selection(1);
                    }
                    handled = true;
                    return;
                }
                if has_visible_completion_items && input.key_pressed(egui::Key::ArrowUp) {
                    if let Some(ref mut state) = self.lsp_manager.completion {
                        state.move_selection(-1);
                    }
                    handled = true;
                    return;
                }
                if has_visible_completion_items
                    && (input.key_pressed(egui::Key::Tab)
                        || (input.key_pressed(egui::Key::Enter) && completion_has_filter))
                {
                    handled = true;
                    accept_completion = true;
                }
            });

            if handled {
                if accept_completion {
                    if let Some((text, anchor_col, _replace_end)) =
                        self.lsp_manager.accept_completion()
                    {
                        // Delete the filter text that was typed, then insert the completion
                        let cursor_col = self.editor.buffer.cursor_col();
                        let replace_start = anchor_col.min(cursor_col);
                        let chars_to_delete = cursor_col.saturating_sub(replace_start);
                        for _ in 0..chars_to_delete {
                            self.editor.buffer.delete_char_before_cursor();
                        }
                        for ch in text.chars() {
                            self.editor.buffer.insert_char(ch);
                        }
                        self.notify_lsp_change();
                    }
                }
                return;
            }
        }

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
            if input.modifiers.command && input.key_pressed(egui::Key::L) {
                if self.active_view == AppView::Settings
                    && self.settings_view.active_tab == SettingsTab::LspServers
                {
                    view_switch = Some(AppView::Editor);
                } else {
                    view_switch = Some(AppView::Settings);
                    self.settings_view.active_tab = SettingsTab::LspServers;
                }
            }
        });
        if let Some(new_view) = view_switch {
            if new_view == AppView::Keybindings {
                self.keybindings_view.search.clear();
            }
            if new_view == AppView::Settings
                && self.settings_view.active_tab == SettingsTab::LspServers
            {
                self.lsp_view.search.clear();
                self.lsp_view.selected_row = 0;
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
                let action = self.settings_view.handle_input(
                    ctx,
                    &mut self.config,
                    &mut self.lsp_view,
                    &mut self.lsp_manager,
                );
                match action {
                    SettingsAction::Close => {
                        self.active_view = AppView::Editor;
                    }
                    SettingsAction::ConfigChanged => {
                        self.editor.set_tab_size(self.config.editor.tab_size);
                        let _ = self.config.save(&NyxConfig::config_path());
                    }
                    SettingsAction::ServerToggled => {
                        self.lsp_document_opened = false;
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

            // Ctrl+Space for autocomplete
            if input.modifiers.ctrl && input.key_pressed(egui::Key::Space) {
                self.trigger_completion();
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
                self.notify_lsp_change();
                if let Some(ref mut state) = self.lsp_manager.completion {
                    state.filter_text.pop();
                    state.selected = 0;
                }
                return;
            }

            // Tab — insert spaces in insert mode
            if input.key_pressed(egui::Key::Tab) && self.editor.mode() == Mode::Insert {
                let tab_size = self.editor.tab_size;
                for _ in 0..tab_size {
                    let action = self.editor.key_parser.handle_key(' ');
                    self.editor.apply_action(action);
                }
                self.notify_lsp_change();
                return;
            }

            // Enter
            if input.key_pressed(egui::Key::Enter) {
                if self.editor.mode() == Mode::Insert {
                    let action = self.editor.key_parser.handle_key('\n');
                    self.editor.apply_action(action);
                    self.notify_lsp_change();
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

                        // Auto-trigger completion on '.' or '::'
                        let is_dot = ch == '.';
                        let is_double_colon = ch == ':' && self.last_typed_char == Some(':');
                        if self.editor.mode() == Mode::Insert && (is_dot || is_double_colon) {
                            self.notify_lsp_change();
                            self.trigger_completion();
                            self.last_typed_char = Some(ch);
                        } else if self.editor.mode() == Mode::Insert {
                            self.notify_lsp_change();
                            self.last_typed_char = Some(ch);
                            // Update completion filter text if popup is open
                            if let Some(ref mut state) = self.lsp_manager.completion {
                                if ch.is_alphanumeric() || ch == '_' {
                                    state.filter_text.push(ch);
                                    state.selected = 0;
                                } else {
                                    self.lsp_manager.completion = None;
                                }
                            } else if ch.is_alphanumeric() || ch == '_' {
                                // Auto-open completion while typing identifiers.
                                self.trigger_completion_debounced();
                            }
                        }
                    }
                }
            }
        });
    }

    fn trigger_completion(&mut self) {
        if let Some(ref path) = self.editor.file_path {
            let text = self.editor.buffer.text();
            let line = self.editor.buffer.cursor_line();
            let col = self.editor.buffer.cursor_col();
            self.lsp_manager.request_completion(path, &text, line, col);
            self.last_completion_request = Some(Instant::now());
        }
    }

    fn trigger_completion_debounced(&mut self) {
        let now = Instant::now();
        let debounce = Duration::from_millis(120);
        if self
            .last_completion_request
            .map(|last| now.duration_since(last) < debounce)
            .unwrap_or(false)
        {
            return;
        }
        self.trigger_completion();
    }

    fn notify_lsp_change(&mut self) {
        if let Some(ref path) = self.editor.file_path {
            let text = self.editor.buffer.text();
            self.lsp_manager.notify_document_change(path, &text);
        }
    }

    fn ensure_lsp_document_opened(&mut self) {
        if self.lsp_document_opened {
            return;
        }
        if let Some(ref path) = self.editor.file_path {
            let ext = std::path::Path::new(path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            if let Some(lang_id) = language_for_extension(ext) {
                let text = self.editor.buffer.text();
                self.lsp_manager.notify_document_open(path, lang_id, &text);
            }
        }
        // Always mark as attempted — ServerToggled resets this to retry.
        // Without this, we'd spam ensure_server_for_language every frame.
        self.lsp_document_opened = true;
    }
}

impl eframe::App for NyxApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.editor.should_quit {
            self.lsp_manager.shutdown_all();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // Ensure LSP document is opened on first frame
        self.ensure_lsp_document_opened();

        // Poll LSP manager for responses
        let buffer_text = Some(self.editor.buffer.text());
        let download_finished = self.lsp_manager.poll(buffer_text.as_deref());
        if download_finished {
            // A server binary was just installed — retry starting it
            self.lsp_document_opened = false;
        }
        if let Some(err) = self.lsp_manager.last_error.clone() {
            if self.last_lsp_error_shown.as_deref() != Some(err.as_str()) {
                self.editor.status_message = Some(format!("LSP error: {}", err));
                self.last_lsp_error_shown = Some(err);
            }
        }

        // Schedule gentle repaints so async LSP responses trigger UI updates
        if self.lsp_manager.has_clients() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }

        self.handle_input(ctx);
        if self.editor.take_did_save_event() {
            if let Some(ref path) = self.editor.file_path {
                let text = self.editor.buffer.text();
                self.lsp_manager.notify_document_save(path, &text);
            }
        }

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
                            &self.lsp_manager,
                        );
                    });
            }
            AppView::Settings => {
                let changed = self.settings_view.render(
                    ctx,
                    &mut self.config,
                    &self.theme,
                    &self.lsp_view,
                    &self.lsp_manager,
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
                            &self.lsp_manager,
                        );
                    });
                // Overlay on top
                self.keybindings_view.render(ctx, &self.theme);
            }
        }
    }
}
