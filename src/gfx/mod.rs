//! GPU helpers for the virtual-book canvas (wgpu PaintCallback).
//!
//! Only the book viewport uses this path; the rest of the UI stays on egui.

pub mod book_fx;

pub use book_fx::{BookFxState, init_book_fx_gpu};
