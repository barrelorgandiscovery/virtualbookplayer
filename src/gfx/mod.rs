//! GPU helpers for the virtual-book canvas (wgpu PaintCallback).
//!
//! Only the book viewport uses this path; the rest of the UI stays on egui.

pub mod book_fx;
pub mod effect;

pub use book_fx::{init_book_fx_gpu, BookFxState};
pub use effect::BookEffectKind;
