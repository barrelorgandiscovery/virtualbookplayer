//! VirtualBookPlayer structure, exposing the several subsystem and functions
//!
#![warn(clippy::all, rust_2018_idioms)]

mod app;
pub mod appplayer;
pub mod file_store;
pub mod playlist;
pub mod virtualbookcomponent;

pub use app::VirtualBookApp;
