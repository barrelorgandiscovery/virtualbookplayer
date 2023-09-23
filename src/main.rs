#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Name of the person to greet
    #[arg(short, long)]
    reset_preferences: bool,

    #[arg(short, long)]
    lang_code: Option<String>,

    #[arg(short, long)]
    full_screen: Option<bool>,
}

fn main() -> eframe::Result<()> {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

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

    let args = Args::parse();
    log::debug!("commandline arguments : {:?}", args);

    let mut native_options = eframe::NativeOptions::default();
    if let Some(fs) = args.full_screen {
        native_options.fullscreen = fs;
    }
    eframe::run_native(
        "VirtualBook Player",
        native_options,
        Box::new(move |cc| {
            Box::new(virtualbookplayerapp::VirtualBookApp::new(
                cc,
                args.reset_preferences,
                args.lang_code,
            ))
        }),
    )
}
