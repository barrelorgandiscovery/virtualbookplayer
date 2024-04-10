//! Display component for the book / midi file
//! using egui rendering
//!
use std::collections::BTreeSet;
use std::{io::Cursor, sync::Arc};

use std::error::Error;

use egui::epaint::*;
use egui::*;

use bookparsing::{read_book_stream, Hole, VirtualBook};

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
        Some(self.hole_ref.timestamp.cmp(&other.hole_ref.timestamp))
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
    xscale: f32,
    yfactor: f32,
    fit_to_height: bool,

    scrollbars_visible: bool,
    scrollbats_width: f32,

    virtual_book: Option<Arc<IndexedVirtualBook>>,
}

impl Default for VirtualBookComponent {
    fn default() -> Self {
        Self {
            offset_ms: 0.0,
            xscale: 3_000f32,
            yfactor: 3.0f32,
            fit_to_height: true,
            scrollbars_visible: true,
            scrollbats_width: 12.0,
            virtual_book: None,
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

    pub fn xscale(mut self, xscale: f32) -> Self {
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

    pub fn ui_content(&mut self, ui: &mut Ui) -> egui::Response {
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

        egui::ScrollArea::horizontal()
            //.hscroll(*scrollbars_visible)
            .show(ui, |ui| {
                let (response, painter) = ui.allocate_painter(
                    Vec2::new(width_container, ui.available_height()),
                    Sense::hover(),
                );

                let midx = width_container / 2.0f32;
                let maxy = response.rect.height();

                if let Some(current_vb) = &mut self.virtual_book {
                    let to_screen = emath::RectTransform::from_to(
                        Rect::from_min_size(Pos2::ZERO, response.rect.size()),
                        response.rect,
                    );

                    if *fit_to_height {
                        *yfactor =
                            response.rect.size().y / current_vb.virtualbook.scale.definition.width;
                    }

                    // background draw
                    let book_background = Rect::from_points(&[
                        pos2(0.0, 0.0),
                        to_screen * pos2(width_container, response.rect.size().y),
                    ]);
                    painter.add(RectShape::filled(
                        book_background,
                        Rounding::default(),
                        Color32::from_rgb(255, 255, 255),
                    ));

                    #[cfg(feature = "profiling")]
                    profiling::scope!("Filtering Holes");
                    // range search for speed up the display
                    let visible: Vec<&OrdHole> = current_vb
                        .index_start
                        .range(std::ops::Range {
                            start: OrdHole {
                                hole_ref: Hole {
                                    timestamp: (*offset_in_millis * 1000.0
                                        - width_container as f64 / 2.0 * *xscale as f64)
                                        as i64,
                                    track: 0,
                                    length: 0,
                                },
                            },

                            end: OrdHole {
                                hole_ref: Hole {
                                    timestamp: (*offset_in_millis * 1000.0
                                        + width_container as f64 / 2.0 * *xscale as f64)
                                        as i64,
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

                    #[cfg(feature = "profiling")]
                    profiling::scope!("Display Holes");
                    // notes draw
                    let rects: Vec<(Rect, Color32)> = visible
                        .into_iter()
                        .map(|h| {
                            [
                                pos2(
                                    (((h.hole_ref.timestamp as f64 - *offset_in_millis * 1000.0)
                                        / *xscale as f64)
                                        + width_container as f64 / 2.0)
                                        as f32,
                                    mappingy(
                                        (h.hole_ref.track as f32
                                            * current_vb
                                                .virtualbook
                                                .scale
                                                .definition
                                                .intertrackdistance
                                            + current_vb
                                                .virtualbook
                                                .scale
                                                .definition
                                                .firsttrackdistance
                                            - current_vb
                                                .virtualbook
                                                .scale
                                                .definition
                                                .defaulttrackheight
                                                / 2.0)
                                            * *yfactor,
                                    ),
                                ),
                                pos2(
                                    ((((h.hole_ref.timestamp + h.hole_ref.length) as f64
                                        - *offset_in_millis * 1000.0)
                                        / *xscale as f64)
                                        + width_container as f64 / 2.0)
                                        as f32,
                                    mappingy(
                                        (h.hole_ref.track as f32
                                            * current_vb
                                                .virtualbook
                                                .scale
                                                .definition
                                                .intertrackdistance
                                            + current_vb
                                                .virtualbook
                                                .scale
                                                .definition
                                                .firsttrackdistance
                                            + current_vb
                                                .virtualbook
                                                .scale
                                                .definition
                                                .defaulttrackheight
                                                / 2.0)
                                            * *yfactor,
                                    ),
                                ),
                            ]
                        })
                        .enumerate()
                        .map(|(i, h)| {
                            let points_in_screen: Vec<Pos2> =
                                h.iter().map(|p| to_screen * (*p)).collect();

                            let rect = Rect::from_points(&points_in_screen);
                            let point_response =
                                ui.interact(rect, response.id.with(i), Sense::hover());

                            let mut color = Color32::from_rgb(100, 100, 100);
                            if point_response.hovered() {
                                color = Color32::from_rgb(50, 50, 50);
                            }

                            (rect, color)
                        })
                        .collect();

                    // draw the elements, for reactive elements
                    for (r, c) in rects.iter() {
                        painter.add(RectShape::filled(*r, Rounding::default(), *c));
                    }
                    // blue bar
                    let bar = Rect::from_points(&[
                        to_screen * pos2(midx - 1.0, 0.0),
                        to_screen * pos2(midx + 1.0, maxy + 10.0),
                    ]);
                    painter.add(RectShape::filled(bar, Rounding::default(), Color32::BLUE));
                } else {
                    // no virtualbook

                    let to_screen = emath::RectTransform::from_to(
                        Rect::from_min_size(Pos2::ZERO, response.rect.size()),
                        response.rect,
                    );
                    // draw an empty
                    let book_background = to_screen
                        .transform_rect(Rect::from_min_size(Pos2::ZERO, response.rect.size()));

                    painter.add(RectShape::filled(
                        book_background,
                        Rounding::default(),
                        Color32::WHITE,
                    ));

                    let bar = Rect::from_points(&[
                        to_screen * pos2(midx - 1.0, 0.0),
                        to_screen * pos2(midx + 1.0, maxy + 10.0),
                    ]);

                    painter.add(RectShape::filled(bar, Rounding::default(), Color32::BLUE));
                }
                ui.ctx().request_repaint();
                ui.interact(response.rect, Id::new("area"), Sense::click())
                //response from the area
            })
            .inner
    }
}

/// widget implementation
impl Widget for VirtualBookComponent {
    #[cfg_attr(any(feature = "profiling"), profiling::function)]
    fn ui(mut self, ui: &mut Ui) -> Response {
        self.ui_content(ui)
    }
}
