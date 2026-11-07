//! Selectable visual effects for the virtual-book canvas.
//!
//! New effects are added as variants of [`BookEffectKind`] and wired in
//! [`crate::gfx::book_fx`]. Preferences persist the chosen kind.

use serde::{Deserialize, Serialize};

/// User-selectable book visual effect (preferences + runtime).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BookEffectKind {
    /// Plain notes only — no particles or playhead glow.
    #[default]
    None,
    /// Warm sparks on the playhead that ride left with the carton after trigger.
    NoteFire,
    /// Cool blue sparkles that fall under gravity; intensity follows note length.
    Sparkle,
}

impl BookEffectKind {
    /// All effects exposed in preferences (order = UI order).
    pub const fn all() -> &'static [Self] {
        &[Self::None, Self::NoteFire, Self::Sparkle]
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::NoteFire => "note_fire",
            Self::Sparkle => "sparkle",
        }
    }

    /// Whether this effect needs the wgpu book path (notes shader + particles).
    pub const fn uses_gpu_fx(self) -> bool {
        matches!(self, Self::NoteFire | Self::Sparkle)
    }

    /// Whether particles / playhead glow should run.
    pub const fn emits_particles(self) -> bool {
        matches!(self, Self::NoteFire | Self::Sparkle)
    }
}
