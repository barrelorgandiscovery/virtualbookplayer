// Ecran de visualisation du carton

use std::sync::Arc;

use egui::Ui;

use crate::{virtualbookcomponent::VirtualBookComponent, VirtualBookApp};

pub(crate) fn ui_content(app: &mut VirtualBookApp, _ctx: &egui::Context, ui: &mut Ui) {
    // egui::warn_if_debug_build(ui);
    let VirtualBookApp {
        pid_regulated_offset_ms,
        xscale,
        appplayer,
        ..
    } = app;

    if let Some(vbc) = appplayer.virtual_book.read().clone() {
        // draw canvas

        ui.add(egui::Slider::new(xscale, 1000.0..=30_000.0));
        //ui.add(egui::Slider::new(offset, 0.0..=100000.0));

        let foffset: f64 = *pid_regulated_offset_ms;

        ui.add(
            VirtualBookComponent::from_some_indexedvirtualbook(Some(Arc::clone(&vbc)))
                .offset_ms(foffset)
                .xscale(*xscale)
                .scrollbar_width(32.0),
        );
    }
}
