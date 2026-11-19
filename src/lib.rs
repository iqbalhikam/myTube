mod app;
mod brave;
mod catalog;
mod model;
mod sponsor;
mod store;

use std::path::PathBuf;
use std::sync::OnceLock;

pub(crate) static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn run_desktop() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([430.0, 900.0]).with_title("NoIklan"),
        ..Default::default()
    };
    eframe::run_native("NoIklan", options, Box::new(|cc| Box::new(app::NoIklanApp::new(cc))))
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: android_activity::AndroidApp) {
    if let Some(p) = app.internal_data_path() {
        let _ = DATA_DIR.set(p);
    }
    let options = eframe::NativeOptions {
        android_app: Some(app),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native("NoIklan", options, Box::new(|cc| Box::new(app::NoIklanApp::new(cc)))).unwrap();
}
