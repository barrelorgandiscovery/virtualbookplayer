// Ecran de visualisation du carton

use std::sync::Arc;

use egui::{Rangef, Ui};
use egui_extras::{Size, StripBuilder};

use crate::{virtualbookcomponent::VirtualBookComponent, VirtualBookApp};

pub(crate) fn ui_content(app: &mut VirtualBookApp, ctx: &egui::Context, ui: &mut Ui) {
    let playing = app.appplayer.is_playing() && !app.appplayer.is_paused();
    let opt_vb = app.appplayer.virtual_book.read().clone();
    let gpu = app.book_fx.as_ref().is_some_and(|fx| fx.gpu_ready());
    let book_effect = app.book_effect;
    let book_effect_scale = app.book_effect_scale;

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
                            if let Some(name) = app.appplayer.next_file_name() {
                                ui.label(name);
                            }

                            if ui.button(egui_phosphor::regular::FAST_FORWARD).clicked() {
                                app.appplayer.next();
                            }
                        });
                    });
                    strip.cell(|ui| {
                        ui.add(
                            egui::Slider::new(&mut app.xscale, 1000.0..=30_000.0).show_value(false),
                        );
                    });
                });
        });

        // Latency-compensated playhead: blue bar + note FX track what you hear.
        let display_offset_ms = app.visual_offset_ms();
        let xscale = app.xscale;

        if app.background_texture_handle.is_none() {
            app.background_texture_handle = Some(ctx.load_texture(
                "bgbookimage",
                app.background_texture_image.clone(),
                egui::TextureOptions {
                    magnification: egui::TextureFilter::Linear,
                    minification: egui::TextureFilter::Linear,
                    wrap_mode: egui::TextureWrapMode::Repeat,
                    mipmap_mode: None,
                },
            ));
            app.background_textureid = Some(egui::TextureId::from(
                app.background_texture_handle.as_ref().unwrap(),
            ));
        }
        let background_textureid = app.background_textureid;

        let mut component =
            VirtualBookComponent::from_some_indexedvirtualbook(Some(Arc::clone(&vbc)))
                .offset_ms(display_offset_ms)
                .xscale(xscale)
                .scrollbar_width(32.0)
                .set_background_texture_id(background_textureid)
                .with_gpu_fx(gpu)
                .book_effect(book_effect)
                .book_effect_scale(book_effect_scale)
                .playing(playing);

        component.ui_content_with_fx(ui, app.book_fx.as_mut());
    }
}
