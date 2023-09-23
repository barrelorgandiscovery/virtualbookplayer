use std::{io::Cursor, sync::Arc};

use egui::epaint::*;
use egui::*;

use bookparsing::{read_book_stream, VirtualBook};

pub struct VirtualBookComponent {
    offset: f32,
    xscale: f32,
    yfactor: f32,
    fit_to_height: bool,

    scrollbars_visible: bool,
    scrollbats_width: f32,

    virtual_book: Option<Arc<VirtualBook>>,
}

impl Default for VirtualBookComponent {
    fn default() -> Self {
        Self {
            offset: 0.0,
            xscale: 3_000f32,
            yfactor: 3.0f32,
            fit_to_height: true,
            scrollbars_visible: true,
            scrollbats_width: 12.0,
            virtual_book: None,
        }
    }
}

impl VirtualBookComponent {
    /// create the component state from the virtual book
    pub fn from(virtual_book: Arc<VirtualBook>) -> VirtualBookComponent {
        VirtualBookComponent {
            virtual_book: Some(virtual_book),
            ..Default::default()
        }
    }

    /// create the component state from the virtual book
    pub fn from_some_virtualbook(
        some_virtual_book: Option<Arc<VirtualBook>>,
    ) -> VirtualBookComponent {
        VirtualBookComponent {
            virtual_book: some_virtual_book,
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
    pub fn open_from_string_content(&mut self, file_string_content: String) -> std::io::Result<()> {
        let mut c = Cursor::new(file_string_content.as_bytes().to_vec());
        self.virtual_book = Some(Arc::new(read_book_stream(&mut c)?));
        Ok(())
    }

    pub fn xscale(mut self, xscale: f32) -> Self {
        self.xscale = xscale;
        self
    }

    /// percentage of the display
    pub fn offset(mut self, offset: f32) -> Self {
        self.offset = offset;
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
            offset,
            xscale,
            yfactor,
            fit_to_height,
            scrollbars_visible,
            ..
        } = self;

        let mut style = ui.style_mut().clone();
        style.spacing.scroll_bar_width = self.scrollbats_width;
        style.spacing.scroll_handle_min_length = 50.0;

        ui.set_style(style);

        let width_container = ui.available_width();

        egui::ScrollArea::horizontal()
            .hscroll(*scrollbars_visible)
            .show(ui, |ui| {
                let mut book_screen_width = ui.available_width();

                if let Some(current_vb) = &self.virtual_book {
                    if let Some(maxtime) = current_vb.max_time() {
                        book_screen_width = ((maxtime as f64) / *xscale as f64) as f32;
                    }
                }

                let offset_with_bar = *offset * book_screen_width - width_container / 2.0;

                let (response, painter) = ui.allocate_painter(
                    Vec2::new(book_screen_width, ui.available_height()),
                    Sense::hover(),
                );

                let midx = width_container / 2.0f32;
                let maxy = response.rect.height();
                // println!("midx {}, maxy {}", midx,maxy);

                if let Some(current_vb) = &mut self.virtual_book {
                    let to_screen = emath::RectTransform::from_to(
                        Rect::from_min_size(Pos2::ZERO, response.rect.size()),
                        response.rect,
                    );

                    if *fit_to_height {
                        *yfactor = response.rect.size().y / current_vb.scale.definition.width;
                    }

                    let book_background = Rect::from_points(&[
                        pos2(0.0, 0.0),
                        to_screen
                            * pos2(
                                book_screen_width,
                                current_vb.scale.definition.width * *yfactor,
                            ),
                    ]);
                    painter.add(RectShape::filled(
                        book_background,
                        Rounding::default(),
                        Color32::from_rgb(255, 255, 255),
                    ));

                    let rects: Vec<(Rect, Color32)> = current_vb
                        .holes
                        .holes
                        .iter()
                        .map(|h| {
                            [
                                pos2(
                                    (h.timestamp as f64 / *xscale as f64) as f32,
                                    (h.track as f32
                                        * current_vb.scale.definition.intertrackdistance
                                        + current_vb.scale.definition.firsttrackdistance
                                        - current_vb.scale.definition.defaulttrackheight / 2.0)
                                        * *yfactor,
                                ),
                                pos2(
                                    ((h.timestamp + h.length) as f64 / *xscale as f64) as f32,
                                    (h.track as f32
                                        * current_vb.scale.definition.intertrackdistance
                                        + current_vb.scale.definition.firsttrackdistance
                                        + current_vb.scale.definition.defaulttrackheight / 2.0)
                                        * *yfactor,
                                ),
                            ]
                        })
                        .enumerate()
                        .map(|(i, h)| {
                            let points_in_screen: Vec<Pos2> = h
                                .iter()
                                .map(|p| {
                                    to_screen
                                        * (*p
                                            - Vec2 {
                                                x: offset_with_bar,
                                                y: 0.0,
                                            })
                                })
                                .collect();

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

                    let bar = Rect::from_points(&[
                        to_screen * pos2(midx - 1.0, 0.0),
                        to_screen * pos2(midx + 1.0, maxy + 10.0),
                    ]);
                    painter.add(RectShape::filled(bar, Rounding::default(), Color32::BLUE));
                } else {
                    // some virtualbook

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
                //response
            })
            .inner
    }
}

/// widget implementation
impl Widget for VirtualBookComponent {
    fn ui(mut self, ui: &mut Ui) -> Response {
        self.ui_content(ui)
    }
}
