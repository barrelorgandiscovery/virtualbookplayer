use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::epaint::*;
use egui::*;
use egui_extras::image::load_image_bytes;
use egui_extras::{Size, StripBuilder};
use im_native_dialog::ImNativeFileDialog;
use player::midiio::{DeviceInformation, MidiPlayerFactory};
use player::{PlayerFactory, Response};

use crate::appplayer::AppPlayer;
use crate::file_store::*;

use log::error;

use self::i18n::{I18NMessages, create_i18n_fr_message};

mod screen_playlist;
mod screen_visu;
mod i18n;


#[path = "frame_history.rs"]
mod frame_history;

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
    screen_zoom_factor: f32,

    xscale: f32,

    #[serde(skip)]
    offset: f32,

    #[serde(skip)]
    screen: Screen,

    #[serde(skip)]
    frame_history: frame_history::FrameHistory,


    #[serde(skip)]
    file_path_dialog: ImNativeFileDialog<Option<PathBuf>>,
    
    file_store_path: Option<PathBuf>,

    #[serde(skip)]
    file_store: Option<FileStore>,

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

    selected_device: usize,

    #[serde(skip)]
    latest_duration_time: Duration,
    #[serde(skip)]
    adjusted_start_time: Instant,

    #[serde(skip)]
    i18n: Box<I18NMessages>,
}

impl Default for VirtualBookApp {
    fn default() -> Self {
        let mut appplayer = AppPlayer::new();

        appplayer.player(None);

        let img = load_image_bytes(include_bytes!("bg2.png")).unwrap();

        Self {
            frame_history: frame_history::FrameHistory::default(),
            offset: 0.0,
            xscale: 3_000f32,
            screen: Screen::PlayListConstruction,
            screen_zoom_factor: 2.0,

            current_typed_no: "".into(),

            
            file_path_dialog: im_native_dialog::ImNativeFileDialog::default(),
            file_store_path: None,
            file_store: None,

            bg_image: img,
            texture_handle: None,

            appplayer,
            current_duration: Duration::new(0, 0),
            current_devices: vec![],
            selected_device: 0,

            latest_duration_time: Duration::new(0, 0),
            adjusted_start_time: Instant::now(),

            i18n: create_i18n_fr_message(),
        }
    }
}

impl VirtualBookApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        let mut fonts = FontDefinitions::default();

        // Install my own font (maybe supporting non-latin characters):
        fonts.font_data.insert(
            "my_font".to_owned(),
            FontData::from_static(include_bytes!("../../fonts/Rubik-VariableFont_wght.ttf")),
        ); // .ttf and .otf supported

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

            let (scmd, rcmd) = channel();
            let (s, r) = channel();

            match factory.create(s, rcmd) {
                Ok(player) => {
                    old_storage.appplayer.player(Some((player, r)));
                }
                Err(e) => {
                    error!("fail to open device {}", e);
                }
            }

            if let Some(path) = &old_storage.file_store_path {
                if let Ok(storage_created) = FileStore::new(&PathBuf::from(path)) {
                    old_storage.file_store = Some(storage_created);
                }
            }

            return old_storage;
        }

        let app: VirtualBookApp = Default::default();

        app
    }
}

impl eframe::App for VirtualBookApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // self.frame_history
        //     .on_new_frame(ctx.input(|i| i.time), _frame.info().cpu_usage);

        let mut style = Style::default();

        style.visuals.window_fill = Color32::from_rgba_premultiplied(0, 0, 0, 30);
        style.visuals.window_rounding = Rounding::same(0.0);

        style.visuals.panel_fill = Color32::from_rgba_premultiplied(0, 0, 0, 10);
        style.visuals.extreme_bg_color = Color32::from_rgba_premultiplied(0, 0, 0, 10);
        style.visuals.faint_bg_color = Color32::from_rgba_premultiplied(0, 0, 0, 10);

        ctx.set_style(style);

        let Self {
            appplayer,
            screen_zoom_factor,
            current_duration,
            selected_device,
            latest_duration_time,
            adjusted_start_time,
            file_store_path,
            file_store,
            file_path_dialog,
            i18n,
            ..
        } = self;

        ctx.set_pixels_per_point(*screen_zoom_factor);

        let last_response_arc = Arc::clone(&appplayer.last_response);

        // handling smooth
        if appplayer.is_playing() {
            if *adjusted_start_time + Duration::from_millis(100) < Instant::now() {
                let delta = Instant::now().duration_since(*adjusted_start_time);
                if let Some(vb) = &appplayer.vb {
                    if let Some(max_time) = vb.max_time() {
                        *current_duration = delta;
                        self.offset = delta.as_micros() as f32 / max_time as f32;
                    }
                }
            }
        }

        // handling messages
        if let Ok(mut opt_last_response) = last_response_arc.lock() {
            if opt_last_response.is_some() {
                let last_response = opt_last_response.as_mut().unwrap();
                // println!("command received : {:?}", &last_response);
                match *last_response {
                    Response::EndOfFile => {
                        appplayer.next();
                    }
                    Response::Current_Play_Time(duration) => {
                        *latest_duration_time = duration;
                        *adjusted_start_time = Instant::now() - duration;
                    }
                    Response::EndOfTrack => {}
                    Response::StartOfTrack => {}
                    Response::FileCancelled => {}
                }
                *opt_last_response = None;
            }
        }

        if let Some(Ok(result)) = file_path_dialog.check() {
            *file_store_path = result.clone();
            if let Some(r) = result {
                if let Ok(fs) = FileStore::new(&r) {
                    *file_store = Some(fs);
                } else {
                    error!(
                        "fail to create file store with path {:?}",
                        r.clone()
                    )
                }
            }
        }

        // Examples of how to create different panels and windows.
        // Pick whichever suits you.
        // Tip: a good default choice is to just keep the `CentralPanel`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
        let top_response = egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            let old = ui.visuals().clone();
            let mut visual_mut = old.clone();
            visual_mut.window_fill = Color32::BLACK;
            ctx.set_visuals(visual_mut);

            egui::menu::bar(ui, |ui| {
                if ui
                    .menu_button(&i18n.file, |ui| {
                        if ui.button(&i18n.open_folder).clicked() {
                            
                            let location: Option<PathBuf> = self.file_store_path.clone();

                           
                            //let repaint_signal = ctx.repaint_signal();
                            if let Err(_result_open_signle_dir) =
                                file_path_dialog.open_single_dir(location)
                            {
                                error!("fail to open dir dialog");                                
                            }

                            ui.close_menu();
                        }
                        ui.separator();

                        ui.label("midi out interfaces");
                        for device in &self.current_devices {
                            let selected = *selected_device == device.no;
                            if ui.radio(selected, &device.label).clicked() {

                                if let Some(old_player) = &appplayer.player {
                                    
                                }


                                println!("Open the device");
                                *selected_device = device.no;

                                let factory = MidiPlayerFactory {
                                    device_no: *selected_device,
                                };

                                let (scmd, rcmd) = channel();
                                let (s, r) = channel();

                                match factory.create(s, rcmd) {
                                    Ok(player) => {
                                        appplayer.player(Some((player, r)));
                                    }
                                    Err(e) => {
                                        error!("fail to open device {}", e);
                                    }
                                }
                            }
                        }

                        ui.separator();

                        if ui.button(&i18n.quit).clicked() {
                            _frame.close();
                        }
                    })
                    .response
                    .clicked()
                {
                    if let Ok(devices) = MidiPlayerFactory::list_all_devices() {
                        self.current_devices = devices;
                    }
                }

                ui.menu_button(&i18n.display, |ui| {
                    ui.label(&i18n.zoom);
                    ui.add(egui::Slider::new(screen_zoom_factor, 0.5..=4.0));
                });

                if ui.toggle_value(&mut appplayer.play_mod, &i18n.play).clicked() {
                    if appplayer.play_mod {
                        appplayer.play_file_on_top();
                    } else {
                        appplayer.stop();
                    }
                }

                if appplayer.is_playing() {
                    let cell = &appplayer.playlist.current();
                    match cell {
                        Some(t) => {
                            let name = t.name.clone();
                            let mut rt = RichText::new(format!(" ➡ {} ⬅ ", name));

                            rt = rt.background_color(ui.style().visuals.selection.bg_fill);
                            rt = rt.color(ui.style().visuals.selection.stroke.color);

                            ui.label(rt.monospace());
                        }
                        None => {}
                    }

                    ui.label(format!("{:.0}s", &current_duration.as_secs_f32()));
                }
            });
            ctx.set_visuals(old);
        });

        if appplayer.player.is_some() {
            egui::CentralPanel::default().show(ctx, |ui| {
                let p = ctx.layer_painter(LayerId {
                    order: Order::Background,
                    id: Id::new("source"),
                });

                if self.texture_handle.is_none() {
                    let textureid = ctx.load_texture(
                        "bgimage",
                        self.bg_image.clone(),
                        TextureOptions {
                            magnification: TextureFilter::Nearest,
                            minification: TextureFilter::Linear,
                        },
                    );
                    self.texture_handle = Some(textureid);
                }

                let uv = Rect {
                    min: pos2(0.0, 0.0),
                    max: pos2(1.0, 1.0),
                };
                //mesh.add_rect_with_uv(ctx.screen_rect(), uv, Color32::WHITE);
                let mut displayed_image = ctx.screen_rect();
                *displayed_image.top_mut() += top_response.response.rect.bottom();

                if let Some(t) = &self.texture_handle {
                    p.image(t.id(), displayed_image, uv, Color32::WHITE);
                }
                let mut rect = ctx.screen_rect().clone();
                *rect.top_mut() += top_response.response.rect.bottom();
                *rect.bottom_mut() -= top_response.response.rect.bottom()
                    - (ui.style().spacing.window_margin.bottom
                        + ui.style().spacing.window_margin.top);

                Window::new("title")
                    .title_bar(false)
                    .fixed_rect(rect)
                    .show(ctx, |ui| {
                        // ui.group(|ui| {
                        if self.screen == Screen::Display {
                            // The central panel the region left after adding TopPanel's and SidePanel's
                            // print fps
                            // self.frame_history.ui(ui);

                            StripBuilder::new(ui)
                                .size(Size::relative(0.05))
                                .size(Size::remainder())
                                .horizontal(|mut strip| {
                                    strip.cell(|ui| {
                                        ui.centered_and_justified(|ui| {
                                            if ui.button("<").clicked() {
                                                self.screen = Screen::PlayListConstruction
                                            }
                                        });
                                    });
                                    strip.cell(|ui| {
                                        screen_visu::ui_content(self, ctx, ui);
                                    });
                                });
                        } else {
                            // The central panel the region left after adding TopPanel's and SidePanel's
                            // print fps
                            // self.frame_history.ui(ui);

                            StripBuilder::new(ui)
                                .size(Size::remainder())
                                .size(Size::relative(0.05))
                                .horizontal(|mut strip| {
                                    strip.cell(|ui| {
                                        screen_playlist::ui_content(self, ctx, ui);
                                    });

                                    strip.cell(|ui| {
                                        ui.centered_and_justified(|ui| {
                                            if ui.button(">").clicked() {
                                                self.screen = Screen::Display
                                            }
                                        });
                                    });
                                });
                        }
                    });
            });
        }
        ctx.request_repaint();
    }
}
