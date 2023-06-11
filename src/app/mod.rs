use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;

use egui::epaint::*;
use egui::*;
use egui_extras::{Size, StripBuilder};
use player::midiio::{DeviceInformation, MidiPlayerFactory};
use player::{PlayerFactory, Response};

use crate::appplayer::AppPlayer;
use crate::file_store::*;

mod screen_playlist;
mod screen_visu;
use log::error;

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
pub struct TemplateApp {
    screen_zoom_factor: f32,

    xscale: f32,

    #[serde(skip)]
    offset: f32,

    #[serde(skip)]
    screen: Screen,

    #[serde(skip)]
    frame_history: frame_history::FrameHistory,

    #[serde(skip)]
    file_store: FileStore,

    #[serde(skip)]
    current_typed_no: String,

    #[serde(skip)]
    appplayer: AppPlayer,

    #[serde(skip)]
    current_duration: Duration,

    #[serde(skip)]
    current_devices: Vec<DeviceInformation>,

    selected_device: usize,
}

impl Default for TemplateApp {
    fn default() -> Self {
        // let mut r = BufReader::new(File::open("52 Surprise fox2.book").unwrap());
        // let vb = Some(Arc::new(Box::new(
        //     bookparsing::read_book_stream(&mut r).unwrap(),
        // )));

        let mut appplayer = AppPlayer::new();

        appplayer.player(None);

        //let vb = VirtualBookComponent::default();
        Self {
            frame_history: frame_history::FrameHistory::default(),
            offset: 0.0,
            xscale: 3_000f32,
            screen: Screen::PlayListConstruction,
            screen_zoom_factor: 2.0,

            current_typed_no: "".into(),
            file_store: FileStore::new(&PathBuf::from(
                "/home/use/projets/2022-02_Orgue_Electronique/work/mpy-orgue/files",
            ))
            .unwrap(),
            appplayer,
            current_duration: Duration::new(0, 0),
            current_devices: vec![],
            selected_device: 0,
        }
    }
}

impl TemplateApp {
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

        cc.egui_ctx.set_fonts(fonts);

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            let mut old_storage : Self = eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
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


            return old_storage;
        }

        Default::default()
    }

    // pub fn open_from_string_content(&mut self, file_content_string: String) -> std::io::Result<()> {
    //     self.vb.open_from_string_content(file_content_string)?;
    //     Ok(())
    // }
}

impl eframe::App for TemplateApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_history
            .on_new_frame(ctx.input(|i| i.time), _frame.info().cpu_usage);

        let Self {
            frame_history,
            xscale,
            offset,
            screen,
            file_store,
            current_typed_no,
            appplayer,
            screen_zoom_factor,
            current_duration,
            current_devices,
            selected_device,
        } = self;

        let last_response_arc = Arc::clone(&appplayer.last_response);
        // handling messages
        if let Ok(mut opt_last_response) = last_response_arc.lock() {
            if opt_last_response.is_some() {
                let last_response = opt_last_response.as_mut().unwrap();
                // println!("command received : {:?}", &last_response);
                match *last_response {
                    Response::EndOfFile => {
                        &appplayer.next();
                    }
                    Response::Current_Play_Time(duration) => {
                        *current_duration = duration;
                        if let Some(vb) = &appplayer.vb {
                            if let Some(max_time) = vb.max_time() {
                                self.offset = duration.as_micros() as f32 / max_time as f32;
                            }
                        }
                    }
                    Response::EndOfTrack => {}
                    Response::StartOfTrack => {}
                    Response::FileCancelled => {}
                }
                *opt_last_response = None;
            }
        }

        // Examples of how to create different panels and windows.
        // Pick whichever suits you.
        // Tip: a good default choice is to just keep the `CentralPanel`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                if ui
                    .menu_button("File", |ui| {
                        if ui.button("Choose directory ..").clicked() {}
                        ui.separator();

                        ui.label("midi out interfaces");
                        for device in &self.current_devices {
                            let mut selected = *selected_device == device.no;
                            if ui.radio(selected, &device.label).clicked() {
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

                        if ui.button("Quit").clicked() {
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

                ui.menu_button("Display", |ui| {
                    ui.label("Zoom :");
                    ui.add(egui::Slider::new(screen_zoom_factor, 0.5..=4.0));
                });

                if ui.toggle_value(&mut appplayer.play_mod, "Play").clicked() {
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
                            let name = t.as_ref().borrow().name.clone();
                            let mut rt = RichText::new(format!(" ➡ {} ⬅", name));
                            rt = rt.color(Color32::RED);

                            ui.label(rt);
                        }
                        None => {}
                    }

                    ui.label(format!("{:.0}s", &current_duration.as_secs_f32()));
                }

                let mut has_next_file = false;
                let mut next_file_name: String = String::from("");
                if appplayer.playlist.file_list.len() > 1 {
                    let next_element = appplayer.playlist.file_list.get(1);
                    match next_element {
                        Some(t) => {
                            next_file_name = t.as_ref().borrow().name.clone();
                            has_next_file = true;
                        }
                        None => {}
                    }
                }
                if has_next_file {
                    if ui
                        .button(format!("Next file : {}", &next_file_name))
                        .clicked()
                    {
                        appplayer.next();
                    }
                }
            });
        });
        ctx.set_pixels_per_point(*screen_zoom_factor);
        if appplayer.player.is_some() {
            egui::CentralPanel::default().show(ctx, |ui| {
                // The central panel the region left after adding TopPanel's and SidePanel's
                // print fps
                // self.frame_history.ui(ui);

                // ui.group(|ui| {
                if self.screen == Screen::Display {
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
        }
    }
}
