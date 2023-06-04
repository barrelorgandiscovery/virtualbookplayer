use std::fs::File;
use std::io::{BufReader, Cursor};
use std::sync::Arc;

use bookparsing::{read_book_stream, VirtualBook};
use egui::epaint::*;
use egui::*;

use crate::virtualbookcomponent::*;

#[path = "frame_history.rs"]
mod frame_history;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    #[serde(skip)]
    vb: Option<Arc<Box<VirtualBook>>>,

    xscale: f32,
    offset: f32,

    #[serde(skip)]
    frame_history: frame_history::FrameHistory,
}

impl Default for TemplateApp {
    fn default() -> Self {
        let mut r = BufReader::new(File::open("52 Surprise fox2.book").unwrap());
        let vb = Some(Arc::new(Box::new(
            bookparsing::read_book_stream(&mut r).unwrap(),
        )));

        //let vb = VirtualBookComponent::default();
        Self {
            vb,
            frame_history: frame_history::FrameHistory::default(),
            offset: 0.0,
            xscale: 16_000f32,
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

        let Self { vb, 
            frame_history,
        xscale,offset } = self;

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
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            // print fps
            self.frame_history.ui(ui);

            egui::warn_if_debug_build(ui);
            let Self {
                vb,
                frame_history,
                offset,
                xscale
            } = self;
            if let Some(vbc) = vb {
                // draw canvas

                ui.add(egui::Slider::new(xscale, 1.0..=100_000.0));
                ui.add(egui::Slider::new(offset, 0.0..=100000.0));


                ui.add(VirtualBookComponent::from(vbc.clone()).offset(*offset).xscale(*xscale));
            }
        });
    }
}
