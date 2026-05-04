pub struct NyxApp;

impl NyxApp {
    pub fn new() -> Self {
        Self
    }
}

impl eframe::App for NyxApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        eframe::egui::CentralPanel::default().show(ctx, |ui| {
            ui.label("Nyx editor");
        });
    }
}
