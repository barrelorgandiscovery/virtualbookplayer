//! Named interface themes.
//!
//! Each theme is one egui [`Visuals`] plus a few colors the widgets paint
//! themselves (stars, folders, the title of the file that is playing).
//! Add a theme by extending [`ThemeId`] and filling one [`Theme`] in
//! [`ThemeId::theme`]. Preferences lists [`ThemeId::ALL`].

use egui::style::WidgetVisuals;
use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, Visuals};
use serde::{Deserialize, Serialize};

/// Bold face installed in [`super::VirtualBookApp::setup_fonts`].
pub(crate) const BOLD_FAMILY: &str = "rubik_bold";

pub(crate) fn bold_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(BOLD_FAMILY.into()))
}

/// Persisted theme choice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ThemeId {
    /// Warm dark booth. Replaces the stock egui dark theme.
    #[default]
    Night,
    /// Logo light theme: cream, amber pipes, dark-blue keys.
    Console,
    /// Fairground night: indigo, bulb gold, carnival red.
    Marquee,
    /// Music-roll paper in daylight.
    Parchment,
    /// Cool rehearsal room, oxidized brass.
    Loft,
}

impl ThemeId {
    pub(crate) const ALL: [ThemeId; 5] = [
        ThemeId::Night,
        ThemeId::Console,
        ThemeId::Marquee,
        ThemeId::Parchment,
        ThemeId::Loft,
    ];

    pub(crate) fn theme(self) -> Theme {
        match self {
            ThemeId::Night => night(),
            ThemeId::Console => console(),
            ThemeId::Marquee => marquee(),
            ThemeId::Parchment => parchment(),
            ThemeId::Loft => loft(),
        }
    }
}

/// Colors that are not widget chrome: they are drawn as text and badges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ThemeTokens {
    pub star: Color32,
    pub folder: Color32,
    pub now_playing_bg: Color32,
    pub now_playing_fg: Color32,
}

pub(crate) struct Theme {
    pub visuals: Visuals,
    pub tokens: ThemeTokens,
}

/// Older saves only stored `islight`. When `theme_set` is still false, a
/// light preference becomes [`ThemeId::Console`]; dark stays the default.
pub(crate) fn resolve_saved_theme(
    theme: ThemeId,
    theme_set: bool,
    legacy_islight: bool,
) -> ThemeId {
    if !theme_set && legacy_islight {
        ThemeId::Console
    } else {
        theme
    }
}

const THEME_ID_KEY: &str = "vbp.theme.id";
const THEME_TOKENS_KEY: &str = "vbp.theme.tokens";

/// Install `id` on the context. Does nothing when that theme is already current,
/// so a frame does not rebuild the style unless the choice changed.
pub(crate) fn apply(ctx: &egui::Context, id: ThemeId) {
    let current = ctx.data(|data| data.get_temp::<ThemeId>(egui::Id::unique(THEME_ID_KEY)));
    if current == Some(id) {
        return;
    }

    let theme = id.theme();
    // egui keeps a dark style and a light style. Paint both, then select the
    // one that matches this scheme, so a menu does not fall back to stock colors.
    for slot in [egui::Theme::Dark, egui::Theme::Light] {
        ctx.set_visuals_of(slot, theme.visuals.clone());
        ctx.style_mut_of(slot, |style| {
            let fill = style.visuals.window_fill;
            style.visuals.window_fill =
                Color32::from_rgba_premultiplied(fill.r(), fill.g(), fill.b(), 255);
            style.visuals.window_corner_radius = CornerRadius::ZERO;
        });
    }
    ctx.set_theme(if theme.visuals.dark_mode {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });

    ctx.data_mut(|data| {
        data.insert_temp(egui::Id::unique(THEME_ID_KEY), id);
        data.insert_temp(egui::Id::unique(THEME_TOKENS_KEY), theme.tokens);
    });
}

pub(crate) fn tokens(ctx: &egui::Context) -> ThemeTokens {
    ctx.data(|data| data.get_temp(egui::Id::unique(THEME_TOKENS_KEY)))
        .unwrap_or_else(|| ThemeId::default().theme().tokens)
}

struct Ink {
    dark: bool,
    /// Window, panel, and plain labels.
    bg: Color32,
    /// Text edits and the deepest recess.
    deep: Color32,
    /// Badges and faint strips.
    faint: Color32,
    /// Body text.
    text: Color32,
    /// Button label, a step stronger than body text.
    text_strong: Color32,
    /// Rules, window stroke, separators.
    line: Color32,
    button: Color32,
    hover: Color32,
    hover_line: Color32,
    active: Color32,
    active_text: Color32,
    open: Color32,
    selection: Color32,
    selection_text: Color32,
    link: Color32,
    warn: Color32,
    star: Color32,
    folder: Color32,
    now_playing_bg: Color32,
    now_playing_fg: Color32,
}

fn widget(fill: Color32, stroke: Color32, text: Color32, radius: u8, width: f32) -> WidgetVisuals {
    WidgetVisuals {
        weak_bg_fill: fill,
        bg_fill: fill,
        bg_stroke: Stroke::new(width, stroke),
        fg_stroke: Stroke::new(width, text),
        corner_radius: CornerRadius::same(radius),
        expansion: 0.0,
    }
}

fn paint(ink: Ink) -> Theme {
    let mut visuals = if ink.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    visuals.dark_mode = ink.dark;
    visuals.window_fill = ink.bg;
    visuals.panel_fill = ink.bg;
    visuals.window_stroke = Stroke::new(1.0, ink.line);
    visuals.extreme_bg_color = ink.deep;
    visuals.faint_bg_color = ink.faint;
    visuals.code_bg_color = ink.faint;
    visuals.hyperlink_color = ink.link;
    visuals.warn_fg_color = ink.warn;
    visuals.error_fg_color = Color32::from_rgb(196, 48, 42);
    visuals.text_cursor.stroke = Stroke::new(2.0, ink.text_strong);

    visuals.widgets.noninteractive = widget(ink.bg, ink.line, ink.text, 4, 1.0);
    visuals.widgets.inactive = widget(ink.button, ink.line, ink.text_strong, 6, 1.0);
    visuals.widgets.hovered = widget(ink.hover, ink.hover_line, ink.text_strong, 6, 1.5);
    visuals.widgets.active = widget(ink.active, ink.line, ink.active_text, 6, 2.0);
    visuals.widgets.open = widget(ink.open, ink.hover_line, ink.text, 6, 1.0);

    visuals.selection.bg_fill = ink.selection;
    visuals.selection.stroke = Stroke::new(1.0, ink.selection_text);

    Theme {
        visuals,
        tokens: ThemeTokens {
            star: ink.star,
            folder: ink.folder,
            now_playing_bg: ink.now_playing_bg,
            now_playing_fg: ink.now_playing_fg,
        },
    }
}

fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

fn night() -> Theme {
    paint(Ink {
        dark: true,
        bg: rgb(26, 22, 20),
        deep: rgb(16, 13, 12),
        faint: rgb(40, 34, 30),
        text: rgb(232, 214, 190),
        text_strong: rgb(248, 236, 214),
        line: rgb(92, 70, 52),
        button: rgb(58, 46, 36),
        hover: rgb(96, 70, 42),
        hover_line: rgb(212, 160, 72),
        active: rgb(140, 72, 42),
        active_text: rgb(255, 244, 230),
        open: rgb(48, 38, 30),
        selection: rgb(120, 58, 40),
        selection_text: rgb(255, 220, 180),
        link: rgb(232, 177, 90),
        warn: rgb(255, 160, 64),
        star: rgb(240, 193, 74),
        folder: rgb(126, 182, 217),
        now_playing_bg: rgb(88, 42, 30),
        now_playing_fg: rgb(255, 244, 230),
    })
}

/// The previous custom light theme, kept as its own named scheme.
fn console() -> Theme {
    let warm_beige = rgb(250, 245, 235);
    let golden_amber = rgb(255, 200, 100);
    let golden_amber_light = rgb(255, 220, 150);
    let reddish_brown = rgb(180, 100, 60);
    let reddish_brown_light = rgb(200, 120, 80);
    let dark_brown = rgb(80, 50, 30);
    let dark_blue = rgb(30, 50, 90);

    paint(Ink {
        dark: false,
        bg: warm_beige,
        deep: rgb(255, 250, 240),
        faint: rgb(250, 240, 225),
        text: dark_blue,
        text_strong: dark_blue,
        line: dark_brown,
        button: golden_amber_light,
        hover: golden_amber,
        hover_line: reddish_brown,
        active: reddish_brown_light,
        active_text: Color32::WHITE,
        open: golden_amber_light,
        selection: dark_blue,
        selection_text: warm_beige,
        link: rgb(50, 70, 120),
        warn: rgb(176, 88, 16),
        star: rgb(146, 96, 0),
        folder: dark_blue,
        now_playing_bg: dark_blue,
        now_playing_fg: Color32::WHITE,
    })
}

fn marquee() -> Theme {
    paint(Ink {
        dark: true,
        bg: rgb(16, 12, 28),
        deep: rgb(10, 8, 18),
        faint: rgb(32, 24, 48),
        text: rgb(255, 236, 214),
        text_strong: rgb(255, 246, 232),
        line: rgb(92, 64, 96),
        button: rgb(46, 32, 68),
        hover: rgb(78, 40, 72),
        hover_line: rgb(255, 209, 102),
        active: rgb(168, 42, 58),
        active_text: rgb(255, 246, 230),
        open: rgb(36, 24, 52),
        selection: rgb(168, 42, 58),
        selection_text: rgb(255, 230, 190),
        link: rgb(255, 209, 102),
        warn: rgb(255, 176, 64),
        star: rgb(255, 209, 102),
        folder: rgb(150, 186, 255),
        now_playing_bg: rgb(120, 24, 40),
        now_playing_fg: rgb(255, 246, 220),
    })
}

fn parchment() -> Theme {
    paint(Ink {
        dark: false,
        bg: rgb(244, 232, 208),
        deep: rgb(252, 246, 232),
        faint: rgb(232, 214, 184),
        text: rgb(44, 36, 22),
        text_strong: rgb(36, 28, 16),
        line: rgb(140, 108, 68),
        button: rgb(228, 208, 172),
        hover: rgb(214, 184, 132),
        hover_line: rgb(139, 58, 47),
        active: rgb(139, 58, 47),
        active_text: rgb(252, 244, 230),
        open: rgb(236, 218, 186),
        selection: rgb(139, 58, 47),
        selection_text: rgb(252, 244, 230),
        link: rgb(110, 42, 36),
        warn: rgb(160, 72, 16),
        star: rgb(140, 88, 16),
        folder: rgb(46, 90, 86),
        now_playing_bg: rgb(90, 36, 30),
        now_playing_fg: rgb(252, 244, 230),
    })
}

fn loft() -> Theme {
    paint(Ink {
        dark: true,
        bg: rgb(20, 24, 32),
        deep: rgb(12, 16, 22),
        faint: rgb(30, 38, 48),
        text: rgb(214, 222, 230),
        text_strong: rgb(234, 240, 244),
        line: rgb(60, 78, 96),
        button: rgb(36, 48, 62),
        hover: rgb(42, 72, 78),
        hover_line: rgb(110, 200, 196),
        active: rgb(32, 96, 100),
        active_text: rgb(236, 248, 246),
        open: rgb(28, 36, 48),
        selection: rgb(24, 78, 92),
        selection_text: rgb(220, 245, 242),
        link: rgb(126, 210, 204),
        warn: rgb(232, 168, 72),
        star: rgb(232, 196, 110),
        folder: rgb(110, 200, 196),
        now_playing_bg: rgb(20, 64, 78),
        now_playing_fg: rgb(236, 248, 246),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_preference_becomes_console_until_a_theme_is_chosen() {
        assert_eq!(
            resolve_saved_theme(ThemeId::Night, false, true),
            ThemeId::Console
        );
        assert_eq!(
            resolve_saved_theme(ThemeId::Night, false, false),
            ThemeId::Night
        );
        assert_eq!(
            resolve_saved_theme(ThemeId::Loft, true, true),
            ThemeId::Loft
        );
    }

    #[test]
    fn every_theme_keeps_title_and_marks_readable() {
        for id in ThemeId::ALL {
            let theme = id.theme();
            let tokens = theme.tokens;
            let bg = theme.visuals.panel_fill;
            assert!(
                apart(tokens.now_playing_fg, tokens.now_playing_bg),
                "{id:?} title"
            );
            assert!(apart(tokens.star, bg), "{id:?} star");
            assert!(apart(tokens.folder, bg), "{id:?} folder");
            assert!(
                apart(theme.visuals.widgets.noninteractive.fg_stroke.color, bg),
                "{id:?} text"
            );
        }
    }

    fn apart(a: Color32, b: Color32) -> bool {
        let lum =
            |c: Color32| 0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32;
        (lum(a) - lum(b)).abs() > 80.0
    }
}
