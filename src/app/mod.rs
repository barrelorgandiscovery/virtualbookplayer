use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local};
use egui::epaint::*;
use egui::*;

use egui_extras::image::*;
use egui_extras::{Size, StripBuilder};
use im_native_dialog::ImNativeFileDialog;

use player::midiio::{DeviceInformation, MidiPlayerFactory};
use player::{PlayerFactory, Response};

use crate::appplayer::AppPlayer;
use crate::gfx::{init_book_fx_gpu, BookEffectKind, BookFxState};
use crate::playmetadata::RankingOrder;
use crate::playmetadata_manager::{HistoryFile, MetadataResult, PlayMetadataManager};
use crate::{duration_to_mm_ss, file_store::*};

use log::{debug, error, info, warn};

use self::i18n::{create_i18n_message_with_lang, I18NMessages};

mod i18n;
mod screen_history;
mod screen_playlist;
mod screen_visu;
mod theme;

#[cfg(test)]
mod gui_kittest;

/// activated screen
#[derive(PartialEq)]
enum Screen {
    PlayListConstruction,
    Display,
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct VirtualBookApp {
    #[serde(skip)]
    screen_zoom_factor: f32,
    slider_selected_zoom_factor: f32,

    xscale: f64,

    /// Player clock (MIDI send time), in milliseconds.
    #[serde(skip)]
    offset_ms: f64,

    /// Smoothed player clock drawn toward by the roll follow filter (no latency).
    /// Visual playhead / FX use [`Self::visual_offset_ms`].
    #[serde(skip)]
    display_offset_ms: f64,
    /// previous frame time, used to detect a stalled restore
    #[serde(skip)]
    last_display_tick: Instant,

    #[serde(skip)]
    screen: Screen,

    #[serde(skip)]
    file_path_dialog: ImNativeFileDialog<Option<PathBuf>>,

    file_store_path: Option<PathBuf>,

    #[serde(skip)]
    file_store: Option<FileStore>,

    #[serde(skip)]
    extensions_filters: Option<Vec<String>>,

    #[serde(skip)]
    current_typed_no: String,

    /// Folder tree, or the recent / ranking lists.
    catalog_pane: screen_history::CatalogPane,

    /// Latest plays from the metadata database. `None` until the worker answers.
    #[serde(skip)]
    history_recent: Option<Vec<HistoryFile>>,

    /// Classement rows for [`Self::ranking_order`]. `None` until the worker answers.
    #[serde(skip)]
    history_ranked: Option<Vec<HistoryFile>>,

    /// Classement sort: stars, plays, or stars then plays.
    ranking_order: RankingOrder,

    #[serde(skip)]
    appplayer: AppPlayer,

    #[serde(skip)]
    current_duration: Duration,

    #[serde(skip)]
    current_devices: Vec<DeviceInformation>,

    #[serde(skip)]
    bg_image: ColorImage,

    #[serde(skip)]
    texture_handle: Option<TextureHandle>,

    #[serde(skip)]
    background_textureid: Option<TextureId>,

    #[serde(skip)]
    background_texture_handle: Option<TextureHandle>,

    #[serde(skip)]
    background_texture_image: ColorImage,

    /// GPU particle / glow state for the virtual-book canvas (None = CPU shapes only).
    #[serde(skip)]
    book_fx: Option<BookFxState>,

    selected_device: usize,

    #[serde(skip)]
    latest_duration_time: Duration,
    #[serde(skip)]
    adjusted_start_time: Instant,
    /// Wall time when tune pause began. Resume shifts the playhead anchor by this.
    #[serde(skip)]
    tune_pause_started: Option<Instant>,

    //date code when the user has the last interaction with the application
    #[serde(skip)]
    last_user_application_date: DateTime<Local>,

    // automatic switch for display the whole
    automatic_switch_to_display_after: Option<u16>,

    /// associated language strings
    #[serde(skip)]
    i18n: Box<I18NMessages>,

    /// Selected interface theme. See [`theme::ThemeId`].
    #[serde(default)]
    theme: theme::ThemeId,

    /// False on saves from before named themes. Those saves are read once
    /// from [`Self::legacy_islight`], then this becomes true.
    #[serde(default)]
    theme_set: bool,

    /// Previous light/dark checkbox. Not written back.
    #[serde(default, rename = "islight", skip_serializing)]
    legacy_islight: bool,

    /// Compensates MIDI / instrument lag behind the player clock.
    /// Positive = audio late → hold the roll / FX back so the blue bar matches what you hear.
    play_lattency_ms: i64,

    /// Selected virtual-book visual effect (see [`BookEffectKind`]).
    book_effect: BookEffectKind,

    /// Scales effect impact: particle count, size/radius, and strength (1.0 = default).
    book_effect_scale: f32,

    /// display the number pad
    hidden_number_pad: bool,

    /// wait time before playing the file
    play_wait: f32,

    /// selected language
    lang: Option<String>,

    /// Play metadata manager for background play count queries
    #[serde(skip)]
    metadata_manager: PlayMetadataManager,

    /// Frame counter for throttling metadata queries
    #[serde(skip)]
    metadata_query_frame_counter: u32,

    /// Currently playing file path and its play metadata
    #[serde(skip)]
    current_playing_file: Option<(PathBuf, Option<u32>)>, // (path, play_count)
}

impl Default for VirtualBookApp {
    fn default() -> Self {
        let mut appplayer = AppPlayer::new();

        appplayer.player(None);

        let img: ColorImage = load_image_bytes(include_bytes!("bg2.png")).unwrap();
        let imgbackground = make_seamless_parchment(
            load_image_bytes(include_bytes!("../../assets/craft.png")).unwrap(),
        );

        Self {
            lang: None,

            offset_ms: 0.0,
            display_offset_ms: 0.0,
            last_display_tick: Instant::now(),

            xscale: 3_000f64,
            screen: Screen::PlayListConstruction,
            screen_zoom_factor: 2.0,
            slider_selected_zoom_factor: 2.0,

            // filter
            current_typed_no: "".into(),
            catalog_pane: screen_history::CatalogPane::Files,
            history_recent: None,
            history_ranked: None,
            ranking_order: RankingOrder::StarsThenPlays,

            file_path_dialog: im_native_dialog::ImNativeFileDialog::default(),
            file_store_path: None,
            file_store: None,

            bg_image: img,
            texture_handle: None,

            background_texture_handle: None,
            background_texture_image: imgbackground,
            background_textureid: None,

            appplayer,
            current_duration: Duration::new(0, 0),
            current_devices: vec![],
            selected_device: 0,

            latest_duration_time: Duration::new(0, 0),
            adjusted_start_time: Instant::now(), // start time since we start the play
            tune_pause_started: None,

            play_lattency_ms: 0, // 0 ms lattency
            book_effect: BookEffectKind::NoteFire,
            book_effect_scale: 1.0,

            i18n: create_i18n_message_with_lang(None),

            theme: theme::ThemeId::Night,
            theme_set: true,
            legacy_islight: false,

            hidden_number_pad: false,

            extensions_filters: Some(vec![".mid".into(), ".book".into(), ".playlist".into()]),

            play_wait: 2.0,

            last_user_application_date: chrono::Local::now(),
            automatic_switch_to_display_after: Some(10),
            metadata_manager: PlayMetadataManager::new(),
            metadata_query_frame_counter: 0,
            current_playing_file: None,
            book_fx: None,
        }
    }
}

impl VirtualBookApp {
    /// Get the currently playing file's metadata (path and play count)
    pub fn current_playing_file_metadata(&self) -> Option<(&PathBuf, Option<u32>)> {
        self.current_playing_file
            .as_ref()
            .map(|(path, count)| (path, *count))
    }

    /// Setup fonts for the application
    fn setup_fonts(ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();

        // Install my own font (maybe supporting non-latin characters):
        fonts.font_data.insert(
            "my_font".to_owned(),
            FontData::from_static(include_bytes!("../../fonts/Rubik-VariableFont_wght.ttf")).into(),
        ); // .ttf and .otf supported

        // Install bold font - using Rubik-Bold.ttf
        fonts.font_data.insert(
            "rubik_bold".to_owned(),
            FontData::from_static(include_bytes!("../../fonts/Rubik-Bold.ttf")).into(),
        );

        fonts.font_data.insert(
            "icon_font".to_owned(),
            FontData::from_static(include_bytes!("../../fonts/fa-solid-900.ttf")).into(),
        );

        // Put my font first (highest priority):
        fonts
            .families
            .get_mut(&FontFamily::Proportional)
            .unwrap()
            .insert(0, "my_font".to_owned());

        // Put my font as last fallback for monospace:
        fonts
            .families
            .get_mut(&FontFamily::Monospace)
            .unwrap()
            .push("my_font".to_owned());

        // Register bold font as a separate font family
        let bold_family = vec!["rubik_bold".to_owned()];
        fonts
            .families
            .insert(FontFamily::Name("rubik_bold".into()), bold_family);

        let v = vec!["icon_font".to_owned()];

        fonts
            .families
            .insert(FontFamily::Name("icon_font".into()), v);

        // Add fill variant FIRST (higher priority) for filled icons (stars, folders)
        fonts.font_data.insert(
            "phosphor-fill".to_owned(),
            egui_phosphor::Variant::Fill.font_data().into(),
        );
        // Insert fill variant BEFORE regular variant in Proportional font family
        // This ensures fill variant characters are found first when using fill:: constants
        let proportional_family = fonts.families.get_mut(&FontFamily::Proportional).unwrap();
        // Find where "phosphor" (regular variant) is and insert fill before it
        if let Some(pos) = proportional_family.iter().position(|f| f == "phosphor") {
            proportional_family.insert(pos, "phosphor-fill".to_owned());
        } else {
            // If regular variant not found yet, just add fill variant
            proportional_family.push("phosphor-fill".to_owned());
        }

        // Add regular variant as fallback to Proportional (after fill variant)
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

        ctx.set_fonts(fonts);
    }

    /// Restore MIDI device from saved state
    fn restore_midi_device(app: &mut Self) {
        let devices = match MidiPlayerFactory::list_all_devices() {
            Ok(devices) => devices,
            Err(e) => {
                error!("fail to list MIDI devices: {}", e);
                return;
            }
        };

        if devices.is_empty() {
            warn!("no MIDI output devices available on restore");
            return;
        }

        app.current_devices = devices;
        if !app
            .current_devices
            .iter()
            .any(|device| device.no == app.selected_device)
        {
            let fallback = app.current_devices[0].no;
            warn!(
                "saved MIDI device {} is unavailable; falling back to {}",
                app.selected_device, fallback
            );
            app.selected_device = fallback;
        }

        let factory = MidiPlayerFactory {
            device_no: app.selected_device,
        };

        let (_scmd, rcmd) = channel();
        let (s, r) = channel();

        match factory.create(s, rcmd) {
            Ok(player) => {
                app.appplayer.player(Some((player, r, _scmd)));
            }
            Err(e) => {
                error!("fail to open device {}", e);
            }
        }
    }

    /// Restore file store from saved state
    fn restore_file_store(app: &mut Self) {
        if let Some(path) = &app.file_store_path {
            // Initialize metadata database when restoring
            let db_path = PathBuf::from(path).join(".playmetadata.db");
            info!("Restoring metadata database for folder: {:?}", path);
            app.metadata_manager.set_database_path(db_path);

            match FileStore::new(&PathBuf::from(path)) {
                Ok(mut storage_created) => {
                    if storage_created.is_some() {
                        if let Some(mut fs) = storage_created {
                            if let Ok(v) = fs.view(&None, &app.extensions_filters) {
                                fs.default_view = Some(v);

                                // Immediately query play counts for visible files (don't wait for periodic query)
                                let visible_paths = Self::collect_visible_file_paths(&fs);
                                if !visible_paths.is_empty() {
                                    debug!("Immediately querying play counts for {} visible files (on restore)", visible_paths.len());
                                    app.metadata_manager
                                        .query_play_counts(visible_paths, Vec::new());
                                } else {
                                    warn!("No visible files to query on restore - file tree may not be ready yet");
                                }
                            }
                            storage_created = Some(fs);
                        }
                    }
                    app.file_store = storage_created;
                }
                Err(e) => {
                    error!("error in opening the path {}", e);
                    app.file_store = None;
                }
            }
        }
    }

    /// Restore application state from storage
    fn restore_state(storage: &dyn eframe::Storage, lang: Option<String>) -> Option<Self> {
        let mut old_storage: Self = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();

        Self::restore_midi_device(&mut old_storage);
        Self::restore_file_store(&mut old_storage);

        old_storage.theme = theme::resolve_saved_theme(
            old_storage.theme,
            old_storage.theme_set,
            old_storage.legacy_islight,
        );
        old_storage.theme_set = true;

        old_storage.lang.clone_from(&lang);
        old_storage.i18n = create_i18n_message_with_lang(lang);

        old_storage.screen_zoom_factor = old_storage.slider_selected_zoom_factor;

        // define the wait time on restoration
        old_storage
            .appplayer
            .set_waittime_between_file_play(old_storage.play_wait);

        Some(old_storage)
    }

    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>, reset: bool, lang: Option<String>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        Self::setup_fonts(&cc.egui_ctx);

        // Register book GPU pipelines once (Windows/Linux/macOS via wgpu).
        let gpu_ready = init_book_fx_gpu(cc);

        let mut app = if !reset {
            // Load previous app state (if any).
            // Note that you must enable the `persistence` feature for this to work.
            if let Some(storage) = cc.storage {
                Self::restore_state(storage, lang.clone())
            } else {
                None
            }
        } else {
            None
        }
        .unwrap_or_else(|| {
            let lang_clone = lang.clone();
            VirtualBookApp {
                lang: lang_clone.clone(),
                i18n: create_i18n_message_with_lang(lang_clone),
                ..Default::default()
            }
        });

        let mut fx = BookFxState::default();
        fx.set_gpu_ready(gpu_ready);
        fx.set_effect(app.book_effect);
        fx.set_effect_scale(app.book_effect_scale);
        app.book_fx = Some(fx);
        app
    }

    /// Install the selected theme. Skips the work when it is already current.
    fn setup_visuals_and_style(&self, ctx: &egui::Context) {
        theme::apply(ctx, self.theme);
    }

    /// Book time under the blue bar (and for note-hit FX), in milliseconds.
    ///
    /// Latency is applied here — not inside the follow filter — so the slider
    /// immediately shifts the roll and sparks relative to MIDI without fighting
    /// the smoother, including while paused.
    pub(crate) fn visual_offset_ms(&self) -> f64 {
        self.display_offset_ms - self.play_lattency_ms as f64
    }

    /// Update the smoothed player clock used as the basis for the visual playhead.
    fn update_player_state(&mut self) {
        let now = Instant::now();
        let dt = now
            .saturating_duration_since(self.last_display_tick)
            .as_secs_f64();
        self.last_display_tick = now;

        // Hold the last player position unless a book is actually playing.
        // Latency is NOT folded into this target; see `visual_offset_ms`.
        // Tune pause keeps `is_playing` true but must not walk the roll.
        let paused = self.appplayer.is_paused();
        if paused {
            if self.tune_pause_started.is_none() {
                self.tune_pause_started = Some(now);
            }
        } else if let Some(started) = self.tune_pause_started.take() {
            // Drop the wall time spent paused so the roll continues where it froze.
            let paused_for = now.saturating_duration_since(started);
            self.adjusted_start_time = self
                .adjusted_start_time
                .checked_add(paused_for)
                .unwrap_or(now);
        }

        let mut target_ms = self.display_offset_ms;
        if self.appplayer.is_playing()
            && !paused
            && self.adjusted_start_time + Duration::from_millis(100) < now
            && self.appplayer.virtual_book.read().is_some()
        {
            let delta = now.saturating_duration_since(self.adjusted_start_time);
            self.current_duration = delta;
            self.offset_ms = delta.as_millis() as f64;
            target_ms = self.offset_ms;
        }

        self.display_offset_ms = follow_display_offset(self.display_offset_ms, target_ms, dt);
    }

    /// Handle player response messages.
    ///
    /// Playlist advance already happened on the player thread. This only
    /// catches the UI up: play counts, the open file, and the clock.
    fn handle_player_responses(&mut self) {
        let responses = {
            if let Ok(mut queued) = self.appplayer.last_response.lock() {
                std::mem::take(&mut *queued)
            } else {
                return;
            }
        };

        for last_response in &responses {
            match last_response {
                Response::EndOfFile(_) => {
                    // The next file, if any, was already started off this thread.
                    self.current_playing_file = None;
                }
                Response::CurrentPlayTime(duration) => {
                    self.latest_duration_time = *duration;
                    // Re-anchor the wall clock to the MIDI player's reported time.
                    // Visual latency is applied in `visual_offset_ms`, not here.
                    // While paused (and on the frame that closes the pause) keep the
                    // anchor so the roll does not jump back to the last MIDI event.
                    if !self.appplayer.is_paused() && self.tune_pause_started.is_none() {
                        self.adjusted_start_time = Instant::now() - *duration;
                    }
                }
                Response::FileCancelled => {
                    // Clear current playing file when cancelled
                    self.current_playing_file = None;
                }
                Response::FilePlayStarted((filename, _notes)) => {
                    if let Some(fx) = self.book_fx.as_mut() {
                        fx.on_book_changed();
                    }
                    // Record play event in background thread (low priority)
                    // The filename might be relative or absolute, so we need to handle both cases
                    if let Some(file_path) = &self.file_store_path {
                        // If filename is already absolute, use it; otherwise join with folder path
                        let full_path = if PathBuf::from(&filename).is_absolute() {
                            PathBuf::from(&filename)
                        } else {
                            file_path.join(filename)
                        };
                        info!("Recording play event for file: {:?}", full_path);

                        // Store current playing file and query its play count
                        self.current_playing_file = Some((full_path.clone(), None)); // Count will be updated when result arrives

                        // Record the play event (this will also refresh the count)
                        self.metadata_manager.record_play_event(full_path.clone());

                        // Also immediately query the current file's play count
                        self.metadata_manager
                            .query_current_file_play_count(full_path);
                    }
                }
            }
        }
    }

    /// Handle file dialog responses
    fn handle_file_dialog(&mut self) {
        if let Some(Ok(result)) = self.file_path_dialog.check() {
            self.file_store_path.clone_from(&result);
            if let Some(r) = result {
                // Initialize metadata database in the selected folder
                // Database file will be created in the folder: .playmetadata.db
                let db_path = r.join(".playmetadata.db");
                info!("Initializing metadata database for folder: {:?}", r);
                self.metadata_manager.set_database_path(db_path);
                self.history_recent = None;
                self.history_ranked = None;
                if self.catalog_pane == screen_history::CatalogPane::History {
                    self.request_history_lists();
                }

                match FileStore::new(&r) {
                    Ok(fs) => {
                        self.file_store = fs;
                        debug!("folder opened, reapply the elements and views");
                        // refilter the view using the filters
                        if let Some(store) = &mut self.file_store {
                            if let Ok(result) = store.view(&None, &self.extensions_filters) {
                                store.default_view = Some(result);

                                // Wait a bit for database to initialize, then query play counts
                                // Database initialization happens asynchronously in background thread
                                // We'll query on next frame or use periodic query
                                debug!("File store created, will query play counts after database initialization");

                                // Immediately query play counts for visible files (don't wait for periodic query)
                                // Note: Database might not be ready yet, but query will be queued
                                let visible_paths = Self::collect_visible_file_paths(store);
                                if !visible_paths.is_empty() {
                                    debug!(
                                        "Immediately querying play counts for {} visible files",
                                        visible_paths.len()
                                    );
                                    self.metadata_manager
                                        .query_play_counts(visible_paths, Vec::new());
                                } else {
                                    warn!("No visible files collected after folder open!");
                                }
                            } else {
                                warn!("Failed to create view for file store");
                            }
                        }
                    }
                    Err(e) => {
                        error!(
                            "fail to create file store : {} \n with path {:?}",
                            e,
                            r.clone()
                        )
                    }
                }
            }
        }
    }

    /// Ask the metadata worker for the recent list and the classement.
    pub(crate) fn request_history_lists(&self) {
        self.metadata_manager.query_history(
            screen_history::HISTORY_LIST_LIMIT,
            screen_history::HISTORY_LIST_LIMIT,
            self.ranking_order,
        );
    }

    /// Update metadata - query play counts and star counts for displayed files and process results
    fn update_metadata(&mut self) {
        // Process any results from background thread
        for result in self.metadata_manager.process_results() {
            let file_metadata = match result {
                MetadataResult::History {
                    recent,
                    ranked,
                    ranking_order,
                } => {
                    self.history_recent = Some(recent);
                    if ranking_order == self.ranking_order {
                        self.history_ranked = Some(ranked);
                    }
                    continue;
                }
                MetadataResult::PlayCounts(file_metadata) => file_metadata,
            };
            debug!(
                "Received {} metadata results from background thread",
                file_metadata.len()
            );

            // Update current playing file's metadata if it's in the results
            if let Some((current_path, _)) = &self.current_playing_file {
                if let Some(metadata) = file_metadata.get(current_path) {
                    let path_clone = current_path.clone();
                    self.current_playing_file =
                        Some((path_clone.clone(), Some(metadata.play_count)));
                    info!(
                        "Current playing file metadata: {} -> play={}, star={}",
                        path_clone.display(),
                        metadata.play_count,
                        metadata.star_count
                    );
                }
            }

            // Update FileNode metadata in the file store
            if let Some(file_store) = &mut self.file_store {
                let mut updated_count = 0;
                Self::update_file_node_metadata_with_debug(
                    &mut file_store.root,
                    &file_metadata,
                    &mut updated_count,
                );
                if updated_count > 0 {
                    debug!("Updated {} FileNode metadata in root", updated_count);
                }

                // Also update filtered view if it exists
                if let Some(ref mut filtered_view) = file_store.filtered_view {
                    let mut updated_count = 0;
                    Self::update_file_view_node_metadata_with_debug(
                        &mut filtered_view.root,
                        &file_metadata,
                        &mut updated_count,
                    );
                    if updated_count > 0 {
                        debug!(
                            "Updated {} FileViewNode metadata in filtered view",
                            updated_count
                        );
                    }
                }
                // Update default view
                if let Some(ref mut default_view) = file_store.default_view {
                    let mut updated_count = 0;
                    Self::update_file_view_node_metadata_with_debug(
                        &mut default_view.root,
                        &file_metadata,
                        &mut updated_count,
                    );
                    if updated_count > 0 {
                        debug!(
                            "Updated {} FileViewNode metadata in default view",
                            updated_count
                        );
                    }
                }
            }
        }

        // Periodically query play counts for displayed files (every 2 seconds)
        // Use frame count to throttle queries
        // Only fetch metadata for files that are actually visible (expanded folders)
        self.metadata_query_frame_counter += 1;
        if self.catalog_pane == screen_history::CatalogPane::History
            && self.file_store_path.is_some()
        {
            let waiting = self.history_recent.is_none();
            let refresh_every = if waiting { 30 } else { 120 };
            if self.metadata_query_frame_counter == 1
                || self
                    .metadata_query_frame_counter
                    .is_multiple_of(refresh_every)
            {
                self.request_history_lists();
            }
        }
        if self.metadata_query_frame_counter.is_multiple_of(120)
            || self.metadata_query_frame_counter == 1
        {
            // Folder headers show played/total for collapsed subtrees too, so fill
            // play counts that the visible-file query has not reached yet.
            if let Some(file_store) = &self.file_store {
                let missing = Self::collect_unqueried_file_paths(file_store, 48);
                if !missing.is_empty() {
                    debug!(
                        "Querying play counts for {} files not yet loaded",
                        missing.len()
                    );
                    self.metadata_manager.query_play_counts(missing, Vec::new());
                }
            }
        }
        if self.metadata_query_frame_counter.is_multiple_of(120) {
            // Query every ~2 seconds at 60fps
            if let Some(file_store) = &self.file_store {
                // Only collect visible files (from expanded folders) - don't fetch for collapsed folders
                let visible_paths = Self::collect_visible_file_paths(file_store);

                if !visible_paths.is_empty() {
                    debug!(
                        "Periodically querying play counts for {} visible files",
                        visible_paths.len()
                    );
                    // Only query visible files - empty list for non-visible files
                    self.metadata_manager
                        .query_play_counts(visible_paths, Vec::new());
                }
            }
        }

        // Also trigger an immediate query on first frame (when files are first displayed)
        // This ensures play counts appear immediately rather than waiting for the periodic query
        if self.metadata_query_frame_counter == 1 {
            debug!("First frame - checking if file store is ready for query");
            if let Some(file_store) = &self.file_store {
                let visible_paths = Self::collect_visible_file_paths(file_store);
                if !visible_paths.is_empty() {
                    debug!(
                        "Immediately querying play counts for {} visible files (first frame)",
                        visible_paths.len()
                    );
                    self.metadata_manager
                        .query_play_counts(visible_paths, Vec::new());
                } else {
                    warn!("No visible files on first frame - file tree may not be ready yet");
                }
            } else {
                warn!("No file store available on first frame");
            }
        }
    }

    /// Files whose play count has not been loaded yet, depth-first, capped.
    fn collect_unqueried_file_paths(file_store: &FileStore, limit: usize) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        if let Some(view) = file_store.default_view.as_ref() {
            Self::collect_unqueried_from_view(&mut paths, &view.root, limit);
        }
        paths
    }

    fn collect_unqueried_from_view(
        paths: &mut Vec<PathBuf>,
        node: &std::rc::Rc<std::cell::RefCell<crate::file_store::FileViewNode>>,
        limit: usize,
    ) {
        if paths.len() >= limit {
            return;
        }
        let view_node = node.borrow();
        let file_node = view_node.node.borrow();
        if !file_node.is_folder {
            if file_node.play_count.is_none() {
                paths.push(file_node.path.clone());
            }
            return;
        }
        drop(file_node);
        let children = view_node.childs.clone();
        drop(view_node);
        for child in children {
            if paths.len() >= limit {
                break;
            }
            Self::collect_unqueried_from_view(paths, &child, limit);
        }
    }

    /// Recursively collect all displayed file paths from the file store
    #[allow(dead_code)]
    fn collect_displayed_file_paths(file_store: &FileStore) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let current_view = file_store.default_view.as_ref();
        if let Some(view) = current_view {
            Self::collect_paths_from_view_node(&mut paths, &view.root);
        }
        paths
    }

    /// Recursively collect only visible (expanded) file paths from a FileViewNode
    /// Only collects files from expanded folders (high priority for immediate display)
    /// The base folder is always displayed and expanded, so we start with parent_expanded=true
    fn collect_visible_file_paths(file_store: &FileStore) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let current_view = file_store.default_view.as_ref();
        if let Some(view) = current_view {
            debug!("Default view exists, collecting from root node");
            // Root folder is always displayed in GUI, so we always collect its direct children (files)
            // Pass true for both parent_expanded and is_root to ensure root-level files are always collected
            Self::collect_visible_paths_from_view_node(&mut paths, &view.root, true, true);
            debug!(
                "Collected {} visible file paths from file store",
                paths.len()
            );
            if paths.is_empty() {
                warn!("No visible files collected - checking view structure");
                // Debug: check if view has any children
                let view_node = view.root.borrow();
                let file_node = view_node.node.borrow();
                debug!(
                    "View root: is_folder={}, path={:?}, has {} children",
                    file_node.is_folder,
                    file_node.path,
                    view_node.childs.len()
                );
                if !view_node.childs.is_empty() {
                    warn!(
                        "View has {} children but no files collected - may be folders only",
                        view_node.childs.len()
                    );
                }
            } else if paths.len() <= 10 {
                for path in &paths {
                    debug!("  Visible file: {:?}", path);
                }
            }
        } else {
            warn!("No default view available for collecting visible file paths");
        }
        paths
    }

    /// Recursively collect paths from a FileViewNode (all files, regardless of expansion)
    #[allow(dead_code)]
    fn collect_paths_from_view_node(
        paths: &mut Vec<PathBuf>,
        node: &std::rc::Rc<std::cell::RefCell<crate::file_store::FileViewNode>>,
    ) {
        let view_node = node.borrow();
        let file_node = view_node.node.borrow();

        if !file_node.is_folder {
            paths.push(file_node.path.clone());
        }

        for child in &view_node.childs {
            Self::collect_paths_from_view_node(paths, child);
        }
    }

    /// Recursively collect only visible paths (from expanded folders)
    /// parent_expanded: whether the parent folder is expanded
    /// is_root: whether this is the root folder (always displayed in GUI)
    /// Note: The root folder is always displayed in GUI, so its direct children (files) are always collected
    fn collect_visible_paths_from_view_node(
        paths: &mut Vec<PathBuf>,
        node: &std::rc::Rc<std::cell::RefCell<crate::file_store::FileViewNode>>,
        parent_expanded: bool,
        is_root: bool,
    ) {
        let view_node = node.borrow();
        let file_node = view_node.node.borrow();
        let is_expanded = view_node.expanded;

        // Collect files if:
        // 1. Parent is expanded (so they're visible), OR
        // 2. This is the root folder (always displayed in GUI)
        // Root folder's direct children (files) are always visible regardless of root's expansion state
        if !file_node.is_folder {
            if parent_expanded || is_root {
                debug!(
                    "Collecting file: {:?} (parent_expanded={}, is_root={})",
                    file_node.path, parent_expanded, is_root
                );
                paths.push(file_node.path.clone());
            } else {
                debug!(
                    "Skipping file: {:?} (parent not expanded, not root)",
                    file_node.path
                );
            }
        } else if file_node.is_folder {
            debug!(
                "Processing folder: {:?} (parent_expanded={}, is_expanded={}, is_root={})",
                file_node.path, parent_expanded, is_expanded, is_root
            );
        }

        // Recurse into children if:
        // 1. Parent is expanded (base folder case), OR
        // 2. This folder is expanded, OR
        // 3. This is the root folder (always displayed, so always recurse into its children)
        // Root folder's children are always processed regardless of root's expansion state
        if parent_expanded || is_expanded || is_root {
            debug!("Recursing into {} children of {:?} (parent_expanded={}, is_expanded={}, is_root={})", 
                   view_node.childs.len(), file_node.path, parent_expanded, is_expanded, is_root);
            for child in &view_node.childs {
                // Pass the expanded state of this folder to children
                // For root folder children: is_root=false (they're not root), but parent_expanded should be true if root is always displayed
                // Actually, for root's direct children, we want them to always be collected, so we pass is_root=false but parent_expanded=true
                // For nested folders, we pass is_root=false and use the folder's expanded state
                Self::collect_visible_paths_from_view_node(
                    paths,
                    child,
                    is_expanded || is_root,
                    false,
                );
            }
        } else {
            debug!(
                "Not recursing into {:?} - parent not expanded, folder not expanded, and not root",
                file_node.path
            );
        }
    }

    /// Recursively update FileNode metadata (play counts and star counts) with debug tracking
    fn update_file_node_metadata_with_debug(
        node: &mut std::rc::Rc<std::cell::RefCell<crate::file_store::FileNode>>,
        file_metadata: &std::collections::HashMap<
            PathBuf,
            crate::playmetadata_manager::FileMetadata,
        >,
        updated_count: &mut usize,
    ) {
        let mut file_node = node.borrow_mut();
        if !file_node.is_folder {
            if let Some(metadata) = file_metadata.get(&file_node.path) {
                let old_play_count = file_node.play_count;
                let old_star_count = file_node.star_count;
                file_node.play_count = Some(metadata.play_count);
                file_node.star_count = Some(metadata.star_count);
                if old_play_count != Some(metadata.play_count)
                    || old_star_count != Some(metadata.star_count)
                {
                    debug!(
                        "Updated FileNode metadata: {:?} -> play={} (was {:?}), star={} (was {:?})",
                        file_node.path,
                        metadata.play_count,
                        old_play_count,
                        metadata.star_count,
                        old_star_count
                    );
                    *updated_count += 1;
                }
            } else {
                debug!("No metadata found for FileNode: {:?}", file_node.path);
            }
        }

        for child in &mut file_node.folder_files {
            Self::update_file_node_metadata_with_debug(child, file_metadata, updated_count);
        }
    }

    /// Recursively update FileViewNode metadata (updates underlying FileNode)
    #[allow(dead_code)]
    fn update_file_view_node_metadata(
        node: &mut std::rc::Rc<std::cell::RefCell<crate::file_store::FileViewNode>>,
        file_metadata: &std::collections::HashMap<
            PathBuf,
            crate::playmetadata_manager::FileMetadata,
        >,
    ) {
        let mut updated_count = 0;
        Self::update_file_view_node_metadata_with_debug(node, file_metadata, &mut updated_count);
    }

    /// Recursively update FileViewNode metadata (play counts and star counts) with debug tracking
    fn update_file_view_node_metadata_with_debug(
        node: &mut std::rc::Rc<std::cell::RefCell<crate::file_store::FileViewNode>>,
        file_metadata: &std::collections::HashMap<
            PathBuf,
            crate::playmetadata_manager::FileMetadata,
        >,
        updated_count: &mut usize,
    ) {
        let mut view_node = node.borrow_mut();
        let mut file_node = view_node.node.borrow_mut();

        if !file_node.is_folder {
            if let Some(metadata) = file_metadata.get(&file_node.path) {
                let old_play_count = file_node.play_count;
                let old_star_count = file_node.star_count;
                file_node.play_count = Some(metadata.play_count);
                file_node.star_count = Some(metadata.star_count);
                if old_play_count != Some(metadata.play_count)
                    || old_star_count != Some(metadata.star_count)
                {
                    debug!("Updated FileViewNode metadata: {:?} -> play={} (was {:?}), star={} (was {:?})", 
                        file_node.path, metadata.play_count, old_play_count, metadata.star_count, old_star_count);
                    *updated_count += 1;
                }
            } else {
                debug!("No metadata found for FileViewNode: {:?}", file_node.path);
            }
        }

        drop(file_node);

        for child in &mut view_node.childs {
            Self::update_file_view_node_metadata_with_debug(child, file_metadata, updated_count);
        }
    }

    /// Render file menu items
    fn render_file_menu(&mut self, ui: &mut egui::Ui) {
        if ui
            .button(format!(
                "{} {}",
                egui_phosphor::regular::FOLDER_OPEN,
                self.i18n.open_folder
            ))
            .clicked()
        {
            let mut location: Option<PathBuf> = self.file_store_path.clone();
            if let Some(loc) = &location {
                // check location exists
                match loc.metadata() {
                    Ok(_r) => {
                        if !_r.is_dir() {
                            location = None;
                        }
                    }
                    Err(_e) => {
                        location = None;
                    }
                }
            }

            if let Err(_result_open_single_dir) = self.file_path_dialog.open_single_dir(location) {
                error!("fail to open dir dialog");
            } else {
                debug!("dialog opened");
            }

            ui.close();
        }
        ui.separator();
        if ui
            .button(format!(
                "{} {}",
                egui_phosphor::regular::RECYCLE,
                self.i18n.reload_folder
            ))
            .clicked()
        {
            if let Some(current_path) = &self.file_store_path {
                let new_filestore = FileStore::new(current_path);
                if let Ok(new_store) = new_filestore {
                    self.file_store = new_store.map(|mut fs| {
                        if let Ok(v) = fs.view(&None, &self.extensions_filters) {
                                                fs.default_view = Some(v);

                            // Immediately query play counts for visible files (don't wait for periodic query)
                            let visible_paths = Self::collect_visible_file_paths(&fs);
                            if !visible_paths.is_empty() {
                                debug!("Immediately querying play counts for {} visible files (on reload)", visible_paths.len());
                                self.metadata_manager.query_play_counts(visible_paths, Vec::new());
                            }
                                            }
                                            fs
                                        });
                }
            }
            ui.close();
        };
    }

    /// Render preferences menu
    fn render_preferences_menu(&mut self, ui: &mut egui::Ui) {
        ui.label(&self.i18n.zoom);
        let result = ui.add(egui::Slider::new(
            &mut self.slider_selected_zoom_factor,
            1.5..=6.0,
        ));
        if !result.is_pointer_button_down_on() {
            self.screen_zoom_factor = self.slider_selected_zoom_factor;
        };

        ui.checkbox(&mut self.hidden_number_pad, &self.i18n.hide_num_pad);
        // A submenu keeps this menu short enough to reach the controls below.
        let theme_label = format!("{}: {}", self.i18n.theme, self.i18n.theme_name(self.theme));
        ui.menu_button(theme_label, |ui| {
            for id in theme::ThemeId::ALL {
                let label = self.i18n.theme_name(id).to_owned();
                if ui.radio_value(&mut self.theme, id, label).changed() {
                    self.theme_set = true;
                }
            }
        });
        ui.label(&self.i18n.time_between_file);
        let time_slider = egui::Slider::new(&mut self.play_wait, 0.0..=30.0);
        if time_slider.ui(ui).changed() {
            self.appplayer
                .set_waittime_between_file_play(self.play_wait);
        }
        ui.separator();
        let mut hasvalue = self.automatic_switch_to_display_after.is_some();
        if ui.checkbox(&mut hasvalue, &self.i18n.switch_auto).changed() {
            if !hasvalue {
                self.automatic_switch_to_display_after = None;
            } else {
                self.automatic_switch_to_display_after = Some(10);
            }
        }

        if let Some(value) = &mut self.automatic_switch_to_display_after {
            let automatic_switch_value = egui::Slider::new(value, 5..=300);
            ui.add(automatic_switch_value);
        }

        ui.separator();

        ui.label(&self.i18n.lattence_jeu);
        let play_lattency_slider = egui::Slider::new(&mut self.play_lattency_ms, -1000..=4_000);
        ui.add(play_lattency_slider).on_hover_text(
            "Shifts the roll and play FX relative to MIDI.\n\
             Positive: audio/MIDI late — delay the blue-bar hit to match what you hear.\n\
             Negative: visuals late — advance the blue-bar hit.",
        );

        ui.separator();
        ui.label(&self.i18n.book_effect);
        // Radios work reliably inside egui menus (ComboBox popups often close with the menu).
        for kind in BookEffectKind::all() {
            let label = self.i18n.book_effect_label(*kind).to_owned();
            ui.radio_value(&mut self.book_effect, *kind, label);
        }
        ui.add_enabled_ui(self.book_effect != BookEffectKind::None, |ui| {
            ui.label(&self.i18n.book_effect_scale);
            ui.add(
                egui::Slider::new(&mut self.book_effect_scale, 0.0..=2.5)
                    .suffix("×")
                    .show_value(true),
            )
            .on_hover_text(
                "Scales effect impact during play:\n\
                 particle count, size / radius, and strength.\n\
                 1.0 = default · 0 = muted · >1 = stronger",
            );
        });
        if let Some(fx) = self.book_fx.as_mut() {
            fx.set_effect(self.book_effect);
            fx.set_effect_scale(self.book_effect_scale);
        }
    }

    /// Render MIDI device selection
    fn render_midi_devices(&mut self, ui: &mut egui::Ui) {
        ui.label("midi out interfaces");
        for device in &self.current_devices {
            let selected = self.selected_device == device.no;
            if ui.radio(selected, &device.label).clicked() {
                if let Some(_old_player) = &self.appplayer.player {}

                println!("Open the device");
                self.selected_device = device.no;

                let factory = MidiPlayerFactory {
                    device_no: self.selected_device,
                };

                let (_scmd, rcmd) = channel();
                let (s, player_event_receiver) = channel();

                match factory.create(s, rcmd) {
                    Ok(player) => {
                        // change the player
                        self.appplayer
                            .player(Some((player, player_event_receiver, _scmd)));
                    }
                    Err(e) => {
                        error!("fail to open device {}", e);
                    }
                }

                ui.close();
            }
        }
    }

    /// Render playlist play/stop and, while a file is open, tune pause.
    ///
    /// Playlist play/stop cancels the file and leaves the queue. Tune pause
    /// freezes that file and keeps playlist play mode.
    fn render_floating_play_button(&mut self, ctx: &egui::Context, top_panel_rect: Rect) {
        let play_mod = self.appplayer.play_mod();
        let tune_open = self.appplayer.is_playing();
        let tune_paused = tune_open && self.appplayer.is_paused();

        let button_size = 32.0;
        let margin = 8.0;
        let gap = 8.0;
        let screen_rect = ctx.content_rect();
        let y = top_panel_rect.top() + margin;
        let playlist_x = screen_rect.right() - button_size - margin;

        if tune_open {
            let tune_icon = if tune_paused {
                egui_phosphor::fill::PLAY
            } else {
                egui_phosphor::fill::PAUSE
            };
            let tune_hover = if tune_paused {
                self.i18n.hover_resume_the_tune.clone()
            } else {
                self.i18n.hover_pause_the_tune.clone()
            };
            if show_transport_button(
                ctx,
                Id::unique("tune_pause_button"),
                Pos2::new(playlist_x - gap - button_size, y),
                button_size,
                tune_icon,
                tune_paused,
                &tune_hover,
            ) {
                self.appplayer.set_tune_paused(!tune_paused);
                ctx.request_repaint();
            }
        }

        let playlist_icon = if play_mod {
            egui_phosphor::fill::STOP
        } else {
            egui_phosphor::fill::PLAY
        };
        let playlist_hover = self.i18n.hover_activate_the_play_of_the_playlist.clone();
        if show_transport_button(
            ctx,
            Id::unique("floating_play_button"),
            Pos2::new(playlist_x, y),
            button_size,
            playlist_icon,
            play_mod,
            &playlist_hover,
        ) {
            if play_mod {
                self.appplayer.stop();
            } else {
                // Stay in play mode even when the playlist is still empty,
                // so the first file added later starts on its own.
                self.appplayer.begin_playlist();
            }
            ctx.request_repaint();
        }
    }

    /// Render play time and title (centered)
    fn render_play_time_and_title(&mut self, ui: &mut egui::Ui) {
        if self.appplayer.is_playing() {
            let current_playlist = &self
                .appplayer
                .playlist
                .lock()
                .expect("fail to lock playlist");
            let cell = current_playlist.current();
            if let Some(t) = cell {
                let name = t.name.clone();

                // Calculate durations
                let mut total_duration = String::from("-");
                if let Some(duration) = current_playlist.computed_length {
                    total_duration = duration_to_mm_ss(&duration);
                }

                let mut current_file_remaining_duration = String::from("");
                if let Some(current_play) = current_playlist.current() {
                    if let Some(additional_info) = current_play.additional_informations {
                        if let Some(dur) = additional_info.duration {
                            if dur > self.current_duration {
                                let remaining_current = dur - self.current_duration;
                                current_file_remaining_duration =
                                    duration_to_mm_ss(&remaining_current);
                            }
                        }
                    }
                }

                // Center the content horizontally
                ui.horizontal_centered(|ui| {
                    // Play time
                    ui.label(
                        RichText::new(format!(
                            "{} / {}",
                            current_file_remaining_duration, total_duration
                        ))
                        .font(FontId::proportional(14.0))
                        .color(ui.style().visuals.text_color()),
                    );

                    ui.add_space(12.0);

                    let marks = theme::tokens(ui.ctx());
                    let title_bg_color = marks.now_playing_bg;
                    let title_text_color = marks.now_playing_fg;
                    let bold_font = theme::bold_font(16.0);
                    // Use simple ASCII characters that will render reliably: " > " and " < "
                    let mut rt = RichText::new(format!(" > {} < ", name));
                    rt = rt.font(bold_font); // Uses Rubik-Bold font for better visibility
                    rt = rt.background_color(title_bg_color);
                    rt = rt.color(title_text_color);
                    ui.label(rt);

                    ui.add_space(8.0);

                    // Star button - only show when file is playing (using fill variant for filled star)
                    if ui
                        .button(egui_phosphor::fill::STAR)
                        .on_hover_text_at_pointer(&self.i18n.star_file_tooltip)
                        .clicked()
                    {
                        // Get the current playing file path
                        if let Some(current_play) = current_playlist.current() {
                            let full_path = &current_play.path;
                            info!("Star button clicked for file: {:?}", full_path);
                            self.metadata_manager.record_star_event(full_path.clone());
                        }
                    }
                });
            }
        } else {
            // Show empty state when not playing - no text displayed
            ui.horizontal_centered(|_ui| {
                // Empty - no text shown when not playing
            });
        }
    }
}

/// Scroll a menu when zoom makes it taller than the window.
///
/// `popup_top` is the screen Y where the popup opens. The scroller is capped
/// to the space under that edge, so the rows that would be clipped stay reachable.
#[cfg(not(target_arch = "wasm32"))]
fn show_scrollable_menu(ui: &mut Ui, popup_top: f32, add_contents: impl FnOnce(&mut Ui)) {
    let screen = ui.ctx().content_rect();
    // Popup frame (margin + stroke) and a small gap so the frame stays on screen.
    let chrome = 20.0;
    let room = (screen.height() - chrome).max(32.0);
    let max_height = (screen.bottom() - popup_top - chrome).clamp(32.0, room);
    ui.set_max_height(max_height);
    ScrollArea::vertical()
        .max_height(max_height)
        .show(ui, add_contents);
}

impl VirtualBookApp {
    /// Render top panel menu bar
    #[cfg(not(target_arch = "wasm32"))]
    fn render_top_panel(&mut self, ui: &mut Ui) -> egui::Response {
        // Use inactive widget background for a subtle theme variant color
        let theme_variant_color = ui
            .ctx()
            .global_style()
            .visuals
            .widgets
            .inactive
            .weak_bg_fill;
        let top_panel_response = egui::Panel::top("top_panel")
            .frame(
                Frame::default()
                    .fill(theme_variant_color)
                    .inner_margin(Margin::same(4))
                    .outer_margin(Margin::same(0)),
            )
            .exact_size(32.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Left: File menu
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        let file_response = ui.button(format!(
                            "{} {}",
                            egui_phosphor::variants::regular::HOUSE_LINE,
                            self.i18n.file
                        ));
                        if file_response.clicked() {
                            if let Ok(devices) = MidiPlayerFactory::list_all_devices() {
                                self.current_devices = devices;
                            }
                        }
                        // Popup opens just under the button. Cap the menu to the
                        // space below it so a high zoom can scroll instead of clipping.
                        let file_popup_top = file_response.rect.bottom();
                        Popup::menu(&file_response).show(|ui| {
                            show_scrollable_menu(ui, file_popup_top, |ui| {
                                self.render_file_menu(ui);
                                ui.separator();
                                let preferences_popup_top = ui.cursor().top();
                                ui.menu_button(
                                    format!(
                                        "{} {}",
                                        egui_phosphor::variants::regular::GEAR_SIX,
                                        self.i18n.preferences
                                    ),
                                    |ui| {
                                        show_scrollable_menu(ui, preferences_popup_top, |ui| {
                                            self.render_preferences_menu(ui);
                                        });
                                    },
                                );

                                ui.separator();

                                self.render_midi_devices(ui);

                                ui.separator();

                                if ui.button(&self.i18n.quit).clicked() {
                                    ui.send_viewport_cmd(egui::ViewportCommand::Close);
                                }
                            });
                        });
                    });

                    // Center: Play time and title (centered)
                    ui.with_layout(Layout::top_down(Align::Center), |ui| {
                        ui.set_width(ui.available_width());
                        self.render_play_time_and_title(ui);
                    });
                });
            });

        top_panel_response.response
    }

    /// Handle input events (keyboard, mouse movement, etc.)
    fn handle_input_events(&mut self, ui: &mut egui::Ui) {
        let v = vec![
            (Key::Backspace, String::from(screen_playlist::BACKSPACE)),
            (Key::Enter, String::from(screen_playlist::ENTER)),
        ];

        let skipped_keys = vec![
            Key::Tab,
            Key::PageDown,
            Key::ArrowDown,
            Key::ArrowLeft,
            Key::ArrowRight,
            Key::ArrowUp,
        ];

        ui.input(|i| {
            if i.pointer.is_moving() {
                self.last_user_application_date = chrono::Local::now();
            }

            let difference_non_interaction = Local::now() - self.last_user_application_date;

            if self.screen != Screen::Display && self.appplayer.is_playing() {
                if let Some(timeout) = self.automatic_switch_to_display_after {
                    if difference_non_interaction.num_seconds() > timeout as i64 {
                        self.screen = Screen::Display;
                    }
                }
            }

            // top level key handling
            let mut consumed = false;
            // translate some special keys and call the screen accordingly

            if i.modifiers.alt || i.modifiers.command || i.modifiers.ctrl || i.modifiers.shift {
                return;
            }

            // History replaces the catalog filter, so keystrokes stay out of the search pad.
            if self.catalog_pane == screen_history::CatalogPane::History {
                if i.events
                    .iter()
                    .any(|event| matches!(event, Event::Key { pressed: true, .. }))
                {
                    self.last_user_application_date = chrono::Local::now();
                }
                return;
            }

            for k in skipped_keys {
                if i.key_pressed(k) {
                    return;
                }
            }

            // using space to select using the keyboard
            if i.key_pressed(Key::Space) && self.current_typed_no.is_empty() {
                return;
            }

            for k in v {
                if i.key_pressed(k.0) {
                    let no = k.1;
                    screen_playlist::handling_key(
                        &no,
                        &mut self.current_typed_no,
                        &mut self.file_store,
                        &mut self.appplayer,
                        &self.extensions_filters,
                    );
                    consumed = true;
                }
            }

            if !consumed {
                for e in i.events.iter() {
                    if let Event::Key { key, pressed, .. } = e {
                        if *pressed {
                            screen_playlist::handling_key(
                                key.name(),
                                &mut self.current_typed_no,
                                &mut self.file_store,
                                &mut self.appplayer,
                                &self.extensions_filters,
                            );

                            self.last_user_application_date = chrono::Local::now();
                            self.screen = Screen::PlayListConstruction;
                        }
                    }
                }
            }
        });
    }

    /// Render background image
    fn render_background(&mut self, ctx: &egui::Context, top_panel_bottom: f32) {
        let p = ctx.layer_painter(LayerId {
            order: Order::Background,
            id: Id::unique("source"),
        });

        if self.texture_handle.is_none() {
            let textureid = ctx.load_texture(
                "bgimage",
                self.bg_image.clone(),
                TextureOptions {
                    magnification: TextureFilter::Nearest,
                    minification: TextureFilter::Linear,
                    wrap_mode: TextureWrapMode::Repeat,
                    mipmap_mode: None,
                },
            );
            self.texture_handle = Some(textureid);
        }

        let uv = Rect {
            min: pos2(0.0, 0.0),
            max: pos2(1.0, 1.0),
        };

        // Background should fill the entire area below the top panel
        // Since it's rendered at Background layer, use screen_rect and adjust for top panel
        let mut displayed_image = ctx.content_rect();
        *displayed_image.top_mut() = top_panel_bottom;

        if let Some(t) = &self.texture_handle {
            // background image
            p.image(t.id(), displayed_image, uv, Color32::WHITE);
        }
    }

    /// Render central panel content
    fn render_central_panel(&mut self, ctx: &egui::Context, rect: Rect) {
        // `rect` is the area left over below the top panel

        // windows is the only way to have a transparent overlap in egui
        Window::new("title")
            .title_bar(false)
            .fixed_rect(rect)
            .show(ctx, |ui| {
                self.handle_input_events(ui);

                if self.screen == Screen::Display {
                    StripBuilder::new(ui)
                        .size(Size::relative(0.05))
                        .size(Size::remainder())
                        .horizontal(|mut strip| {
                            strip.cell(|ui| {
                                ui.centered_and_justified(|ui| {
                                    if ui
                                        .button(egui_phosphor::regular::PLAYLIST)
                                        .on_hover_text_at_pointer(
                                            &self.i18n.hover_retour_a_la_playlist,
                                        )
                                        .clicked()
                                    {
                                        self.screen = Screen::PlayListConstruction
                                    }
                                });
                            });
                            strip.cell(|ui| {
                                screen_visu::ui_content(self, ctx, ui);
                            });
                        });
                } else {
                    StripBuilder::new(ui)
                        .size(Size::remainder())
                        .horizontal(|mut strip| {
                            strip.cell(|ui| {
                                screen_playlist::ui_content(self, ctx, ui);
                            });
                        });
                }
            });
    }
}

/// Turn a photo of craft paper into a tileable parchment texture.
///
/// Center-crops, downsamples, then crossfades opposite edges so `Repeat`
/// wrap no longer shows hard rectangular junctions.
fn make_seamless_parchment(src: ColorImage) -> ColorImage {
    let sw = src.width();
    let sh = src.height();
    if sw < 16 || sh < 16 {
        return src;
    }

    let side = sw.min(sh);
    let x0 = (sw - side) / 2;
    let y0 = (sh - side) / 2;

    const TILE: usize = 1024;
    let mut pixels = Vec::with_capacity(TILE * TILE);
    for ty in 0..TILE {
        let sy = y0 + ty * side / TILE;
        for tx in 0..TILE {
            let sx = x0 + tx * side / TILE;
            pixels.push(src.pixels[sy * sw + sx]);
        }
    }
    let mut img = ColorImage::new([TILE, TILE], pixels);

    let feather = (TILE / 7).max(32);
    crossfade_tile_edges(&mut img, feather);
    img
}

fn lerp_color32(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let u = 1.0 - t;
    Color32::from_rgba_unmultiplied(
        (a.r() as f32 * u + b.r() as f32 * t).round() as u8,
        (a.g() as f32 * u + b.g() as f32 * t).round() as u8,
        (a.b() as f32 * u + b.b() as f32 * t).round() as u8,
        (a.a() as f32 * u + b.a() as f32 * t).round() as u8,
    )
}

fn crossfade_tile_edges(img: &mut ColorImage, feather: usize) {
    let w = img.width();
    let h = img.height();
    let f = feather.min(w / 3).min(h / 3).max(1);

    // Smoothstep-ish weight: 0 on the outer edge, 1 inward past `f`.
    let fade = |d: usize| -> f32 {
        let t = (d as f32 + 0.5) / f as f32;
        let t = t.clamp(0.0, 1.0);
        0.5 - 0.5 * (std::f32::consts::PI * t).cos()
    };

    // Horizontal: force left/right edges to meet, then restore inward detail.
    for y in 0..h {
        for d in 0..f {
            let wgt = fade(d);
            let i0 = y * w + d;
            let i1 = y * w + (w - 1 - d);
            let p0 = img.pixels[i0];
            let p1 = img.pixels[i1];
            let avg = lerp_color32(p0, p1, 0.5);
            img.pixels[i0] = lerp_color32(avg, p0, wgt);
            img.pixels[i1] = lerp_color32(avg, p1, wgt);
        }
    }

    // Vertical: same treatment (reads the already-blended horizontal edges).
    for x in 0..w {
        for d in 0..f {
            let wgt = fade(d);
            let i0 = d * w + x;
            let i1 = (h - 1 - d) * w + x;
            let p0 = img.pixels[i0];
            let p1 = img.pixels[i1];
            let avg = lerp_color32(p0, p1, 0.5);
            img.pixels[i0] = lerp_color32(avg, p0, wgt);
            img.pixels[i1] = lerp_color32(avg, p1, wgt);
        }
    }
}

/// Hand-painted square transport control. Returns true when it was clicked.
fn show_transport_button(
    ctx: &egui::Context,
    id: Id,
    pos: Pos2,
    size: f32,
    icon: &str,
    highlighted: bool,
    hover: &str,
) -> bool {
    let visuals = &ctx.global_style().visuals;
    let button_fill = if highlighted {
        visuals.selection.bg_fill
    } else {
        visuals.extreme_bg_color
    };
    let button_stroke = if highlighted {
        Stroke::new(2.0, visuals.selection.stroke.color)
    } else {
        visuals.window_stroke
    };
    let icon_color = if highlighted {
        visuals.selection.stroke.color
    } else {
        visuals.text_color()
    };

    let mut clicked = false;
    Area::new(id)
        .fixed_pos(pos)
        .movable(false)
        .order(Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(Vec2::new(size, size), Sense::click());
            ui.painter()
                .rect_filled(rect, CornerRadius::same(8), button_fill);
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(8),
                button_stroke,
                StrokeKind::Inside,
            );
            let galley = ui.fonts_mut(|fonts| {
                fonts.layout_no_wrap(icon.to_string(), FontId::proportional(20.0), icon_color)
            });
            let icon_pos = rect.center() - galley.size() / 2.0;
            ui.painter()
                .galley_with_override_text_color(icon_pos, galley, icon_color);
            clicked = response.clicked();
            response.on_hover_text_at_pointer(hover);
        });
    clicked
}

/// Time constant of the roll follow, in seconds.
const DISPLAY_FOLLOW_TAU_SECS: f64 = 0.04;
/// A playhead jump larger than this is a seek or a restore: draw it immediately.
const DISPLAY_SNAP_ERROR_MS: f64 = 300.0;
/// A frame gap larger than this means the window was not being drawn.
const DISPLAY_SNAP_DT_SECS: f64 = 0.1;

/// Move the drawn roll toward `target_ms`.
///
/// Continuous playback eases over a few frames. A skipped stretch of frames
/// (window restored, machine woken) or a jump in the playhead snaps, so the
/// roll is not animated in from the origin.
fn follow_display_offset(displayed_ms: f64, target_ms: f64, dt_secs: f64) -> f64 {
    let error = target_ms - displayed_ms;
    if !dt_secs.is_finite()
        || dt_secs >= DISPLAY_SNAP_DT_SECS
        || error.abs() > DISPLAY_SNAP_ERROR_MS
    {
        target_ms
    } else if dt_secs > 0.0 {
        let alpha = 1.0 - (-dt_secs / DISPLAY_FOLLOW_TAU_SECS).exp();
        displayed_ms + error * alpha
    } else {
        displayed_ms
    }
}

#[cfg_attr(any(feature = "profiling"), profiling::all_functions)]
impl eframe::App for VirtualBookApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    #[cfg_attr(any(feature = "profiling"), profiling::function)]
    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.setup_visuals_and_style(&ctx);

        ctx.set_pixels_per_point(self.screen_zoom_factor);

        // Anchor the playhead from the latest player message before drawing,
        // so a restored window shows the current position on the first frame.
        self.handle_player_responses();
        self.update_player_state();
        self.handle_file_dialog();
        self.update_metadata();

        #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
        let top_panel_rect = self.render_top_panel(ui).rect;
        #[cfg(target_arch = "wasm32")]
        let top_panel_rect = Rect::NOTHING;

        let top_panel_bottom = if cfg!(not(target_arch = "wasm32")) {
            top_panel_rect.bottom()
        } else {
            0.0
        };

        // Render floating play button (not part of top panel layout)
        #[cfg(not(target_arch = "wasm32"))]
        if self.appplayer.player.is_some() {
            self.render_floating_play_button(&ctx, top_panel_rect);
        }

        if self.appplayer.player.is_none() {
            // there is no player instanciated (because we need to define the output port)
            egui::CentralPanel::default().show(ui, |ui| {
                ui.label("Choisissez un périphérique de sortie dans le menu Fichier");
            });
        } else {
            // area left below the top panel
            let central_rect = ui.available_rect_before_wrap();
            egui::CentralPanel::default().show(ui, |_ui| {
                self.render_background(&ctx, top_panel_bottom);
            });
            self.render_central_panel(&ctx, central_rect);
        }
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod display_offset_tests {
    use super::follow_display_offset;

    #[test]
    fn snaps_when_the_window_was_not_drawn() {
        // Two seconds hidden, playhead only 200 ms ahead: still show it now.
        assert_eq!(follow_display_offset(10_000.0, 10_200.0, 2.0), 10_200.0);
    }

    #[test]
    fn snaps_from_the_origin_when_the_playhead_is_already_ahead() {
        assert_eq!(follow_display_offset(0.0, 120_000.0, 0.016), 120_000.0);
    }

    #[test]
    fn eases_one_frame_of_steady_playback() {
        let shown = follow_display_offset(1_000.0, 1_016.0, 0.016);
        assert!(
            shown > 1_000.0 && shown < 1_016.0,
            "steady playback should ease, shown = {shown}"
        );
    }

    #[test]
    fn holds_when_no_time_has_passed() {
        assert_eq!(follow_display_offset(500.0, 800.0, 0.0), 500.0);
    }
}
