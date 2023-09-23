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
}

fn main() -> eframe::Result<()> {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    log::debug!("commandline arguments : {:?}", args);

    let native_options = eframe::NativeOptions::default();
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
