//! Selectable visual effects for the virtual-book canvas.
//!
//! New effects are added as variants of [`BookEffectKind`] and wired in
//! [`crate::gfx::book_fx`]. Preferences persist the chosen kind.

use serde::{Deserialize, Serialize};

/// GPU particle sprite style selected by the active effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticleDrawStyle {
    /// Soft radial glow (additive) — Note fire, Neon lane sparks, Arcade burst.
    Soft,
    /// Faceted gold star sprite (alpha) — Sparkle.
    Star,
    /// Expanding ring (additive) — Ripple.
    Ring,
}

/// User-selectable book visual effect (preferences + runtime).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BookEffectKind {
    /// Plain notes only — no particles or playhead glow.
    #[default]
    None,
    /// Warm sparks on the playhead that ride left with the carton after trigger.
    NoteFire,
    /// Faceted gold stars that fall under gravity; intensity follows note length.
    Sparkle,
    /// Piano-viz style expanding ripples from each note hit.
    Ripple,
    /// Rhythm-game neon beams on the playhead while notes are held.
    NeonLane,
    /// Arcade confetti burst — multicolor shards flying outward.
    ArcadeBurst,
}

impl BookEffectKind {
    /// All effects exposed in preferences (order = UI order).
    pub const fn all() -> &'static [Self] {
        &[
            Self::None,
            Self::NoteFire,
            Self::Sparkle,
            Self::Ripple,
            Self::NeonLane,
            Self::ArcadeBurst,
        ]
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::NoteFire => "note_fire",
            Self::Sparkle => "sparkle",
            Self::Ripple => "ripple",
            Self::NeonLane => "neon_lane",
            Self::ArcadeBurst => "arcade_burst",
        }
    }

    /// Whether this effect needs the wgpu book path (notes shader + particles).
    pub const fn uses_gpu_fx(self) -> bool {
        !matches!(self, Self::None)
    }

    /// Whether particles / playhead glow should run.
    pub const fn emits_particles(self) -> bool {
        !matches!(self, Self::None)
    }

    pub const fn particle_draw_style(self) -> ParticleDrawStyle {
        match self {
            Self::Sparkle => ParticleDrawStyle::Star,
            Self::Ripple => ParticleDrawStyle::Ring,
            _ => ParticleDrawStyle::Soft,
        }
    }
}
