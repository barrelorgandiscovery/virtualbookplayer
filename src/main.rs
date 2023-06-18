#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release


#[cfg(target_os = "android")]
use winit::platform::android::activity::AndroidApp;

// When compiling natively:
#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result<()> {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "VirtualBook Player",
        native_options,
        Box::new(|cc| Box::new(virtualbookplayerapp::VirtualBookApp::new(cc))),
    )
}

#[cfg(target_os = "android")]
fn _main(mut options: NativeOptions) {
    options.renderer = Renderer::Wgpu;
    eframe::run_native(
        "VirtualBook Player",
        native_options,
        Box::new(|cc| Box::new(virtualbookplayerapp::VirtualBookApp::new(cc))),
    )
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: AndroidApp) {
    use winit::platform::android::EventLoopBuilderExtAndroid;

    android_logger::init_once(android_logger::Config::default().with_min_level(log::Level::Info));

    let mut options = NativeOptions::default();
    options.event_loop_builder = Some(Box::new(move |builder| {
        builder.with_android_app(app);
    }));
    _main(options);
}
