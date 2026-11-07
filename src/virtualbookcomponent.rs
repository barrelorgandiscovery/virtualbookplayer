//! Display component for the book / midi file.
//! Uses egui shapes by default, or a wgpu PaintCallback when GPU FX is available.

use std::collections::BTreeSet;
use std::{io::Cursor, sync::Arc};

use std::error::Error;

use egui::epaint::*;
use egui::*;

use bookparsing::{read_book_stream, Hole, VirtualBook};

use crate::gfx::book_fx::{self, BookFxState};
use crate::gfx::BookEffectKind;

pub struct IndexedVirtualBook {
    pub virtualbook: Arc<VirtualBook>,
    pub index_start: BTreeSet<OrdHole>,
    pub max_time: Option<i64>,
    pub min_time: Option<i64>,
}

pub struct OrdHole {
    hole_ref: Hole,
}

impl Ord for OrdHole {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.hole_ref.timestamp.cmp(&other.hole_ref.timestamp) {
            std::cmp::Ordering::Equal => match self.hole_ref.track.cmp(&other.hole_ref.track) {
                std::cmp::Ordering::Equal => self.hole_ref.length.cmp(&other.hole_ref.length),
                e => e,
            },
            e => e,
        }
    }
}

impl PartialEq for OrdHole {
    fn eq(&self, other: &Self) -> bool {
        self.hole_ref.track == other.hole_ref.track
            && self.hole_ref.timestamp == other.hole_ref.timestamp
            && self.hole_ref.length == other.hole_ref.length
    }
}

impl Eq for OrdHole {}

impl PartialOrd for OrdHole {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl IndexedVirtualBook {
    pub fn from(vb: &Arc<VirtualBook>) -> Self {
        Self {
            virtualbook: Arc::clone(vb),
            index_start: vb
                .holes
                .holes
                .iter()
                .cloned()
                .map(|h| OrdHole { hole_ref: h })
                .collect(),
            min_time: vb.min_time(),
            max_time: vb.max_time(),
        }
    }

    pub fn min_time(&self) -> Option<i64> {
        self.min_time
    }

    pub fn max_time(&self) -> Option<i64> {
        self.max_time
    }
}

/// Virtualbook component
pub struct VirtualBookComponent {
    // offset of display in milliseconds
    offset_ms: f64,
    xscale: f64,
    yfactor: f32,
    fit_to_height: bool,

    scrollbars_visible: bool,
    scrollbats_width: f32,

    background_texture_id: Option<egui::TextureId>,

    virtual_book: Option<Arc<IndexedVirtualBook>>,

    /// When true and `BookFxState::gpu_ready`, notes/particles use wgpu.
    use_gpu_fx: bool,
    /// Selected visual effect (preferences).
    book_effect: BookEffectKind,
    /// Playing → spawn particles on note starts.
    playing: bool,
}

impl Default for VirtualBookComponent {
    fn default() -> Self {
        Self {
            offset_ms: 0.0_f64,
            xscale: 3_000f64,
            yfactor: 3.0f32,
            fit_to_height: true,
            scrollbars_visible: true,
            scrollbats_width: 12.0,
            background_texture_id: None,
            virtual_book: None,
            use_gpu_fx: false,
            book_effect: BookEffectKind::None,
            playing: false,
        }
    }
}

#[cfg_attr(any(feature = "profiling"), profiling::all_functions)]
impl VirtualBookComponent {
    /// create the component state from the virtual book
    pub fn from(virtual_book: &Arc<VirtualBook>) -> VirtualBookComponent {
        VirtualBookComponent {
            virtual_book: Some(Arc::new(IndexedVirtualBook::from(virtual_book))),
            ..Default::default()
        }
    }

    pub fn from_some_indexedvirtualbook(
        indexed: Option<Arc<IndexedVirtualBook>>,
    ) -> VirtualBookComponent {
        VirtualBookComponent {
            virtual_book: indexed,
            ..Default::default()
        }
    }
    /// create the component state from the virtual book
    pub fn from_some_virtualbook(
        some_virtual_book: &Option<Arc<VirtualBook>>,
    ) -> VirtualBookComponent {
        let index = some_virtual_book
            .as_ref()
            .map(|vb| Arc::new(IndexedVirtualBook::from(vb)));

        VirtualBookComponent {
            virtual_book: index,
            ..Default::default()
        }
    }

    #[allow(dead_code)]
    pub fn new() -> VirtualBookComponent {
        VirtualBookComponent {
            virtual_book: None,
            ..Default::default()
        }
    }

    #[allow(dead_code)]
    pub fn open_from_string_content(
        &mut self,
        file_string_content: String,
    ) -> Result<(), Box<dyn Error>> {
        let mut c = Cursor::new(file_string_content.as_bytes().to_vec());
        self.virtual_book = Some(Arc::new(IndexedVirtualBook::from(&Arc::new(
            read_book_stream(&mut c)?,
        ))));
        Ok(())
    }

    pub fn xscale(mut self, xscale: f64) -> Self {
        self.xscale = xscale;
        self
    }

    /// percentage of the display
    pub fn offset_ms(mut self, offset: f64) -> Self {
        self.offset_ms = offset;
        self
    }

    pub fn hide_scrollbar(mut self) -> Self {
        self.scrollbars_visible = false;
        self
    }

    pub fn scrollbar_width(mut self, width: f32) -> Self {
        self.scrollbats_width = width;
        self
    }

    pub fn set_background_texture_id(mut self, texture_id: Option<egui::TextureId>) -> Self {
        self.background_texture_id = texture_id;
        self
    }

    pub fn with_gpu_fx(mut self, enabled: bool) -> Self {
        self.use_gpu_fx = enabled;
        self
    }

    pub fn book_effect(mut self, effect: BookEffectKind) -> Self {
        self.book_effect = effect;
        self
    }

    pub fn playing(mut self, playing: bool) -> Self {
        self.playing = playing;
        self
    }

    pub fn ui_content(&mut self, ui: &mut Ui) -> egui::Response {
        self.ui_content_with_fx(ui, None)
    }

    pub fn ui_content_with_fx(
        &mut self,
        ui: &mut Ui,
        mut book_fx: Option<&mut BookFxState>,
    ) -> egui::Response {
        let Self {
            offset_ms: offset_in_millis,
            xscale,
            yfactor,
            fit_to_height,
            ..
        } = self;

        let mut style = ui.style_mut().clone();
        style.spacing.scroll.bar_width = self.scrollbats_width;
        style.spacing.scroll.handle_min_length = 50.0;

        ui.set_style(style);

        let width_container = ui.available_width();
        let use_gpu = self.use_gpu_fx
            && self.book_effect.uses_gpu_fx()
            && book_fx.as_ref().is_some_and(|fx| fx.gpu_ready());
        let playing = self.playing;
        let book_effect = self.book_effect;
        let background_texture_id = self.background_texture_id;

        egui::ScrollArea::horizontal()
            .show(ui, |ui| {
                let (response, painter) = ui.allocate_painter(
                    Vec2::new(width_container, ui.available_height()),
                    Sense::hover(),
                );

                // Single width for notes + parchment so they share one book-space X.
                let book_w = response.rect.width().max(1.0);
                let midx = book_w / 2.0;
                let maxy = response.rect.height();

                let to_screen = emath::RectTransform::from_to(
                    Rect::from_min_size(Pos2::ZERO, response.rect.size()),
                    response.rect,
                );

                let book_background = to_screen.transform_rect(Rect::from_min_size(
                    Pos2::ZERO,
                    response.rect.size(),
                ));

                // Parchment UV in the same paper-pixel space as note X:
                //   paper_x(local_x) = (local_x - midx) + offset_us / xscale
                // so the texture translates exactly with the holes (no parallax).
                let offset_us = *offset_in_millis * 1000.0;
                // Tile size in points — larger than before so junctions are rarer;
                // the uploaded craft texture is edge-blended for seamless Repeat.
                const PAPER_TILE_PTS: f64 = 900.0;
                let paper_x0 = -midx as f64 + offset_us / *xscale;
                let paper_x1 = (book_w - midx) as f64 + offset_us / *xscale;
                let u0 = (paper_x0 / PAPER_TILE_PTS) as f32;
                let u1 = (paper_x1 / PAPER_TILE_PTS) as f32;
                let v1 = (maxy as f64 / PAPER_TILE_PTS) as f32;

                if let Some(texture_id) = background_texture_id {
                    let mut mesh = Mesh::with_texture(texture_id);
                    let uv = Rect {
                        min: Pos2::new(u0, 0.0),
                        max: Pos2::new(u1, v1.max(0.01)),
                    };
                    mesh.add_rect_with_uv(book_background, uv, Color32::WHITE);
                    painter.add(Shape::mesh(mesh));
                } else {
                    painter.add(RectShape::filled(
                        book_background,
                        CornerRadius::default(),
                        Color32::WHITE,
                    ));
                }

                if let Some(current_vb) = &mut self.virtual_book {
                    if *fit_to_height {
                        *yfactor = (response.rect.size().y)
                            / (current_vb.virtualbook.scale.definition.width);
                    }

                    #[cfg(feature = "profiling")]
                    profiling::scope!("Filtering Holes");

                    // range search for speed up the display (use index)
                    let visible: Vec<&OrdHole> = current_vb
                        .index_start
                        .range(std::ops::Range {
                            start: OrdHole {
                                hole_ref: Hole {
                                    timestamp: (offset_us - book_w as f64 * *xscale) as i64,
                                    track: 0,
                                    length: 0,
                                },
                            },

                            end: OrdHole {
                                hole_ref: Hole {
                                    timestamp: (offset_us + book_w as f64 / 2.0 * *xscale) as i64,
                                    track: 0,
                                    length: 0,
                                },
                            },
                        })
                        .collect();

                    let mappingy = |y| {
                        if current_vb
                            .virtualbook
                            .scale
                            .definition
                            .ispreferredviewinverted
                        {
                            response.rect.size().y - y
                        } else {
                            y
                        }
                    };

                    // Playhead X — holes are "read" on this exact screen column.
                    let playhead_x = (to_screen * pos2(midx, 0.0)).x;

                    #[cfg(feature = "profiling")]
                    profiling::scope!("Display Holes");

                    let scale = &current_vb.virtualbook.scale.definition;
                    // (start_us, length_us, playhead hit, color) for particle FX
                    let mut note_sparks: Vec<(i64, i64, Pos2, Color32)> =
                        Vec::with_capacity(visible.len());
                    let mut note_draws: Vec<book_fx::NoteDraw> = Vec::with_capacity(visible.len());

                    // UV scale: book microseconds → texture repeats along the roll
                    let uv_per_us = 1.0 / (*xscale as f32 * 80.0);

                    for (i, h) in visible.into_iter().enumerate() {
                        let x0 = ((h.hole_ref.timestamp as f64 - offset_us) / *xscale
                            + midx as f64) as f32;
                        let x1 = (((h.hole_ref.timestamp + h.hole_ref.length) as f64 - offset_us)
                            / *xscale
                            + midx as f64) as f32;
                        let y0 = mappingy(
                            (h.hole_ref.track as f32 * scale.intertrackdistance
                                + scale.firsttrackdistance
                                - scale.defaulttrackheight / 2.0)
                                * *yfactor,
                        );
                        let y1 = mappingy(
                            (h.hole_ref.track as f32 * scale.intertrackdistance
                                + scale.firsttrackdistance
                                + scale.defaulttrackheight / 2.0)
                                * *yfactor,
                        );

                        let points_in_screen =
                            [to_screen * pos2(x0, y0), to_screen * pos2(x1, y1)];
                        let rect = Rect::from_points(&points_in_screen);

                        let mut color = Color32::from_rgb(90, 90, 95);
                        if !use_gpu {
                            let point_response =
                                ui.interact(rect, response.id.with(i), Sense::hover());
                            if point_response.hovered() {
                                color = Color32::from_rgb(50, 0, 0);
                            }
                        }

                        // Active only while the playhead line sits inside the note.
                        let active = rect.min.x <= playhead_x && rect.max.x >= playhead_x;
                        let playhead_hit = if active {
                            color = Color32::from_rgb(255, 55, 40);
                            Some(Pos2::new(playhead_x, rect.center().y))
                        } else {
                            None
                        };

                        // Book-space UVs so the note texture scrolls with carton offset.
                        let u0 = h.hole_ref.timestamp as f32 * uv_per_us;
                        let u1 = (h.hole_ref.timestamp + h.hole_ref.length) as f32 * uv_per_us;
                        let v0 = h.hole_ref.track as f32 * 0.17;
                        let book_uv = Rect::from_min_max(Pos2::new(u0, v0), Pos2::new(u1, v0 + 0.12));

                        note_sparks.push((
                            h.hole_ref.timestamp,
                            h.hole_ref.length,
                            Pos2::new(playhead_x, rect.center().y),
                            Color32::from_rgb(255, 180, 60),
                        ));

                        note_draws.push(book_fx::NoteDraw {
                            rect,
                            color,
                            book_uv,
                            active,
                            playhead_hit,
                        });
                    }

                    if use_gpu {
                        if let Some(fx) = book_fx.as_mut() {
                            fx.set_effect(book_effect);
                            let dt = ui.input(|i| i.stable_dt);
                            let playhead_us = offset_us as i64;
                            fx.update_play(dt, playing, playhead_us, *xscale, &note_sparks);

                            // Viewport = paint callback rect (NDC ±1 on its edges).
                            let (note_verts, particle_verts) =
                                book_fx::build_frame_meshes(&note_draws, fx, response.rect);
                            book_fx::paint_book_fx(
                                &painter,
                                response.rect,
                                note_verts,
                                particle_verts,
                            );

                            if playing || fx.has_alive_particles() {
                                ui.ctx().request_repaint();
                            }
                        }
                    } else {
                        for note in &note_draws {
                            painter.add(RectShape::filled(
                                note.rect,
                                CornerRadius::same(1),
                                note.color,
                            ));
                        }
                        if playing {
                            ui.ctx().request_repaint();
                        }
                    }

                    // Quiet read line on top: precise "now", soft enough for the fire.
                    paint_playhead(&painter, playhead_x, response.rect);
                } else {
                    let to_screen = emath::RectTransform::from_to(
                        Rect::from_min_size(Pos2::ZERO, response.rect.size()),
                        response.rect,
                    );
                    let book_background = to_screen
                        .transform_rect(Rect::from_min_size(Pos2::ZERO, response.rect.size()));

                    painter.add(RectShape::filled(
                        book_background,
                        CornerRadius::default(),
                        Color32::WHITE,
                    ));

                    let playhead_x = (to_screen * pos2(midx, 0.0)).x;
                    paint_playhead(&painter, playhead_x, response.rect);
                }
                ui.interact(response.rect, Id::new("area"), Sense::click())
            })
            .inner
    }
}

/// Quiet, precise vertical read line: soft veil + 1px hairline.
///
/// Drawn muted so particle fire stays the visual focus, while the column of
/// "now" remains easy to track on the parchment.
fn paint_playhead(painter: &Painter, x: f32, book: Rect) {
    let top = book.top();
    let bottom = book.bottom();

    // Wide, very transparent veil — presence without a hard slab of color.
    painter.rect_filled(
        Rect::from_min_max(pos2(x - 5.0, top), pos2(x + 5.0, bottom)),
        CornerRadius::ZERO,
        Color32::from_rgba_unmultiplied(40, 55, 70, 28),
    );
    painter.rect_filled(
        Rect::from_min_max(pos2(x - 2.0, top), pos2(x + 2.0, bottom)),
        CornerRadius::ZERO,
        Color32::from_rgba_unmultiplied(55, 75, 95, 40),
    );

    // Crisp 1px core for the exact play time (pixel-snapped).
    let x_px = painter.round_to_pixel_center(x);
    painter.line_segment(
        [pos2(x_px, top), pos2(x_px, bottom)],
        Stroke::new(1.0, Color32::from_rgba_unmultiplied(70, 95, 120, 160)),
    );
}

/// widget implementation (CPU path only — no FX state).
impl Widget for VirtualBookComponent {
    #[cfg_attr(any(feature = "profiling"), profiling::function)]
    fn ui(mut self, ui: &mut Ui) -> Response {
        self.ui_content(ui)
    }
}
