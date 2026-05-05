// src/main.rs
mod app;
mod buffer;
mod config;
mod editor;
mod file_io;
mod renderer;
mod vim;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    let file_path = std::env::args().nth(1);
    let config = crate::config::NyxConfig::load_or_create(
        &crate::config::NyxConfig::config_path(),
    );

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("Nyx"),
        ..Default::default()
    };
    eframe::run_native(
        "Nyx",
        options,
        Box::new(move |_cc| Ok(Box::new(app::NyxApp::new(file_path, config)))),
    )
}
