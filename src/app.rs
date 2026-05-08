// src/app.rs
use crate::config::lsp_config::LspConfig;
use crate::config::panels_config::PanelsConfig;
use crate::config::NyxConfig;
use crate::editor::Editor;
use crate::lsp::LspManager;
use crate::modules::{CommandPalette, FiletreeModule, ModuleAction, PaletteAction};
use crate::renderer::{EditorView, Theme};
use crate::syntax::languages::language_for_extension;
use crate::views::{
    AppView, KeybindingsView, LspServersView, PanelFocus, PanelSlot, SettingsAction, SettingsTab,
    SettingsView,
};
use crate::vim::{Mode, VimAction, VisualKind};
use eframe::egui;
use std::time::{Duration, Instant};

/// LSP actions triggered from vim keybindings, handled outside the input closure.
enum LspAction {
    GotoDefinition,
    References,
    Hover,
}

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
    /// Panel system
    panel_focus: PanelFocus,
    left_panel_visible: bool,
    bottom_panel_visible: bool,
    right_panel_visible: bool,
    filetree: FiletreeModule,
    command_palette: CommandPalette,
    command_palette_open: bool,
    panels_config: PanelsConfig,
    /// Active tab index per panel: [left, bottom, right]
    panel_active_tab: [usize; 3],
}

impl NyxApp {
    pub fn new(file_path: Option<String>, config: NyxConfig) -> Self {
        let mut editor = Editor::new(file_path);
        editor.set_tab_size(config.editor.tab_size);

        let lsp_config = LspConfig::load_or_create(&LspConfig::config_path());
        let lsp_manager = LspManager::new(lsp_config);

        // Determine filetree root: parent dir of opened file, or cwd
        let filetree_root = editor
            .file_path
            .as_deref()
            .and_then(|p| std::path::Path::new(p).parent().map(|d| d.to_path_buf()))
            .or_else(|| std::env::current_dir().ok());

        let config_dir = NyxConfig::config_dir();
        let panels_config = {
            let path = config_dir.join("panels.json");
            if path.exists() {
                PanelsConfig::load(&config_dir)
            } else {
                let migrated = PanelsConfig::migrate_from_modules(&config.modules);
                let _ = migrated.save(&config_dir);
                migrated
            }
        };

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
            panel_focus: PanelFocus::default(),
            left_panel_visible: !panels_config.is_empty(PanelSlot::Left),
            bottom_panel_visible: !panels_config.is_empty(PanelSlot::Bottom),
            right_panel_visible: !panels_config.is_empty(PanelSlot::Right),
            filetree: FiletreeModule::new(filetree_root),
            command_palette: CommandPalette::new(),
            command_palette_open: false,
            panels_config,
            panel_active_tab: [0; 3],
        }
    }

    fn filetree_slot(&self) -> PanelSlot {
        [PanelSlot::Left, PanelSlot::Bottom, PanelSlot::Right]
            .into_iter()
            .find(|s| {
                self.panels_config
                    .tabs_for(*s)
                    .iter()
                    .any(|t| t.modules.iter().any(|m| m == "filetree"))
            })
            .unwrap_or(PanelSlot::Left)
    }

    fn capitalize(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_uppercase().to_string() + chars.as_str(),
        }
    }

    fn panel_visible(&self, slot: PanelSlot) -> bool {
        match slot {
            PanelSlot::Left => self.left_panel_visible,
            PanelSlot::Bottom => self.bottom_panel_visible,
            PanelSlot::Right => self.right_panel_visible,
        }
    }

    fn set_panel_visible(&mut self, slot: PanelSlot, visible: bool) {
        match slot {
            PanelSlot::Left => self.left_panel_visible = visible,
            PanelSlot::Bottom => self.bottom_panel_visible = visible,
            PanelSlot::Right => self.right_panel_visible = visible,
        }
    }

    fn panel_focus_for_slot(slot: PanelSlot) -> PanelFocus {
        match slot {
            PanelSlot::Left => PanelFocus::LeftPanel,
            PanelSlot::Bottom => PanelFocus::BottomPanel,
            PanelSlot::Right => PanelFocus::RightPanel,
        }
    }

    fn slot_for_focus(focus: PanelFocus) -> Option<PanelSlot> {
        match focus {
            PanelFocus::LeftPanel => Some(PanelSlot::Left),
            PanelFocus::BottomPanel => Some(PanelSlot::Bottom),
            PanelFocus::RightPanel => Some(PanelSlot::Right),
            PanelFocus::Editor => None,
        }
    }

    fn render_panel_modules(
        &mut self,
        ui: &mut egui::Ui,
        slot: PanelSlot,
        slot_index: usize,
        focused: bool,
    ) -> ModuleAction {
        let tabs = self.panels_config.tabs_for(slot);
        let active_tab_idx = self.panel_active_tab[slot_index];
        if let Some(tab) = tabs.get(active_tab_idx).or_else(|| tabs.first()) {
            for module in &tab.modules.clone() {
                match module.as_str() {
                    "filetree" => {
                        let action = self.filetree.render(ui, &self.theme, focused);
                        if action != ModuleAction::None {
                            return action;
                        }
                    }
                    other => {
                        let label = format!("{} — coming soon", Self::capitalize(other));
                        ui.label(
                            egui::RichText::new(label)
                                .color(self.theme.line_number)
                                .size(12.0)
                                .italics(),
                        );
                    }
                }
            }
        }
        ModuleAction::None
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        // --- Command palette interception (highest priority) ---
        if self.command_palette_open {
            let (should_close, action) = self.command_palette.handle_input(ctx);
            if should_close {
                self.command_palette_open = false;
                if action != PaletteAction::None {
                    self.apply_palette_action(action);
                }
            }
            return;
        }

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

        // --- Code action popup interception ---
        if self.lsp_manager.code_actions.is_some() && self.active_view == AppView::Editor {
            let mut handled = false;
            let mut accept_action = false;
            ctx.input(|input| {
                if input.key_pressed(egui::Key::Escape) {
                    self.lsp_manager.dismiss_code_actions();
                    handled = true;
                    return;
                }
                if input.key_pressed(egui::Key::ArrowDown) || input.key_pressed(egui::Key::J) {
                    if let Some(ref mut state) = self.lsp_manager.code_actions {
                        state.move_selection(1);
                    }
                    handled = true;
                    return;
                }
                if input.key_pressed(egui::Key::ArrowUp) || input.key_pressed(egui::Key::K) {
                    if let Some(ref mut state) = self.lsp_manager.code_actions {
                        state.move_selection(-1);
                    }
                    handled = true;
                    return;
                }
                if input.key_pressed(egui::Key::Enter) {
                    handled = true;
                    accept_action = true;
                }
            });
            if handled {
                if accept_action {
                    if let Some(edit) = self.lsp_manager.accept_code_action() {
                        self.apply_workspace_edit(&edit);
                        self.notify_lsp_change();
                    }
                }
                return;
            }
        }

        // --- Dismiss hover on any keypress ---
        if self.lsp_manager.hover_result.is_some() && self.active_view == AppView::Editor {
            let mut any_key = false;
            ctx.input(|input| {
                any_key = !input.events.is_empty();
            });
            if any_key {
                self.lsp_manager.dismiss_hover();
            }
        }

        // --- App-level shortcuts (work from any view) ---
        let mut view_switch: Option<AppView> = None;
        let mut toggle_panel = false;
        let mut toggle_palette = false;
        let mut focus_panel_slot: Option<PanelSlot> = None;
        ctx.input(|input| {
            if input.modifiers.ctrl && input.key_pressed(egui::Key::H) {
                focus_panel_slot = Some(PanelSlot::Left);
            }
            if input.modifiers.ctrl && input.key_pressed(egui::Key::J) {
                focus_panel_slot = Some(PanelSlot::Bottom);
            }
            if input.modifiers.ctrl && input.key_pressed(egui::Key::L) {
                focus_panel_slot = Some(PanelSlot::Right);
            }
            if input.modifiers.command && input.key_pressed(egui::Key::B) {
                toggle_panel = true;
            }
            if input.modifiers.command && input.key_pressed(egui::Key::P) {
                toggle_palette = true;
            }
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
        if toggle_panel {
            let slot = self.filetree_slot();
            let new_vis = !self.panel_visible(slot);
            self.set_panel_visible(slot, new_vis);
            if !new_vis {
                self.panel_focus = PanelFocus::Editor;
            }
            return;
        }
        if toggle_palette {
            self.command_palette_open = !self.command_palette_open;
            if self.command_palette_open {
                self.command_palette.reset();
            }
            return;
        }
        if let Some(slot) = focus_panel_slot {
            if self.active_view == AppView::Editor && !self.panels_config.is_empty(slot) {
                let target_focus = Self::panel_focus_for_slot(slot);
                if self.panel_focus == target_focus {
                    // Already focused — hide panel
                    self.set_panel_visible(slot, false);
                    self.panel_focus = PanelFocus::Editor;
                } else {
                    // Open panel if hidden, then focus it
                    self.set_panel_visible(slot, true);
                    self.panel_focus = target_focus;
                }
                return;
            }
        }
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
                    &mut self.panels_config,
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

        // --- Panel input routing ---
        if let Some(slot) = Self::slot_for_focus(self.panel_focus) {
            if self.panel_visible(slot) {
                // Escape returns focus to editor
                let escape_pressed = ctx.input(|input| {
                    input.key_pressed(egui::Key::Escape)
                        || (input.modifiers.ctrl && input.key_pressed(egui::Key::OpenBracket))
                });
                if escape_pressed {
                    self.panel_focus = PanelFocus::Editor;
                    return;
                }
                // Route input to modules in the focused panel
                let slot_index = match slot {
                    PanelSlot::Left => 0,
                    PanelSlot::Bottom => 1,
                    PanelSlot::Right => 2,
                };
                let active_tab_idx = self.panel_active_tab[slot_index];
                let has_filetree = self
                    .panels_config
                    .tabs_for(slot)
                    .get(active_tab_idx)
                    .or_else(|| self.panels_config.tabs_for(slot).first())
                    .map(|tab| tab.modules.iter().any(|m| m == "filetree"))
                    .unwrap_or(false);
                if has_filetree {
                    let action = self.filetree.handle_input(ctx);
                    match action {
                        ModuleAction::OpenFile(path) => {
                            self.open_file(&path);
                            self.panel_focus = PanelFocus::Editor;
                        }
                        ModuleAction::None => {}
                    }
                }
                return;
            }
        }

        // --- Editor input ---
        let mut lsp_action: Option<LspAction> = None;
        let mut rename_request: Option<String> = None;
        ctx.input(|input| {
            // Command mode intercepts all input
            if self.editor.mode() == Mode::Command {
                if input.key_pressed(egui::Key::Enter) {
                    if let Some(new_name) = self.editor.execute_command() {
                        // :rename command was executed
                        rename_request = Some(new_name);
                    }
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

            // Ctrl+O for jump back — only in Normal mode
            if self.editor.mode() == Mode::Normal
                && input.modifiers.ctrl
                && input.key_pressed(egui::Key::O)
            {
                if let Some(pos) = self.editor.jump_list.go_back() {
                    self.editor.buffer.set_cursor(pos.line, pos.col);
                    self.editor.buffer.clamp_cursor_normal();
                }
                return;
            }

            // Ctrl+I for jump forward — only in Normal mode
            if self.editor.mode() == Mode::Normal
                && input.modifiers.ctrl
                && input.key_pressed(egui::Key::I)
            {
                if let Some(pos) = self.editor.jump_list.go_forward() {
                    self.editor.buffer.set_cursor(pos.line, pos.col);
                    self.editor.buffer.clamp_cursor_normal();
                }
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

            // Ctrl+. for code actions — only in Normal mode
            if self.editor.mode() == Mode::Normal
                && input.modifiers.ctrl
                && input.key_pressed(egui::Key::Period)
            {
                self.trigger_code_actions();
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
                        // Check for LSP actions before consuming
                        match &action {
                            VimAction::LspGotoDefinition => {
                                lsp_action = Some(LspAction::GotoDefinition);
                            }
                            VimAction::LspReferences => {
                                lsp_action = Some(LspAction::References);
                            }
                            VimAction::LspHover => {
                                lsp_action = Some(LspAction::Hover);
                            }
                            _ => {}
                        }
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

        // Handle LSP actions outside the input closure
        match lsp_action {
            Some(LspAction::GotoDefinition) => self.trigger_goto_definition(),
            Some(LspAction::References) => self.trigger_references(),
            Some(LspAction::Hover) => self.trigger_hover(),
            None => {}
        }
        if let Some(new_name) = rename_request {
            self.trigger_rename(&new_name);
        }
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

    fn handle_goto_results(&mut self, locations: Vec<crate::lsp::GotoLocation>) {
        if locations.is_empty() {
            self.editor.status_message = Some("No results found".to_string());
            return;
        }

        let current_path = self
            .editor
            .file_path
            .as_deref()
            .and_then(|p| std::fs::canonicalize(p).ok())
            .unwrap_or_default();
        let cursor_line = self.editor.buffer.cursor_line();
        let cursor_col = self.editor.buffer.cursor_col();

        // Partition into same-file and other-file locations
        let is_same_file = |loc: &crate::lsp::GotoLocation| {
            std::fs::canonicalize(&loc.file_path)
                .unwrap_or_else(|_| std::path::PathBuf::from(&loc.file_path))
                == current_path
        };

        // For same-file results, skip the one we're already on
        let is_different_position =
            |loc: &crate::lsp::GotoLocation| loc.line != cursor_line || loc.col != cursor_col;

        // Try to find a same-file result that isn't the current position
        let jump_target = locations
            .iter()
            .find(|l| is_same_file(l) && is_different_position(l));

        if let Some(loc) = jump_target {
            // Count same-file results for status message
            let same_file_count = locations.iter().filter(|l| is_same_file(l)).count();
            self.editor.jump_list.push(cursor_line, cursor_col);
            self.editor.buffer.set_cursor(loc.line, loc.col);
            self.editor.buffer.clamp_cursor_normal();
            if same_file_count > 1 {
                self.editor.status_message = Some(format!("[1/{}] results", same_file_count));
            }
        } else if locations.len() == 1 && is_same_file(&locations[0]) {
            // Only result is where we already are
            self.editor.status_message = Some("Already at the only result".to_string());
        } else {
            // All results are in other files — show the first one
            let loc = locations
                .iter()
                .find(|l| !is_same_file(l))
                .unwrap_or(&locations[0]);
            self.editor.status_message = Some(format!(
                "{}:{}:{} ({} results)",
                loc.file_path,
                loc.line + 1,
                loc.col + 1,
                locations.len()
            ));
        }
    }

    fn trigger_goto_definition(&mut self) {
        if let Some(ref path) = self.editor.file_path {
            let text = self.editor.buffer.text();
            let line = self.editor.buffer.cursor_line();
            let col = self.editor.buffer.cursor_col();
            self.lsp_manager
                .request_goto_definition(path, &text, line, col);
        }
    }

    fn trigger_references(&mut self) {
        if let Some(ref path) = self.editor.file_path {
            let text = self.editor.buffer.text();
            let line = self.editor.buffer.cursor_line();
            let col = self.editor.buffer.cursor_col();
            self.lsp_manager.request_references(path, &text, line, col);
        }
    }

    fn trigger_hover(&mut self) {
        if let Some(ref path) = self.editor.file_path {
            let text = self.editor.buffer.text();
            let line = self.editor.buffer.cursor_line();
            let col = self.editor.buffer.cursor_col();
            self.lsp_manager.request_hover(path, &text, line, col);
        }
    }

    fn trigger_rename(&mut self, new_name: &str) {
        if let Some(ref path) = self.editor.file_path {
            let text = self.editor.buffer.text();
            let line = self.editor.buffer.cursor_line();
            let col = self.editor.buffer.cursor_col();
            self.lsp_manager
                .request_rename(path, &text, line, col, new_name);
        }
    }

    fn trigger_code_actions(&mut self) {
        if let Some(ref path) = self.editor.file_path {
            let text = self.editor.buffer.text();
            let line = self.editor.buffer.cursor_line();
            let col = self.editor.buffer.cursor_col();
            self.lsp_manager
                .request_code_actions(path, &text, line, col);
        }
    }

    fn apply_workspace_edit(&mut self, edit: &crate::lsp::protocol::WorkspaceEdit) {
        let current_uri = self
            .editor
            .file_path
            .as_deref()
            .map(crate::lsp::path_to_uri);
        if let Some(ref changes) = edit.changes {
            if let Some(ref uri) = current_uri {
                if let Some(edits) = changes.get(uri) {
                    let text = self.editor.buffer.text();
                    let new_text = crate::lsp::apply_workspace_edit_to_text(&text, edits);
                    // Replace buffer contents while preserving cursor approximately
                    let cursor_line = self.editor.buffer.cursor_line();
                    let cursor_col = self.editor.buffer.cursor_col();
                    let len = self.editor.buffer.len_chars();
                    self.editor.buffer.delete_range(0, len);
                    self.editor.buffer.insert_text_at(0, &new_text);
                    self.editor.buffer.set_cursor(cursor_line, cursor_col);
                    self.editor.buffer.clamp_cursor_normal();
                }
            }
        }
    }

    fn open_file(&mut self, path: &str) {
        // Close old LSP document
        if let Some(ref old_path) = self.editor.file_path {
            self.lsp_manager.notify_document_close(old_path);
        }

        // Read file and replace buffer
        let content = match crate::file_io::read_file(std::path::Path::new(path)) {
            Ok(content) => content,
            Err(e) => {
                self.editor.status_message = Some(format!("Failed to open: {}", e));
                return;
            }
        };
        self.editor.buffer = crate::buffer::TextBuffer::from_text(&content);
        self.editor.file_path = Some(path.to_string());
        self.editor.mark_saved();

        // Re-initialize syntax highlighting
        let ext = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        self.editor.syntax_state = language_for_extension(ext).and_then(|lang_name| {
            let mut state = crate::syntax::SyntaxState::new(lang_name, ext)?;
            state.parse(&self.editor.buffer.text());
            Some(state)
        });

        // Reset LSP state for new file
        self.lsp_document_opened = false;
        self.lsp_manager.dismiss_completion();
        self.lsp_manager.dismiss_hover();

        // Reset editor state
        self.editor.key_parser.set_mode(Mode::Normal);
        self.editor.status_message = Some(format!("Opened {}", path));
    }

    fn apply_palette_action(&mut self, action: PaletteAction) {
        match action {
            PaletteAction::None => {}
            PaletteAction::ToggleFiletree => {
                let slot = self.filetree_slot();
                let new_vis = !self.panel_visible(slot);
                self.set_panel_visible(slot, new_vis);
                if !new_vis {
                    self.panel_focus = PanelFocus::Editor;
                }
            }
            PaletteAction::OpenSettings => {
                self.active_view = AppView::Settings;
            }
            PaletteAction::OpenKeybindings => {
                self.keybindings_view.search.clear();
                self.active_view = AppView::Keybindings;
            }
            PaletteAction::OpenLspServers => {
                self.settings_view.active_tab = SettingsTab::LspServers;
                self.lsp_view.search.clear();
                self.lsp_view.selected_row = 0;
                self.active_view = AppView::Settings;
            }
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

        // Handle goto definition/references results
        if let Some(locations) = self.lsp_manager.goto_result.take() {
            self.handle_goto_results(locations);
        }

        // Handle rename results
        if let Some(edit) = self.lsp_manager.take_rename_result() {
            self.apply_workspace_edit(&edit);
            self.notify_lsp_change();
            self.editor.status_message = Some("Rename applied".to_string());
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

        // Panels — must be rendered before CentralPanel per egui rules
        let mut panel_action = ModuleAction::None;
        let mut panel_clicked = None;
        if self.active_view == AppView::Editor {
            let panel_bg = self.theme.background;

            // Detect primary click position for panel focus
            let click_pos = ctx.input(|i| {
                if i.pointer.any_pressed() {
                    i.pointer.interact_pos()
                } else {
                    None
                }
            });

            // Left panel
            if self.left_panel_visible {
                let focused = self.panel_focus == PanelFocus::LeftPanel;
                if let Some(resp) = egui::SidePanel::left("left_panel")
                    .default_width(220.0)
                    .width_range(150.0..=400.0)
                    .frame(egui::Frame::NONE.fill(panel_bg).inner_margin(8.0))
                    .show_animated(ctx, true, |ui| {
                        let action = self.render_panel_modules(ui, PanelSlot::Left, 0, focused);
                        if action != ModuleAction::None {
                            panel_action = action;
                        }
                    })
                {
                    if let Some(pos) = click_pos {
                        if resp.response.rect.contains(pos) {
                            panel_clicked = Some(PanelFocus::LeftPanel);
                        }
                    }
                }
            }

            // Right panel
            if self.right_panel_visible {
                let focused = self.panel_focus == PanelFocus::RightPanel;
                if let Some(resp) = egui::SidePanel::right("right_panel")
                    .default_width(220.0)
                    .width_range(150.0..=400.0)
                    .frame(egui::Frame::NONE.fill(panel_bg).inner_margin(8.0))
                    .show_animated(ctx, true, |ui| {
                        let action = self.render_panel_modules(ui, PanelSlot::Right, 2, focused);
                        if action != ModuleAction::None {
                            panel_action = action;
                        }
                    })
                {
                    if let Some(pos) = click_pos {
                        if resp.response.rect.contains(pos) {
                            panel_clicked = Some(PanelFocus::RightPanel);
                        }
                    }
                }
            }

            // Bottom panel
            if self.bottom_panel_visible {
                let focused = self.panel_focus == PanelFocus::BottomPanel;
                if let Some(resp) = egui::TopBottomPanel::bottom("bottom_panel")
                    .default_height(200.0)
                    .height_range(100.0..=300.0)
                    .frame(egui::Frame::NONE.fill(panel_bg).inner_margin(8.0))
                    .show_animated(ctx, true, |ui| {
                        let action = self.render_panel_modules(ui, PanelSlot::Bottom, 1, focused);
                        if action != ModuleAction::None {
                            panel_action = action;
                        }
                    })
                {
                    if let Some(pos) = click_pos {
                        if resp.response.rect.contains(pos) {
                            panel_clicked = Some(PanelFocus::BottomPanel);
                        }
                    }
                }
            }
        }
        if let Some(focus) = panel_clicked {
            self.panel_focus = focus;
        }
        if let ModuleAction::OpenFile(path) = panel_action {
            self.open_file(&path);
            self.panel_focus = PanelFocus::Editor;
        }

        match self.active_view {
            AppView::Editor => {
                self.editor.ensure_syntax_parsed();
                let mut click = None;
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        click = self.editor_view.render(
                            ui,
                            &self.editor,
                            &self.theme,
                            self.config.editor.font_size,
                            self.config.editor.line_numbers,
                            &self.lsp_manager,
                        );
                    });
                if let Some(c) = click {
                    self.editor.buffer.set_cursor(c.line, c.col);
                    self.panel_focus = PanelFocus::Editor;
                }
            }
            AppView::Settings => {
                let changed = self.settings_view.render(
                    ctx,
                    &mut self.config,
                    &self.theme,
                    &self.lsp_view,
                    &self.lsp_manager,
                    &self.panels_config,
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

        // Command palette overlay (rendered on top of everything)
        if self.command_palette_open {
            self.command_palette.render(ctx, &self.theme);
        }
    }
}
