use crate::modules::ModuleAction;
use crate::renderer::Theme;
use eframe::egui;
use egui::Vec2;
use egui_term::{
    BackendCommand, BackendSettings, ColorPalette, PtyEvent, TerminalBackend, TerminalTheme,
    TerminalView,
};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

pub struct TerminalModule {
    backend: Option<TerminalBackend>,
    pty_receiver: Option<Receiver<(u64, PtyEvent)>>,
    theme: TerminalTheme,
    working_directory: PathBuf,
    exited: bool,
    next_id: u64,
}

impl TerminalModule {
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            backend: None,
            pty_receiver: None,
            theme: Self::default_theme(),
            working_directory,
            exited: false,
            next_id: 0,
        }
    }

    fn ensure_initialized(&mut self, ctx: &egui::Context) {
        if self.backend.is_some() {
            return;
        }
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let (sender, receiver) = std::sync::mpsc::channel();
        let id = self.next_id;
        self.next_id += 1;
        match TerminalBackend::new(
            id,
            ctx.clone(),
            sender,
            BackendSettings {
                shell,
                working_directory: Some(self.working_directory.clone()),
                ..Default::default()
            },
        ) {
            Ok(backend) => {
                self.backend = Some(backend);
                self.pty_receiver = Some(receiver);
                self.exited = false;
            }
            Err(e) => {
                tracing::error!("Failed to create terminal backend: {e}");
            }
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui, _theme: &Theme, focused: bool) -> ModuleAction {
        self.ensure_initialized(ui.ctx());

        // Drain PTY events, check for exit
        if let Some(ref receiver) = self.pty_receiver {
            while let Ok((_, event)) = receiver.try_recv() {
                if matches!(event, PtyEvent::Exit) {
                    self.exited = true;
                    self.backend = None;
                    self.pty_receiver = None;
                    break;
                }
            }
        }

        if self.exited {
            ui.vertical_centered(|ui| {
                ui.add_space(20.0);
                ui.label(
                    egui::RichText::new("Shell exited. Press Enter to restart.")
                        .color(egui::Color32::from_rgb(0xa6, 0xad, 0xc8))
                        .size(13.0),
                );
            });
            if focused && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.exited = false;
                // Lazy re-init will happen next frame
            }
            return ModuleAction::None;
        }

        if let Some(ref mut backend) = self.backend {
            // egui_term requires both has_focus() AND contains_pointer() for input.
            // When the user focuses the terminal via keyboard (Ctrl+J) but their mouse
            // is elsewhere, we forward keyboard events manually.
            let terminal_rect = egui::Rect::from_min_size(
                ui.cursor().min,
                Vec2::new(ui.available_width(), ui.available_height()),
            );
            let pointer_over_terminal = ui.ctx().input(|i| {
                i.pointer
                    .hover_pos()
                    .is_some_and(|pos| terminal_rect.contains(pos))
            });
            if focused && !pointer_over_terminal {
                Self::forward_keyboard_input(ui.ctx(), backend);
            }

            let terminal = TerminalView::new(ui, backend)
                .set_focus(focused)
                .set_theme(self.theme.clone())
                .set_size(Vec2::new(ui.available_width(), ui.available_height()));
            ui.add(terminal);
        }

        ModuleAction::None
    }

    /// Forward keyboard events to the PTY backend when egui_term's
    /// built-in input handling is bypassed (pointer not over widget).
    fn forward_keyboard_input(ctx: &egui::Context, backend: &mut TerminalBackend) {
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            match event {
                egui::Event::Text(text) => {
                    backend.process_command(BackendCommand::Write(text.into_bytes()));
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    if let Some(bytes) = Self::key_to_bytes(key, modifiers) {
                        backend.process_command(BackendCommand::Write(bytes));
                    }
                }
                egui::Event::Paste(text) => {
                    backend.process_command(BackendCommand::Write(text.into_bytes()));
                }
                _ => {}
            }
        }
    }

    /// Map special keys and ctrl combos to terminal escape sequences.
    fn key_to_bytes(key: egui::Key, modifiers: egui::Modifiers) -> Option<Vec<u8>> {
        use egui::Key;

        if modifiers.ctrl {
            let ctrl_byte: Option<u8> = match key {
                Key::A => Some(0x01),
                Key::B => Some(0x02),
                Key::C => Some(0x03),
                Key::D => Some(0x04),
                Key::E => Some(0x05),
                Key::F => Some(0x06),
                Key::G => Some(0x07),
                Key::K => Some(0x0b),
                Key::N => Some(0x0e),
                Key::O => Some(0x0f),
                Key::P => Some(0x10),
                Key::Q => Some(0x11),
                Key::R => Some(0x12),
                Key::S => Some(0x13),
                Key::T => Some(0x14),
                Key::U => Some(0x15),
                Key::V => Some(0x16),
                Key::W => Some(0x17),
                Key::X => Some(0x18),
                Key::Y => Some(0x19),
                Key::Z => Some(0x1a),
                _ => None,
            };
            return ctrl_byte.map(|b| vec![b]);
        }

        match key {
            Key::Enter => Some(b"\r".to_vec()),
            Key::Backspace => Some(vec![0x7f]),
            Key::Tab => Some(b"\t".to_vec()),
            Key::Escape => Some(b"\x1b".to_vec()),
            Key::ArrowUp => Some(b"\x1b[A".to_vec()),
            Key::ArrowDown => Some(b"\x1b[B".to_vec()),
            Key::ArrowRight => Some(b"\x1b[C".to_vec()),
            Key::ArrowLeft => Some(b"\x1b[D".to_vec()),
            Key::Home => Some(b"\x1b[H".to_vec()),
            Key::End => Some(b"\x1b[F".to_vec()),
            Key::Delete => Some(b"\x1b[3~".to_vec()),
            Key::PageUp => Some(b"\x1b[5~".to_vec()),
            Key::PageDown => Some(b"\x1b[6~".to_vec()),
            Key::Insert => Some(b"\x1b[2~".to_vec()),
            _ => None, // Regular characters handled via Event::Text
        }
    }

    fn default_theme() -> TerminalTheme {
        // Catppuccin Mocha palette matching Nyx's default dark theme
        TerminalTheme::new(Box::new(ColorPalette {
            foreground: String::from("#CDD6F4"),
            background: String::from("#1E1E2E"),
            black: String::from("#45475A"),
            red: String::from("#F38BA8"),
            green: String::from("#A6E3A1"),
            yellow: String::from("#F9E2AF"),
            blue: String::from("#89B4FA"),
            magenta: String::from("#F5C2E7"),
            cyan: String::from("#94E2D5"),
            white: String::from("#BAC2DE"),
            bright_black: String::from("#585B70"),
            bright_red: String::from("#F38BA8"),
            bright_green: String::from("#A6E3A1"),
            bright_yellow: String::from("#F9E2AF"),
            bright_blue: String::from("#89B4FA"),
            bright_magenta: String::from("#F5C2E7"),
            bright_cyan: String::from("#94E2D5"),
            bright_white: String::from("#A6ADC8"),
            ..Default::default()
        }))
    }
}
