// Ecran de visualisation

use egui::Ui;

use crate::{virtualbookcomponent::VirtualBookComponent, TemplateApp};

pub(crate) fn ui_content(app: &mut TemplateApp, ctx: &egui::Context, ui: &mut Ui) {
    egui::warn_if_debug_build(ui);
    let TemplateApp {
        offset,
        xscale,
        appplayer,
        ..
    } = app;

    if let Some(vbc) = &appplayer.vb {
        // draw canvas

        ui.add(egui::Slider::new(xscale, 1.0..=100_000.0));
        //ui.add(egui::Slider::new(offset, 0.0..=100000.0));

        ui.add(
            VirtualBookComponent::from(vbc.clone())
                .offset(*offset)
                .xscale(*xscale),
        );
    }
}
