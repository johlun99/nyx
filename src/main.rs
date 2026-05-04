// src/main.rs
mod app;
mod buffer;
mod editor;
mod renderer;
mod vim;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    let file_path = std::env::args().nth(1);

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("Nyx"),
        ..Default::default()
    };
    eframe::run_native(
        "Nyx",
        options,
        Box::new(move |_cc| Ok(Box::new(app::NyxApp::new(file_path)))),
    )
}
