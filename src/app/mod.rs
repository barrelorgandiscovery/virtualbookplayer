use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::Arc;
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
use crate::{duration_to_mm_ss, file_store::*};

use log::{debug, error, info};

use self::i18n::{create_i18n_message_with_lang, I18NMessages};

use pid_lite::Controller;

use egui_extras_xt::displays::IndicatorButton;

mod i18n;
mod screen_playlist;
mod screen_visu;

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

    /// offset in the play (in seconds)
    #[serde(skip)]
    offset_ms: f64,

    #[serde(skip)]
    pid_regulated_offset_ms: f64,
    #[serde(skip)]
    pid_controller: Controller,

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

    selected_device: usize,

    #[serde(skip)]
    latest_duration_time: Duration,
    #[serde(skip)]
    adjusted_start_time: Instant,

    //date code when the user has the last interaction with the application
    #[serde(skip)]
    last_user_application_date: DateTime<Local>,

    // automatic switch for display the whole
    automatic_switch_to_display_after: Option<u16>,

    /// associated language strings
    #[serde(skip)]
    i18n: Box<I18NMessages>,

    islight: bool,

    play_lattency_ms: i64,

    /// display the number pad
    hidden_number_pad: bool,

    /// wait time before playing the file
    play_wait: f32,

    /// selected language
    lang: Option<String>,
}

impl Default for VirtualBookApp {
    fn default() -> Self {
        let mut appplayer = AppPlayer::new();

        appplayer.player(None);

        let img: ColorImage = load_image_bytes(include_bytes!("bg2.png")).unwrap();
        let imgbackground = load_image_bytes(include_bytes!("../../assets/craft.png")).unwrap();

        Self {
            lang: None,

            offset_ms: 0.0,
            pid_regulated_offset_ms: 0.0,
            pid_controller: Controller::new(0.0, 0.30, 0.010, 0.05),

            xscale: 3_000f64,
            screen: Screen::PlayListConstruction,
            screen_zoom_factor: 2.0,
            slider_selected_zoom_factor: 2.0,

            // filter
            current_typed_no: "".into(),

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

            play_lattency_ms: 0, // 0 ms lattency

            i18n: create_i18n_message_with_lang(None),

            islight: false,

            hidden_number_pad: false,

            extensions_filters: Some(vec![".mid".into(), ".book".into(), ".playlist".into()]),

            play_wait: 2.0,

            last_user_application_date: chrono::Local::now(),
            automatic_switch_to_display_after: Some(10),
        }
    }
}

impl VirtualBookApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>, reset: bool, lang: Option<String>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        let mut fonts = egui::FontDefinitions::default();

        // Install my own font (maybe supporting non-latin characters):
        fonts.font_data.insert(
            "my_font".to_owned(),
            FontData::from_static(include_bytes!("../../fonts/Rubik-VariableFont_wght.ttf")),
        ); // .ttf and .otf supported

        fonts.font_data.insert(
            "icon_font".to_owned(),
            FontData::from_static(include_bytes!("../../fonts/fa-solid-900.ttf")),
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

        let v = vec!["icon_font".to_owned()];

        fonts
            .families
            .insert(FontFamily::Name("icon_font".into()), v);

        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

        cc.egui_ctx.set_fonts(fonts);

        if !reset {
            // user ask to reset the stored values

            // Load previous app state (if any).
            // Note that you must enable the `persistence` feature for this to work.
            if let Some(storage) = cc.storage {
                let mut old_storage: Self =
                    eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();

                // reopen the selected midi port
                // reopening
                let factory = MidiPlayerFactory {
                    device_no: old_storage.selected_device,
                };

                let (_scmd, rcmd) = channel();
                let (s, r) = channel();

                match factory.create(s, rcmd) {
                    Ok(player) => {
                        old_storage.appplayer.player(Some((player, r, _scmd)));
                    }
                    Err(e) => {
                        error!("fail to open device {}", e);
                    }
                }

                if let Some(path) = &old_storage.file_store_path {
                    match FileStore::new(&PathBuf::from(path)) {
                        Ok(mut storage_created) => {
                            if storage_created.is_some() {
                                if let Some(mut fs) = storage_created {
                                    if let Ok(v) = fs.view(&None, &old_storage.extensions_filters) {
                                        fs.default_view = Some(v);
                                    }
                                    storage_created = Some(fs);
                                }
                            }
                            old_storage.file_store = storage_created;
                        }
                        Err(e) => {
                            error!("error in opening the path {}", &e);
                            old_storage.file_store = None;
                        }
                    }
                }
                old_storage.lang.clone_from(&lang);
                old_storage.i18n = create_i18n_message_with_lang(lang);

                old_storage.screen_zoom_factor = old_storage.slider_selected_zoom_factor;

                // define the wait time on restoration
                old_storage
                    .appplayer
                    .set_waittime_between_file_play(old_storage.play_wait);

                return old_storage;
            }
        }

        VirtualBookApp {
            lang: lang.clone(),
            i18n: create_i18n_message_with_lang(lang),
            ..Default::default()
        }
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
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let old = if self.islight {
            Visuals::light()
        } else {
            Visuals::dark()
        };

        ctx.set_visuals(old);

        let mut style: Style = Style::default();
        style.clone_from(&ctx.style());

        let mut c = style.visuals.window_fill;
        c = Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), 60);
        style.visuals.window_fill = c;
        style.visuals.window_rounding = Rounding::same(0.0);

        ctx.set_style(style);

        let Self {
            appplayer,
            slider_selected_zoom_factor,
            screen_zoom_factor,
            current_duration,
            selected_device,
            latest_duration_time,
            adjusted_start_time,
            file_store_path,
            file_store,
            file_path_dialog,
            i18n,
            islight,
            hidden_number_pad,
            extensions_filters,
            play_wait,
            play_lattency_ms,
            automatic_switch_to_display_after,
            ..
        } = self;

        ctx.set_pixels_per_point(*screen_zoom_factor);

        let last_response_arc = Arc::clone(&appplayer.last_response);

        // // handling smooth
        if appplayer.is_playing()
            && *adjusted_start_time + Duration::from_millis(100) < Instant::now()
        // evaluated every 100ms, if the events
        {
            let delta = Instant::now().duration_since(*adjusted_start_time);
            if let Some(_vb) = appplayer.virtual_book.read().as_deref() {
                *current_duration = delta;
                self.offset_ms = delta.as_millis() as f64;
                self.pid_controller
                    .set_target(self.offset_ms - *play_lattency_ms as f64);
            }
        }

        // compute smoothed values for nice display
        //self.pid_regulated_offset
        self.pid_regulated_offset_ms = self.pid_controller.update(self.pid_regulated_offset_ms);

        // handling messages
        if let Ok(mut opt_last_response) = last_response_arc.lock() {
            if opt_last_response.is_some() {
                let last_response = opt_last_response.as_mut().unwrap();
                match &*last_response {
                    Response::EndOfFile => {
                        appplayer.next();
                    }
                    Response::CurrentPlayTime(duration) => {
                        *latest_duration_time = *duration;
                        *adjusted_start_time = Instant::now() - *duration;

                        // depending on the midi control, some may have a
                        // time shift
                        // accordingly,

                        self.pid_controller.set_target(
                            ((*duration).as_micros() as f64 + *play_lattency_ms as f64 * 1000.0)
                                / 1000.0,
                        );
                    }
                    Response::FileCancelled => {}
                    Response::FilePlayStarted((_filename, _notes)) => {}
                }
                *opt_last_response = None;
            }
        }

        // Open folder Dialog response
        if let Some(Ok(result)) = file_path_dialog.check() {
            file_store_path.clone_from(&result);
            if let Some(r) = result {
                match FileStore::new(&r) {
                    Ok(fs) => {
                        *file_store = fs;
                        debug!("folder opened, reapply the elements and views");
                        // refilter the view using the filters
                        if let Some(store) = file_store {
                            if let Ok(result) = store.view(&None, extensions_filters) {
                                store.default_view = Some(result);
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

        #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
        let top_response = egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:

            let old = ui.visuals().clone();
            let mut visual_mut = old.clone();

            let mut c = visual_mut.window_fill;
            c = Color32::from_rgb(c.r(), c.g(), c.b());
            visual_mut.window_fill = c;

            ctx.set_visuals(visual_mut);

            // file menu
            egui::menu::bar(ui, |ui| {
                #[allow(clippy::blocks_in_conditions)]
                if ui
                    .menu_button(
                        format!(
                            "{} {}",
                            egui_phosphor::variants::regular::HOUSE_LINE,
                            &i18n.file
                        ),
                        |ui| {
                            if ui
                                .button(format!(
                                    "{} {}",
                                    egui_phosphor::regular::FOLDER_OPEN,
                                    &i18n.open_folder
                                ))
                                .clicked()
                            {
                                let mut location: Option<PathBuf> = self.file_store_path.clone();
                                if let Some(loc) = &location {
                                    // check l,ocation exists
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

                                //let repaint_signal = ctx.repaint_signal();
                                if let Err(_result_open_single_dir) =
                                    file_path_dialog.open_single_dir(location)
                                {
                                    error!("fail to open dir dialog");
                                } else {
                                    info!("dialog opened");
                                }

                                ui.close_menu();
                            }
                            ui.separator();
                            if ui
                                .button(format!(
                                    "{} {}",
                                    egui_phosphor::regular::RECYCLE,
                                    &i18n.reload_folder
                                ))
                                .clicked()
                            {
                                if let Some(current_path) = &self.file_store_path {
                                    let new_filestore = FileStore::new(current_path);
                                    if let Ok(new_store) = new_filestore {
                                        self.file_store = new_store.map(|mut fs| {
                                            if let Ok(v) = fs.view(&None, &self.extensions_filters)
                                            {
                                                fs.default_view = Some(v);
                                            }
                                            fs
                                        });
                                    }
                                }
                                ui.close_menu();
                            };

                            ui.separator();
                            // preferences menu
                            ui.menu_button(
                                format!(
                                    "{} {}",
                                    egui_phosphor::variants::regular::GEAR_SIX,
                                    &i18n.preferences
                                ),
                                |ui| {
                                    ui.label(&i18n.zoom);
                                    let result = ui.add(egui::Slider::new(
                                        slider_selected_zoom_factor,
                                        1.5..=6.0,
                                    ));
                                    if !result.is_pointer_button_down_on() {
                                        *screen_zoom_factor = *slider_selected_zoom_factor;
                                    };

                                    ui.checkbox(hidden_number_pad, &i18n.hide_num_pad);
                                    ui.checkbox(islight, &i18n.dark_light);
                                    ui.label(&i18n.time_between_file);
                                    let time_slider = egui::Slider::new(play_wait, 0.0..=30.0);
                                    if time_slider.ui(ui).changed() {
                                        appplayer.set_waittime_between_file_play(*play_wait);
                                    }
                                    ui.separator();
                                    let mut hasvalue = automatic_switch_to_display_after.is_some();
                                    if ui.checkbox(&mut hasvalue, &i18n.switch_auto).changed() {
                                        if !hasvalue {
                                            *automatic_switch_to_display_after = None;
                                        } else {
                                            *automatic_switch_to_display_after = Some(10);
                                        }
                                    }

                                    if let Some(value) = automatic_switch_to_display_after {
                                        let automatic_switch_value =
                                            egui::Slider::new(value, 5..=300);
                                        ui.add(automatic_switch_value);
                                    }

                                    ui.separator();

                                    ui.label(&i18n.lattence_jeu);
                                    let play_lattency_slider =
                                        egui::Slider::new(play_lattency_ms, -1000..=4_000);
                                    ui.add(play_lattency_slider);
                                },
                            );

                            ui.separator();

                            ui.label("midi out interfaces");
                            for device in &self.current_devices {
                                let selected = *selected_device == device.no;
                                if ui.radio(selected, &device.label).clicked() {
                                    if let Some(_old_player) = &appplayer.player {}

                                    println!("Open the device");
                                    *selected_device = device.no;

                                    let factory = MidiPlayerFactory {
                                        device_no: *selected_device,
                                    };

                                    let (_scmd, rcmd) = channel();
                                    let (s, player_event_receiver) = channel();

                                    match factory.create(s, rcmd) {
                                        Ok(player) => {
                                            // change the player
                                            appplayer.player(Some((
                                                player,
                                                player_event_receiver,
                                                _scmd,
                                            )));
                                        }
                                        Err(e) => {
                                            error!("fail to open device {}", e);
                                        }
                                    }

                                    ui.close_menu();
                                }
                            }

                            ui.separator();

                            if ui.button(&i18n.quit).clicked() {
                                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                            }
                        },
                    )
                    .response
                    .clicked()
                {
                    if let Ok(devices) = MidiPlayerFactory::list_all_devices() {
                        self.current_devices = devices;
                    }
                }

                let play_mod = &mut appplayer.play_mod;
                let play_mod_value = *play_mod;

                let indicator_play_button = IndicatorButton::new(play_mod)
                    .label(if !play_mod_value {
                        egui_phosphor::regular::PLAY
                    } else {
                        egui_phosphor::regular::STOP
                    })
                    .width(32.0)
                    .height(24.0);

                let indicator_play_response = ui.add(indicator_play_button);

                if indicator_play_response.changed() {
                    if *play_mod {
                        appplayer.play_file_on_top();
                    } else {
                        appplayer.stop();
                    }
                }

                indicator_play_response
                    .on_hover_text_at_pointer(&i18n.hover_activate_the_play_of_the_playlist);

                /*
                if ui.add(Button::new("pause")).clicked() {
                    appplayer.pause();
                }
                */

                // playing title

                if appplayer.is_playing() {
                    let current_playlist =
                        &appplayer.playlist.lock().expect("fail to lock playlist");
                    let cell = current_playlist.current();
                    if let Some(t) = cell {
                        let name = t.name.clone();
                        let mut rt = RichText::new(format!(" ➡ {} ⬅ ", name));
                        rt = rt.font(FontId::proportional(12.0));
                        rt = rt.background_color(ui.style().visuals.selection.bg_fill);
                        rt = rt.color(ui.style().visuals.selection.stroke.color);

                        ui.horizontal(|ui| {
                            let mut total_duration = String::from("-");
                            if let Some(duration) = current_playlist.computed_length {
                                total_duration = duration_to_mm_ss(&duration);
                            }

                            let mut current_file_remaining_duration = String::from("");
                            if let Some(current_play) = current_playlist.current() {
                                if let Some(additional_info) = current_play.additional_informations
                                {
                                    if let Some(dur) = additional_info.duration {
                                        if dur > *current_duration {
                                            let remaining_current = dur - *current_duration;

                                            current_file_remaining_duration =
                                                duration_to_mm_ss(&remaining_current);
                                        }
                                    }
                                }
                            }

                            ui.label(format!(
                                "{} / {}",
                                // duration_to_mm_ss(current_duration),
                                current_file_remaining_duration,
                                total_duration
                            ));

                            ui.label(rt.monospace());
                        });
                    }
                }
            });
            ctx.set_visuals(old);
        });

        let present = appplayer.player.is_some();
        if !present {
            // there is no player instanciated (because we need to define the output port)
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.label("Choisissez un périphérique de sortie dans le menu Fichier");
            });
        } else {
            egui::CentralPanel::default().show(ctx, |ui| {
                // borrow checker clarification for the closure
                let self1 = self;
                let extensions_filter = &self1.extensions_filters;
                let file_store1 = &mut self1.file_store;
                let current_typed_no1 = &mut self1.current_typed_no;
                let last_user_interaction = &mut self1.last_user_application_date;
                let screen = &mut self1.screen;
                let automatic_switch_to_display_after =
                    &mut self1.automatic_switch_to_display_after;
                {
                    let appplayer = &mut self1.appplayer;

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
                            *last_user_interaction = chrono::Local::now();
                        }

                        let difference_non_interaction = Local::now() - *last_user_interaction;

                        if *screen != Screen::Display && appplayer.is_playing() {
                            if let Some(timeout) = automatic_switch_to_display_after {
                                if difference_non_interaction.num_seconds() > *timeout as i64 {
                                    *screen = Screen::Display;
                                }
                            }
                        }

                        // top level key handling

                        let mut consumed = false;
                        // translate some special keys and call the screen accordingly

                        if i.modifiers.alt
                            || i.modifiers.command
                            || i.modifiers.ctrl
                            || i.modifiers.shift
                        {
                            return;
                        }

                        for k in skipped_keys {
                            if i.key_pressed(k) {
                                return;
                            }
                        }

                        // using space to select using the keyboard
                        if i.key_pressed(Key::Space) && current_typed_no1.is_empty() {
                            return;
                        }

                        for k in v {
                            if i.key_pressed(k.0) {
                                let no = k.1;
                                screen_playlist::handling_key(
                                    &no,
                                    current_typed_no1,
                                    file_store1,
                                    appplayer,
                                    extensions_filter,
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
                                            current_typed_no1,
                                            file_store1,
                                            appplayer,
                                            extensions_filter,
                                        );

                                        *last_user_interaction = chrono::Local::now();
                                        *screen = Screen::PlayListConstruction;
                                    }
                                }
                            }
                        }
                    });
                }
                let p = ctx.layer_painter(LayerId {
                    order: Order::Background,
                    id: Id::new("source"),
                });

                if self1.texture_handle.is_none() {
                    let textureid = ctx.load_texture(
                        "bgimage",
                        self1.bg_image.clone(),
                        TextureOptions {
                            magnification: TextureFilter::Nearest,
                            minification: TextureFilter::Linear,
                            wrap_mode: TextureWrapMode::Repeat,
                        },
                    );
                    self1.texture_handle = Some(textureid);
                }

                let uv = Rect {
                    min: pos2(0.0, 0.0),
                    max: pos2(1.0, 1.0),
                };

                let mut displayed_image = ctx.screen_rect();
                *displayed_image.top_mut() += top_response.response.rect.bottom();

                if let Some(t) = &self1.texture_handle {
                    // background image
                    p.image(t.id(), displayed_image, uv, Color32::WHITE);
                }
                let mut rect = ctx.screen_rect();
                *rect.top_mut() += top_response.response.rect.bottom();
                *rect.bottom_mut() -= top_response.response.rect.bottom()
                    - (ui.style().spacing.window_margin.bottom
                        + ui.style().spacing.window_margin.top);

                // windows is the only way to have a transparent overlap in egui
                Window::new("title")
                    .title_bar(false)
                    .fixed_rect(rect)
                    .show(ctx, |ui| {
                        // ui.group(|ui| {
                        if self1.screen == Screen::Display {
                            // The central panel the region left after adding TopPanel's and SidePanel's
                            // print fps
                            // self.frame_history.ui(ui);

                            StripBuilder::new(ui)
                                .size(Size::relative(0.05))
                                .size(Size::remainder())
                                .horizontal(|mut strip| {
                                    strip.cell(|ui| {
                                        ui.centered_and_justified(|ui| {
                                            if ui
                                                .button(egui_phosphor::regular::PLAYLIST)
                                                .on_hover_text_at_pointer(
                                                    &self1.i18n.hover_retour_a_la_playlist,
                                                )
                                                .clicked()
                                            {
                                                self1.screen = Screen::PlayListConstruction
                                            }
                                        });
                                    });
                                    strip.cell(|ui| {
                                        screen_visu::ui_content(self1, ctx, ui);
                                    });
                                });
                        } else {
                            // The central panel the region left after adding TopPanel's and SidePanel's
                            // print fps
                            // self.frame_history.ui(ui);

                            StripBuilder::new(ui)
                                .size(Size::remainder())
                                // .size(Size::relative(0.05))
                                .horizontal(|mut strip| {
                                    strip.cell(|ui| {
                                        screen_playlist::ui_content(self1, ctx, ui);
                                    });
                                });
                        }
                    });
            });
        }
        ctx.request_repaint();
    }
}
