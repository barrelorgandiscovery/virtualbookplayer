//! GUI tests for [`VirtualBookApp`](super::VirtualBookApp).
//!
//! The window asks egui to repaint on every frame, so [`egui_kittest::Harness::run`]
//! never settles. Drive the UI with [`Harness::step`] instead.
//!
//! Image snapshots land in `tests/snapshots`. Refresh them with
//! `UPDATE_SNAPSHOTS=true cargo test --lib gui_kittest`.

use std::error::Error;
use std::sync::{mpsc, Arc};

use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use player::{FileInformations, FileInformationsConstructor, NotesInformations, Player};

use crate::gfx::BookEffectKind;

use super::theme::ThemeId;
use super::VirtualBookApp;

/// Player that never opens a MIDI port, so the playlist screen can be shown in tests.
struct IdlePlayer;

impl Player for IdlePlayer {
    fn start_play(
        &mut self,
        _filename: &std::path::PathBuf,
        _start_time: Option<f32>,
    ) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn stop(&mut self) {}

    fn is_paused(&self) -> bool {
        false
    }

    fn is_playing(&self) -> bool {
        false
    }

    fn current_play_time(&self) -> i64 {
        0
    }

    fn associated_notes(&self) -> Arc<NotesInformations> {
        Arc::new(NotesInformations::default())
    }

    fn create_information_getter(
        &self,
    ) -> Result<Box<dyn FileInformationsConstructor>, Box<dyn Error>> {
        Ok(Box::new(IdleInfo))
    }
}

struct IdleInfo;

impl FileInformationsConstructor for IdleInfo {
    fn compute(
        &mut self,
        _filename: &std::path::PathBuf,
    ) -> Result<FileInformations, Box<dyn Error>> {
        Ok(FileInformations { duration: None })
    }
}

fn attach_idle_player(app: &mut VirtualBookApp) {
    let (response_tx, response_rx) = mpsc::channel();
    let (command_tx, command_rx) = mpsc::channel();
    app.appplayer
        .player(Some((Box::new(IdlePlayer), response_rx, command_tx)));
    // Nothing in these tests sends player events or commands.
    drop(response_tx);
    drop(command_rx);
}

fn english_app(cc: &eframe::CreationContext<'_>, with_player: bool) -> VirtualBookApp {
    let mut app = VirtualBookApp::new(cc, true, Some("en".into()));
    // The preference slider starts at 1.5. A lower value is clamped as soon
    // as that menu is drawn, which would change zoom in the middle of a test.
    app.screen_zoom_factor = 1.5;
    app.slider_selected_zoom_factor = 1.5;
    if with_player {
        attach_idle_player(&mut app);
    }
    app
}

fn ui_harness(with_player: bool) -> Harness<'static, VirtualBookApp> {
    ui_harness_sized(with_player, egui::vec2(1100.0, 700.0))
}

fn ui_harness_sized(with_player: bool, size: egui::Vec2) -> Harness<'static, VirtualBookApp> {
    Harness::builder()
        .with_size(size)
        .with_os(egui::os::OperatingSystem::Nix)
        .build_eframe(|cc| english_app(cc, with_player))
}

fn snapshot_harness(with_player: bool) -> Harness<'static, VirtualBookApp> {
    Harness::builder()
        .with_size(egui::vec2(1100.0, 700.0))
        .with_pixels_per_point(1.0)
        .with_os(egui::os::OperatingSystem::Nix)
        .with_render_cursor(false)
        .wgpu()
        .build_eframe(|cc| english_app(cc, with_player))
}

/// Click a widget and paint two more frames.
///
/// A menu's first frame only measures itself, so its rows are not in the
/// accesskit tree until the frame after the click.
fn click(harness: &mut Harness<'static, VirtualBookApp>, label: &str) {
    harness.get_by_label_contains(label).click();
    harness.step();
    harness.step();
}

#[test]
fn startup_asks_for_a_midi_output() {
    let harness = ui_harness(false);

    harness.get_by_label_contains("Choisissez un périphérique");
    harness.get_by_label_contains("File");
    // The playlist is hidden until an output exists.
    assert!(harness
        .query_all_by_label_contains("Catalog")
        .next()
        .is_none());
}

#[test]
fn file_menu_preferences_change_theme_and_effect() {
    let mut harness = ui_harness_sized(false, egui::vec2(1100.0, 1400.0));

    click(&mut harness, "File");
    harness.get_by_label("Quit");
    harness.get_by_label_contains("Open folder");
    harness.get_by_label_contains("midi out interfaces");
    click(&mut harness, "Preferences");

    click(&mut harness, "Sparkle");
    assert_eq!(harness.state().book_effect, BookEffectKind::Sparkle);

    // A click inside the menu closes it, so open Preferences again.
    click(&mut harness, "File");
    click(&mut harness, "Preferences");
    assert_eq!(harness.state().theme, ThemeId::Night);
    click(&mut harness, "Theme");
    click(&mut harness, "Console");
    assert_eq!(harness.state().theme, ThemeId::Console);
    assert!(harness.state().theme_set);
}

#[test]
fn playlist_numpad_appends_the_typed_number() {
    let mut harness = ui_harness(true);

    harness.get_by_label_contains("Catalog");
    harness.get_by_label_contains("Playlist");
    harness.get_by_label("5").click();
    harness.step();
    harness.get_by_label("2").click();
    harness.step();

    assert_eq!(harness.state().current_typed_no, "52");
}

#[test]
fn history_pane_explains_that_a_folder_is_required() {
    let mut harness = ui_harness(true);

    click(&mut harness, "History");

    assert_eq!(
        harness
            .query_all_by_label("Open a folder to see the history")
            .count(),
        2
    );
    assert_eq!(
        harness.state().catalog_pane,
        super::screen_history::CatalogPane::History
    );
}

#[test]
fn hide_numpad_removes_the_digit_keys() {
    let mut harness = ui_harness(true);

    harness.get_by_label("7");

    click(&mut harness, "File");
    click(&mut harness, "Preferences");
    click(&mut harness, "Hide numpad");

    assert!(harness.state().hidden_number_pad);
    assert!(harness.query_all_by_label("7").next().is_none());
}

#[test]
fn snapshot_startup_choose_output() {
    let mut harness = snapshot_harness(false);
    harness.snapshot("startup_choose_output");
}

#[test]
fn snapshot_empty_playlist() {
    let mut harness = snapshot_harness(true);
    harness.snapshot("playlist_empty");
}
