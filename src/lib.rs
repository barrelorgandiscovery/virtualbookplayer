//! VirtualBookPlayer structure, exposing the several subsystem and functions
//!
#![warn(clippy::all, rust_2018_idioms)]

mod app;
pub mod appplayer;
pub mod file_store;
pub mod playlist;
pub mod virtualbookcomponent;

use std::time::Duration;

pub use app::VirtualBookApp;

pub fn duration_to_mm_ss(duration: &Duration) -> String {
    let seconds = duration.as_secs() % 60;
    let minutes = (duration.as_secs() / 60) % 60;
    format!("{:0>2}:{:0>2}", minutes, seconds)
}
