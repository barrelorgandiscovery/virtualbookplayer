use std::f32::consts::TAU;

use ecolor::Hsva;
use egui::{vec2, Align2, Color32, FontFamily, FontId, Rect, Shape, Stroke, Ui, Vec2};
use itertools::Itertools;
use strum::Display;

use crate::common::normalized_angle_unsigned_excl;
use crate::hash::PearsonHash;

// ----------------------------------------------------------------------------

#[must_use]
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Display, PartialEq)]
pub enum DefaultCompassMarkerColor {
    #[strum(to_string = "System")]
    System,

    #[strum(to_string = "Fixed")]
    Fixed(Color32),

    #[strum(to_string = "HSV by angle")]
    HsvByAngle {
        hue_phase: f32,
        saturation: f32,
        value: f32,
    },

    #[strum(to_string = "HSV by label")]
    HsvByLabel {
        hue_phase: f32,
        saturation: f32,
        value: f32,
    },
}

impl DefaultCompassMarkerColor {
    #[must_use]
    pub(crate) fn color(&self, ui: &Ui, marker: &CompassMarker) -> Color32 {
        match *self {
            DefaultCompassMarkerColor::System => ui.style().visuals.text_color(),
            DefaultCompassMarkerColor::Fixed(color) => color,
            DefaultCompassMarkerColor::HsvByAngle {
                hue_phase,
                saturation,
                value,
            } => {
                let hue_raw = marker.angle / TAU;
                let hue = (hue_raw + hue_phase).rem_euclid(1.0);
                Color32::from(Hsva::new(hue, saturation, value, 1.0))
            }
            DefaultCompassMarkerColor::HsvByLabel {
                hue_phase,
                saturation,
                value,
            } => {
                let marker_label = marker.label.unwrap_or("");
                let hue_raw = marker_label.pearson_hash() as f32 / 255.0;
                let hue = (hue_raw + hue_phase).rem_euclid(1.0);
                Color32::from(Hsva::new(hue, saturation, value, 1.0))
            }
        }
    }
}

// ----------------------------------------------------------------------------

#[must_use = "You should put this marker into a compass with `compass.markers(&[markers]);`"]
pub struct CompassMarker<'a> {
    pub(crate) angle: f32,
    pub(crate) distance: Option<f32>,
    pub(crate) shape: Option<CompassMarkerShape>,
    pub(crate) label: Option<&'a str>,
    pub(crate) color: Option<Color32>,
}

impl<'a> CompassMarker<'a> {
    pub fn new(angle: f32) -> Self {
        Self {
            angle: normalized_angle_unsigned_excl(angle),
            distance: None,
            shape: None,
            label: None,
            color: None,
        }
    }

    pub fn distance(mut self, distance: f32) -> Self {
        self.distance = Some(distance);
        self
    }

    pub fn shape(mut self, shape: CompassMarkerShape) -> Self {
        self.shape = Some(shape);
        self
    }

    pub fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }
}

// ----------------------------------------------------------------------------

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Display, PartialEq)]
pub enum CompassMarkerShape {
    #[strum(to_string = "Square")]
    Square,

    #[strum(to_string = "Circle")]
    Circle,

    #[strum(to_string = "Right arrow")]
    RightArrow,

    #[strum(to_string = "Up arrow")]
    UpArrow,

    #[strum(to_string = "Left arrow")]
    LeftArrow,

    #[strum(to_string = "Down arrow")]
    DownArrow,

    #[strum(to_string = "Diamond")]
    Diamond,

    #[strum(to_string = "Star")]
    Star(usize, f32),

    #[strum(to_string = "Emoji")]
    Emoji(char),
}

impl CompassMarkerShape {
    pub(crate) fn paint(&self, ui: &mut Ui, rect: Rect, fill: Color32, stroke: Stroke) {
        match *self {
            CompassMarkerShape::Square => {
                ui.painter().rect(rect, 0.0, fill, stroke);
            }
            CompassMarkerShape::Circle => {
                ui.painter().rect(rect, rect.width() / 2.0, fill, stroke);
            }
            CompassMarkerShape::RightArrow => {
                let rect = Rect::from_center_size(
                    rect.center(),
                    rect.size() * vec2(3.0f32.sqrt() / 2.0, 1.0),
                );

                ui.painter().add(Shape::convex_polygon(
                    vec![rect.right_center(), rect.left_bottom(), rect.left_top()],
                    fill,
                    stroke,
                ));
            }
            CompassMarkerShape::UpArrow => {
                let rect = Rect::from_center_size(
                    rect.center(),
                    rect.size() * vec2(1.0, 3.0f32.sqrt() / 2.0),
                );

                ui.painter().add(Shape::convex_polygon(
                    vec![rect.center_top(), rect.right_bottom(), rect.left_bottom()],
                    fill,
                    stroke,
                ));
            }
            CompassMarkerShape::LeftArrow => {
                let rect = Rect::from_center_size(
                    rect.center(),
                    rect.size() * vec2(3.0f32.sqrt() / 2.0, 1.0),
                );

                ui.painter().add(Shape::convex_polygon(
                    vec![rect.left_center(), rect.right_top(), rect.right_bottom()],
                    fill,
                    stroke,
                ));
            }
            CompassMarkerShape::DownArrow => {
                let rect = Rect::from_center_size(
                    rect.center(),
                    rect.size() * vec2(1.0, 3.0f32.sqrt() / 2.0),
                );

                ui.painter().add(Shape::convex_polygon(
                    vec![rect.left_top(), rect.right_top(), rect.center_bottom()],
                    fill,
                    stroke,
                ));
            }
            CompassMarkerShape::Diamond => {
                ui.painter().add(Shape::convex_polygon(
                    vec![
                        rect.center_top(),
                        rect.right_center(),
                        rect.center_bottom(),
                        rect.left_center(),
                    ],
                    fill,
                    stroke,
                ));
            }
            CompassMarkerShape::Star(rays, ratio) => {
                assert!(rays >= 2, "star-shaped markers must have at least 2 rays");
                assert!(
                    (0.0..=1.0).contains(&ratio),
                    "ray ratio of star-shaped markers must be normalized"
                );

                let outer_radius = rect.width() * 0.5;
                let inner_radius = outer_radius * ratio;
                let star_rotation = -TAU * 0.25;

                let outer_points = (0..rays).map(|point_index| {
                    rect.center()
                        + Vec2::angled(
                            star_rotation + TAU * ((point_index as f32 + 0.0) / rays as f32),
                        ) * outer_radius
                });

                let inner_points = (0..rays).map(|point_index| {
                    rect.center()
                        + Vec2::angled(
                            star_rotation + TAU * ((point_index as f32 + 0.5) / rays as f32),
                        ) * inner_radius
                });

                // TODO: Broken polygon renderer
                // https://github.com/emilk/egui/issues/513
                ui.painter().add(Shape::convex_polygon(
                    outer_points.interleave(inner_points).collect_vec(),
                    fill,
                    stroke,
                ));
            }
            CompassMarkerShape::Emoji(emoji) => {
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    emoji,
                    FontId::new(rect.height(), FontFamily::Proportional),
                    fill,
                );
            }
        }
    }
}
