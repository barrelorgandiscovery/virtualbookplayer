// Ecran de visualisation

use std::sync::Arc;

use egui::Ui;

use crate::{virtualbookcomponent::VirtualBookComponent, VirtualBookApp};

pub(crate) fn ui_content(app: &mut VirtualBookApp, _ctx: &egui::Context, ui: &mut Ui) {
    // egui::warn_if_debug_build(ui);
    let VirtualBookApp {
        offset,
        xscale,
        appplayer,
        ..
    } = app;

    if let Some(vbc) = &appplayer.virtual_book {
        // draw canvas

        ui.add(egui::Slider::new(xscale, 1.0..=100_000.0));
        //ui.add(egui::Slider::new(offset, 0.0..=100000.0));

        ui.add(
            VirtualBookComponent::from(Arc::clone(vbc))
                .offset(*offset)
                .xscale(*xscale),
        );
    }
}
