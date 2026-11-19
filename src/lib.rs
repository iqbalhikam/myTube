mod app;
mod brave;
mod catalog;
mod icons;
mod model;
mod sponsor;
mod store;
mod youtube;

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
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("NoIklan")
            .with_max_level(log::LevelFilter::Debug),
    );
    std::panic::set_hook(Box::new(|info| log::error!("PANIC: {info}")));
    log::info!("android_main start");
    if let Some(p) = app.internal_data_path() {
        let _ = DATA_DIR.set(p);
    }
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        event_loop_builder: Some(Box::new(move |builder| {
            use winit::platform::android::EventLoopBuilderExtAndroid;
            builder.with_android_app(app);
        })),
        ..Default::default()
    };
    match eframe::run_native("NoIklan", options, Box::new(|cc| Box::new(app::NoIklanApp::new(cc)))) {
        Ok(()) => log::warn!("run_native returned Ok"),
        Err(e) => log::error!("run_native failed: {e}"),
    }
    log::warn!("android_main end");
}
