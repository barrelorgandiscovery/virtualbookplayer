//! Virtual book player application,
//! this is a graphical application to player midi and output to a midi device
//!
//! The application has additional command line arguments :
//!
//!
//! use@alexa:~/projets/2023-05-01_test_egui_draw/virtualbookplayer$ target/debug/virtualbookplayerapp --help
//! ```
//! ==========================================================================
//! Virtual Book Player 0.0.56 (Patrice Freydiere - BarrelOrganDiscovery)
//! https://www.barrel-organ-discovery.org
//!   date: dim. 15 oct. 2023 08:57:03 CEST
//!   build: 10adc0cf4a8f004c0f7b969ca69a5b6be5a96de5
//! ==========================================================================
//! Usage: virtualbookplayerapp [OPTIONS]
//!
//! Options:
//!   -r, --reset-preferences          reset the gui preferences
//!   -l, --lang-code <LANG_CODE>      choose and force the current language, if not passed the program use environment variables provided by the OS to try to detect the user language
//!   -f, --full-screen <FULL_SCREEN>  launch the application with no window decoration (best for a fullscreen experience) [possible values: true, false]
//!   -h, --help                       Print help
//!   -V, --version                    Print version
//!
//! ```
//!
//! *reset-references* option permit to reset the gui saved elements (windows position, selected folder, and other preferences)
//! *lang-code* define the gui language (currently two langage are provided french and english, this can be extended in providing label translation in i18n file)

#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use clap::Parser;
use egui::{IconData, ViewportBuilder};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// reset the gui preferences
    #[arg(short, long)]
    reset_preferences: bool,

    /// choose and force the current language, if not passed the program use environment variables provided by the OS to try to detect the user language
    #[arg(short, long)]
    lang_code: Option<String>,

    /// launch the application with no window decoration (best for a fullscreen experience)
    #[arg(short, long)]
    full_screen: Option<bool>,
}

pub(crate) fn load_icon() -> IconData {
    let (icon_rgba, icon_width, icon_height) = {
        let icon = include_bytes!("../assets/logo_color.png");
        let image = image::load_from_memory(icon)
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };

    IconData {
        rgba: icon_rgba,
        width: icon_width,
        height: icon_height,
    }
}

fn main() -> eframe::Result<()> {
    // // Log to stdout (if you run with `RUST_LOG=debug`).
    // tracing_subscriber::fmt::init();

    env_logger::try_init_from_env(env_logger::Env::default().filter_or("RUST_LOG", "error")).ok();

    println!("==========================================================================");
    let version = env!("CARGO_PKG_VERSION");
    println!(
        "Virtual Book Player {} (Patrice Freydiere - BarrelOrganDiscovery)",
        version
    );
    println!("https://www.barrel-organ-discovery.org");

    println!("  date: {}", std::env!("BUILD_DATE"));
    println!("  build: {}", std::env!("GIT_HASH"));
    println!("==========================================================================");

    #[cfg(feature = "profiling")]
    tracy_client::Client::start();

    let args = Args::parse();
    log::debug!("commandline arguments : {:?}", args);

    let mut viewport_build = ViewportBuilder::default();
    if let Some(fs) = args.full_screen {
        viewport_build = viewport_build.with_fullscreen(fs).with_decorations(false);
    }

    viewport_build = viewport_build.with_icon(load_icon());

    let native_options = eframe::NativeOptions {
        viewport: viewport_build,
        ..Default::default()
    };

    const VERSION_STRING: &str = concat!("VirtualBookPlayer - v", env!("CARGO_PKG_VERSION"));

    eframe::run_native(
        VERSION_STRING,
        native_options,
        Box::new(move |cc| {
            Box::new(virtualbookplayer::VirtualBookApp::new(
                cc,
                args.reset_preferences,
                args.lang_code,
            ))
        }),
    )
}
