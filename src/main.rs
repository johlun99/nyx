mod app;
mod buffer;
mod renderer;
mod vim;

fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("Nyx"),
        ..Default::default()
    };
    eframe::run_native("Nyx", options, Box::new(|_cc| Ok(Box::new(app::NyxApp::new()))))
}
