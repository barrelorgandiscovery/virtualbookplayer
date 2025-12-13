// Ecran de visualisation du carton

use std::sync::Arc;

use egui::{Rangef, Ui};
use egui_extras::{Size, StripBuilder};

use crate::{virtualbookcomponent::VirtualBookComponent, VirtualBookApp};

pub(crate) fn ui_content(app: &mut VirtualBookApp, _ctx: &egui::Context, ui: &mut Ui) {
    // egui::warn_if_debug_build(ui);
    let VirtualBookApp {
        pid_regulated_offset_ms,
        xscale,
        appplayer,
        ..
    } = app;

    let opt_vb = appplayer.virtual_book.read().clone();
    if let Some(vbc) = opt_vb {
        // draw canvas
        ui.horizontal(|ui| {
            StripBuilder::new(ui)
                .size(Size::remainder())
                .size(Size::Relative {
                    fraction: 0.2,
                    range: Rangef {
                        min: 100.0,
                        max: 200.0,
                    },
                })
                .horizontal(|mut strip| {
                    strip.cell(|ui| {
                        ui.horizontal_centered(|ui| {
                            // Display next file name if available
                            if let Some(name) = appplayer.next_file_name() {
                                ui.label(name);
                            }

                            if ui.button(egui_phosphor::regular::FAST_FORWARD).clicked() {
                                appplayer.next();
                            }
                        });
                    });
                    strip.cell(|ui| {
                        ui.add(egui::Slider::new(xscale, 1000.0..=30_000.0).show_value(false));
                    });
                });
        });
        //ui.add(egui::Slider::new(offset, 0.0..=100000.0));

        let foffset: f64 = *pid_regulated_offset_ms;

        ui.add(
            VirtualBookComponent::from_some_indexedvirtualbook(Some(Arc::clone(&vbc)))
                .offset_ms(foffset)
                .xscale(*xscale)
                .scrollbar_width(32.0)
                .set_background_texture_id(app.background_textureid),
        );
    }
}
