mod claude_provider;
pub mod message;
pub mod provider;

use crate::config::ai_config::AiConfig;
use crate::modules::ModuleAction;
use crate::renderer::Theme;
use eframe::egui;
use std::path::PathBuf;

use message::{ChatMessage, ChatRole, StreamState};
use provider::{ChatContext, ProviderEvent, ProviderHandle, ProviderRequest};

pub struct AiChatModule {
    config: AiConfig,
    messages: Vec<ChatMessage>,
    input_buffer: String,
    stream_state: StreamState,
    current_response: String,
    provider_handle: Option<ProviderHandle>,
    auto_scroll: bool,
    working_directory: Option<PathBuf>,
    current_file_path: Option<String>,
    current_file_content: Option<String>,
    token_count: usize,
    session_token_count: usize,
    context_window: usize,
    status_text: String,
}

impl AiChatModule {
    pub fn new(config: AiConfig) -> Self {
        Self {
            config,
            messages: Vec::new(),
            input_buffer: String::new(),
            stream_state: StreamState::Idle,
            current_response: String::new(),
            provider_handle: None,
            auto_scroll: true,
            working_directory: std::env::current_dir().ok(),
            current_file_path: None,
            current_file_content: None,
            token_count: 0,
            session_token_count: 0,
            context_window: 0,
            status_text: String::new(),
        }
    }

    pub fn set_context(&mut self, file_path: Option<String>, file_content: Option<String>) {
        self.current_file_path = file_path;
        self.current_file_content = file_content;
    }

    pub fn poll_provider(&mut self) {
        let handle = match &self.provider_handle {
            Some(h) => h,
            None => return,
        };

        while let Ok(event) = handle.event_rx.try_recv() {
            match event {
                ProviderEvent::Status(status) => {
                    self.status_text = status;
                }
                ProviderEvent::Token(text) => {
                    self.status_text.clear();
                    self.current_response.push_str(&text);
                    self.auto_scroll = true;
                }
                ProviderEvent::Done {
                    token_usage,
                    context_window,
                } => {
                    self.status_text.clear();
                    if let Some(usage) = token_usage {
                        self.token_count = usage;
                        self.session_token_count += usage;
                    }
                    if let Some(cw) = context_window {
                        self.context_window = cw;
                    }
                    if !self.current_response.is_empty() {
                        self.messages.push(ChatMessage {
                            role: ChatRole::Assistant,
                            content: std::mem::take(&mut self.current_response),
                        });
                    }
                    self.stream_state = StreamState::Idle;
                }
                ProviderEvent::Error(err) => {
                    self.status_text.clear();
                    if !self.current_response.is_empty() {
                        self.messages.push(ChatMessage {
                            role: ChatRole::Assistant,
                            content: std::mem::take(&mut self.current_response),
                        });
                    }
                    self.stream_state = StreamState::Error(err);
                }
            }
        }
    }

    fn send_message(&mut self, ctx: &egui::Context) {
        let text = self.input_buffer.trim().to_string();
        if text.is_empty() {
            return;
        }

        self.messages.push(ChatMessage {
            role: ChatRole::User,
            content: text.clone(),
        });
        self.input_buffer.clear();

        let prompt = self.build_prompt(&text);

        if self.provider_handle.is_none() {
            if let Some(provider_config) = self.config.active_provider_config().cloned() {
                match claude_provider::spawn_provider(provider_config, ctx.clone()) {
                    Ok(handle) => self.provider_handle = Some(handle),
                    Err(e) => {
                        self.stream_state = StreamState::Error(e);
                        return;
                    }
                }
            } else {
                self.stream_state = StreamState::Error("No active provider configured".to_string());
                return;
            }
        }

        let context = ChatContext {
            file_path: self.current_file_path.clone(),
            file_content: self.current_file_content.clone(),
            selection: None,
            working_directory: self.working_directory.clone(),
        };

        if let Some(handle) = &self.provider_handle {
            let _ = handle.request_tx.send(ProviderRequest::SendMessage {
                prompt,
                context: Some(context),
            });
        }

        self.stream_state = StreamState::Streaming;
        self.current_response.clear();
        self.token_count = 0;
        self.auto_scroll = true;
    }

    fn build_prompt(&self, new_message: &str) -> String {
        if self.messages.len() <= 1 {
            return new_message.to_string();
        }

        let mut prompt = String::new();
        for msg in &self.messages[..self.messages.len() - 1] {
            let role_label = match msg.role {
                ChatRole::User => "User",
                ChatRole::Assistant => "Assistant",
            };
            prompt.push_str(&format!("{}: {}\n\n", role_label, msg.content));
        }
        prompt.push_str(&format!("User: {}", new_message));
        prompt
    }

    fn cancel_stream(&mut self) {
        if let Some(handle) = &self.provider_handle {
            let _ = handle.request_tx.send(ProviderRequest::Cancel);
        }
        self.stream_state = StreamState::Idle;
    }

    /// Handle Escape for cancel. Text input is handled by TextEdit in render_input.
    pub fn handle_input(&mut self, ctx: &egui::Context) -> ModuleAction {
        ctx.input(|input| {
            if input.key_pressed(egui::Key::Escape) && self.stream_state == StreamState::Streaming {
                self.cancel_stream();
            }
        });
        ModuleAction::None
    }

    pub fn render(&mut self, ui: &mut egui::Ui, theme: &Theme, focused: bool) -> ModuleAction {
        self.poll_provider();

        self.render_header(ui, theme, focused);
        ui.add_space(4.0);

        // Separator below header
        let rect =
            egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), 1.0));
        ui.painter()
            .rect_filled(rect, 0.0, theme.line_number.gamma_multiply(0.3));
        ui.allocate_space(egui::vec2(ui.available_width(), 1.0));
        ui.add_space(4.0);

        // Message area (scrollable)
        let available = ui.available_size();
        let input_height = 60.0;
        let messages_height = (available.y - input_height).max(40.0);

        egui::Frame::NONE.show(ui, |ui| {
            ui.set_min_height(messages_height);
            ui.set_max_height(messages_height);

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .stick_to_bottom(self.auto_scroll)
                .show(ui, |ui| {
                    self.render_messages(ui, theme);
                });
        });

        // Status line above input
        let status = if !self.status_text.is_empty() {
            Some(&self.status_text as &str)
        } else if self.stream_state == StreamState::Streaming {
            Some("streaming...")
        } else {
            None
        };
        if let Some(status) = status {
            ui.label(
                egui::RichText::new(status)
                    .color(theme.syntax.string)
                    .size(10.0)
                    .italics(),
            );
            ui.add_space(2.0);
        }

        // Separator
        let rect =
            egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), 1.0));
        ui.painter()
            .rect_filled(rect, 0.0, theme.line_number.gamma_multiply(0.3));
        ui.allocate_space(egui::vec2(ui.available_width(), 1.0));

        self.render_input(ui, theme, focused);

        ModuleAction::None
    }

    fn render_header(&self, ui: &mut egui::Ui, theme: &Theme, _focused: bool) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("AI CHAT")
                    .color(theme.syntax.keyword)
                    .size(11.0)
                    .strong(),
            );

            ui.add_space(8.0);

            ui.label(
                egui::RichText::new(format!("[{}]", self.config.active_provider))
                    .color(theme.line_number)
                    .size(10.0),
            );
        });
    }

    fn render_messages(&self, ui: &mut egui::Ui, theme: &Theme) {
        if self.messages.is_empty() && self.current_response.is_empty() {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Type a message and press Enter")
                        .color(theme.line_number)
                        .size(11.0)
                        .italics(),
                );
            });
            return;
        }

        for msg in &self.messages {
            self.render_single_message(ui, theme, msg);
            ui.add_space(8.0);
        }

        if !self.current_response.is_empty() {
            let streaming_msg = ChatMessage {
                role: ChatRole::Assistant,
                content: self.current_response.clone(),
            };
            self.render_single_message(ui, theme, &streaming_msg);
            ui.add_space(8.0);
        }

        if let StreamState::Error(ref err) = self.stream_state {
            ui.label(
                egui::RichText::new(format!("Error: {}", err))
                    .color(theme.syntax.keyword)
                    .size(11.0),
            );
        }
    }

    fn render_single_message(&self, ui: &mut egui::Ui, theme: &Theme, msg: &ChatMessage) {
        let (role_label, role_color) = match msg.role {
            ChatRole::User => ("You:", theme.syntax.keyword),
            ChatRole::Assistant => ("Claude:", theme.syntax.function),
        };

        ui.label(
            egui::RichText::new(role_label)
                .color(role_color)
                .size(11.0)
                .strong(),
        );

        let content_color = theme.foreground;
        for line in msg.content.lines() {
            if line.starts_with("```") {
                ui.label(
                    egui::RichText::new(line)
                        .color(theme.line_number)
                        .size(11.0)
                        .monospace(),
                );
            } else if line.starts_with("  ") || line.starts_with('\t') {
                ui.label(
                    egui::RichText::new(line)
                        .color(theme.syntax.string)
                        .size(11.0)
                        .monospace(),
                );
            } else {
                ui.label(egui::RichText::new(line).color(content_color).size(11.0));
            }
        }
    }

    fn render_input(&mut self, ui: &mut egui::Ui, theme: &Theme, focused: bool) {
        ui.add_space(4.0);

        let max_input_height = 80.0; // ~5 lines before scrolling

        let mut wants_send = false;

        egui::ScrollArea::vertical()
            .max_height(max_input_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let text_edit = egui::TextEdit::multiline(&mut self.input_buffer)
                    .hint_text("type your message, Enter to send")
                    .desired_rows(1)
                    .desired_width(ui.available_width())
                    .font(egui::TextStyle::Body)
                    .text_color(theme.foreground)
                    .frame(false)
                    .lock_focus(true);

                let response = ui.add(text_edit);

                if focused && !response.has_focus() {
                    response.request_focus();
                }

                // Enter sends, Shift+Enter inserts newline
                if response.has_focus() {
                    wants_send =
                        ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
                }
            });

        if wants_send {
            // Remove trailing newline the TextEdit just inserted
            while self.input_buffer.ends_with('\n') {
                self.input_buffer.pop();
            }
            if !self.input_buffer.trim().is_empty() {
                self.send_message(ui.ctx());
            }
        }

        if self.session_token_count > 0 {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::BOTTOM), |ui| {
                let mut parts = Vec::new();
                if self.token_count > 0 {
                    parts.push(format!("tokens: {}", format_number(self.token_count)));
                }
                parts.push(format!(
                    "session: {}",
                    format_number(self.session_token_count)
                ));
                if self.context_window > 0 {
                    let pct =
                        (self.session_token_count as f64 / self.context_window as f64) * 100.0;
                    parts.push(format!("{:.0}%", pct));
                }
                ui.label(
                    egui::RichText::new(parts.join(" | "))
                        .color(theme.line_number)
                        .size(9.0),
                );
            });
        }
    }

    pub fn shutdown(&mut self) {
        if let Some(handle) = self.provider_handle.take() {
            let _ = handle.request_tx.send(ProviderRequest::Shutdown);
        }
    }
}

impl Drop for AiChatModule {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn format_number(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
