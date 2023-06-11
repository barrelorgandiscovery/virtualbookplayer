use std::borrow::BorrowMut;
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;

use bookparsing::{read_book_stream, VirtualBook};
use egui::epaint::*;
use egui::*;
use egui_extras::{Size, StripBuilder};
use player::midiio::{MidiPlayer, MidiPlayerFactory};
use player::{Player, PlayerFactory, Response};

use crate::appplayer::AppPlayer;
use crate::file_store::*;
use crate::playlist::PlayList;
use crate::virtualbookcomponent::*;

mod screen_playlist;
mod screen_visu;

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
    #[serde(skip)]
    vb: Option<Arc<Box<VirtualBook>>>,

    screen_zoom_factor: f32,

    xscale: f32,
    offset: f32,

    #[serde(skip)]
    screen: Screen,

    #[serde(skip)]
    frame_history: frame_history::FrameHistory,

    #[serde(skip)]
    file_store: FileStore,

    current_typed_no: String,

    #[serde(skip)]
    appplayer: AppPlayer,

    current_duration: Duration,
}

impl Default for TemplateApp {
    fn default() -> Self {
        let mut r = BufReader::new(File::open("52 Surprise fox2.book").unwrap());
        let vb = Some(Arc::new(Box::new(
            bookparsing::read_book_stream(&mut r).unwrap(),
        )));

        let factory = MidiPlayerFactory { device_no: 2 };

        let (scmd, rcmd) = channel();
        let (s, r) = channel();

        let player = factory.create(s, rcmd).unwrap();

        let mut appplayer = AppPlayer::new();
        appplayer.player(Some((player, r)));

        //let vb = VirtualBookComponent::default();
        Self {
            vb,
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
            current_duration: Duration::new(0,0)
        }
    }
}

impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
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
            vb,
            frame_history,
            xscale,
            offset,
            screen,
            file_store,
            current_typed_no,
            appplayer,
            screen_zoom_factor,
            current_duration,
        } = self;

        let last_response_arc = Arc::clone(&self.appplayer.last_response);
        // handling messages
        if let Ok(mut opt_last_response) = last_response_arc.lock() {
            if opt_last_response.is_some() {
                let last_response = opt_last_response.as_mut().unwrap();
                // println!("command received : {:?}", &last_response);
                match *last_response {
                    Response::EndOfFile => {
                        &self.appplayer.next();
                    }
                    Response::Current_Play_Time(duration) => {
                        self.current_duration = duration;    
                        ctx.request_repaint_after(Duration::new(1,0));                   
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
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        _frame.close();
                    }
                });
                ui.label("Zoom :");
                ui.add(egui::Slider::new(screen_zoom_factor, 0.5..=4.0));
            });
        });
        ctx.set_pixels_per_point(*screen_zoom_factor);

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
            //});
        });
    }
}
