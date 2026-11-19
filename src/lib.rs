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

/// Tinggi status bar dan navigation bar Android dalam piksel fisik.
#[cfg(target_os = "android")]
pub(crate) fn system_insets() -> Option<(f32, f32)> {
    static CACHE: OnceLock<(f32, f32)> = OnceLock::new();
    if let Some(v) = CACHE.get() {
        return Some(*v);
    }
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm() as *mut jni::sys::JavaVM) }.ok()?;
    let mut env = vm.attach_current_thread_permanently().ok()?;
    let activity = unsafe { jni::objects::JObject::from_raw(ctx.context() as jni::sys::jobject) };
    let window = env.call_method(&activity, "getWindow", "()Landroid/view/Window;", &[]).ok()?.l().ok()?;
    if window.is_null() {
        return None;
    }
    let decor = env.call_method(&window, "getDecorView", "()Landroid/view/View;", &[]).ok()?.l().ok()?;
    if decor.is_null() {
        return None;
    }
    let insets = env.call_method(&decor, "getRootWindowInsets", "()Landroid/view/WindowInsets;", &[]).ok()?.l().ok()?;
    if insets.is_null() {
        return None;
    }
    let top = env.call_method(&insets, "getSystemWindowInsetTop", "()I", &[]).ok()?.i().ok()? as f32;
    let bottom = env.call_method(&insets, "getSystemWindowInsetBottom", "()I", &[]).ok()?.i().ok()? as f32;
    if top <= 0.0 {
        return None;
    }
    let _ = CACHE.set((top, bottom.max(0.0)));
    CACHE.get().copied()
}

#[cfg(not(target_os = "android"))]
pub(crate) fn system_insets() -> Option<(f32, f32)> {
    None
}

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
