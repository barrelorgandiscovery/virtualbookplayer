#![warn(clippy::all, rust_2018_idioms)]

mod app;
mod appplayer;
mod file_store;
mod playlist;
mod virtualbookcomponent;

pub use app::VirtualBookApp;
