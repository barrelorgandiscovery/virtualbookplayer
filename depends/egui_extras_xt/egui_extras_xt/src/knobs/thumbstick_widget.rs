use std::f32::consts::TAU;
use std::ops::RangeInclusive;

use egui::{self, lerp, remap_clamp, Response, Sense, Ui, Widget};
use emath::{vec2, Rot2, Vec2};
use strum::Display;

use crate::common::paint_ellipse;

// ----------------------------------------------------------------------------

/// Combined into one function (rather than two) to make it easier
/// for the borrow checker.
type GetSetValue<'a> = Box<dyn 'a + FnMut(Option<(f32, f32)>) -> (f32, f32)>;

fn get(get_set_value: &mut GetSetValue<'_>) -> (f32, f32) {
    (get_set_value)(None)
}

fn set(get_set_value: &mut GetSetValue<'_>, value: (f32, f32)) {
    (get_set_value)(Some(value));
}

// ----------------------------------------------------------------------------

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Display, PartialEq)]
pub enum ThumbstickSnap {
    #[strum(to_string = "None")]
    None,

    #[strum(to_string = "Strict")]
    Strict {
        axes: usize,
        rotation: f32,
        threshold: f32,
    },
}

impl ThumbstickSnap {
    fn eval(&self, input: Vec2) -> Vec2 {
        match *self {
            ThumbstickSnap::None => input,
            ThumbstickSnap::Strict {
                axes,
                rotation,
                threshold,
            } => {
                assert!(axes > 0, "snapping to non-positive number of axes");
                assert!(
                    (0.0..=1.0).contains(&threshold),
                    "threshold must be normalized"
                );

                if input.length() >= threshold {
                    let mut angle = input.angle() - rotation;
                    angle = (angle / TAU * (axes as f32)).round() * TAU / (axes as f32);
                    angle += rotation;

                    Vec2::angled(angle) * input.length()
                } else {
                    input
                }
            }
        }
    }
}

// ----------------------------------------------------------------------------

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Display, PartialEq)]
pub enum ThumbstickDeadZone {
    #[strum(to_string = "None")]
    None,

    #[strum(to_string = "Scaled radial")]
    ScaledRadial { dead_zone: f32 },
}

impl ThumbstickDeadZone {
    fn eval(&self, input: Vec2) -> Vec2 {
        match *self {
            ThumbstickDeadZone::None => input,
            ThumbstickDeadZone::ScaledRadial { dead_zone } => {
                assert!(
                    (0.0..=1.0).contains(&dead_zone),
                    "dead zone must be normalized"
                );

                let scaled_length = (input.length() - dead_zone).max(0.0) / (1.0 - dead_zone);
                input.normalized() * scaled_length
            }
        }
    }
}

// ----------------------------------------------------------------------------

#[must_use = "You should put this widget in an ui with `ui.add(widget);`"]
pub struct ThumbstickWidget<'a> {
    get_set_value: GetSetValue<'a>,
    range_x: RangeInclusive<f32>,
    range_y: RangeInclusive<f32>,
    precision: f32,
    interactive: bool,
    diameter: f32,
    animated: bool,
    auto_center: bool,
    show_axes: bool,
    snap: ThumbstickSnap,
    dead_zone: ThumbstickDeadZone,
}

impl<'a> ThumbstickWidget<'a> {
    pub fn new(value: &'a mut (f32, f32)) -> Self {
        Self::from_get_set(move |v: Option<(f32, f32)>| {
            if let Some(v) = v {
                *value = v;
            }
            *value
        })
    }

    pub fn from_get_set(get_set_value: impl 'a + FnMut(Option<(f32, f32)>) -> (f32, f32)) -> Self {
        Self {
            get_set_value: Box::new(get_set_value),
            range_x: -1.0..=1.0,
            range_y: -1.0..=1.0,
            precision: 1.0,
            interactive: true,
            diameter: 96.0,
            animated: true,
            auto_center: true,
            show_axes: true,
            snap: ThumbstickSnap::None,
            dead_zone: ThumbstickDeadZone::None,
        }
    }

    pub fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    pub fn diameter(mut self, diameter: impl Into<f32>) -> Self {
        self.diameter = diameter.into();
        self
    }

    pub fn animated(mut self, animated: bool) -> Self {
        self.animated = animated;
        self
    }

    pub fn range(mut self, range: RangeInclusive<f32>) -> Self {
        self.range_x = range.clone();
        self.range_y = range;
        self
    }

    pub fn range_x(mut self, range_x: RangeInclusive<f32>) -> Self {
        self.range_x = range_x;
        self
    }

    pub fn range_y(mut self, range_y: RangeInclusive<f32>) -> Self {
        self.range_y = range_y;
        self
    }

    pub fn precision(mut self, precision: impl Into<f32>) -> Self {
        self.precision = precision.into();
        self
    }

    pub fn auto_center(mut self, auto_center: bool) -> Self {
        self.auto_center = auto_center;
        self
    }

    pub fn show_axes(mut self, show_axes: bool) -> Self {
        self.show_axes = show_axes;
        self
    }

    pub fn snap(mut self, snap: ThumbstickSnap) -> Self {
        self.snap = snap;
        self
    }

    pub fn dead_zone(mut self, dead_zone: ThumbstickDeadZone) -> Self {
        self.dead_zone = dead_zone;
        self
    }
}

impl<'a> Widget for ThumbstickWidget<'a> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let desired_size = Vec2::splat(self.diameter);

        let (rect, mut response) = ui.allocate_exact_size(
            desired_size,
            if self.interactive {
                Sense::click_and_drag()
            } else {
                Sense::hover()
            },
        );

        if response.dragged() {
            let mut v =
                (response.interact_pointer_pos().unwrap() - rect.center()) / (self.diameter / 2.0);

            if v.length() > 1.0 {
                v = v.normalized();
            }

            v = self.dead_zone.eval(v);
            v = v.normalized() * v.length().powf(self.precision);
            v = self.snap.eval(v);

            v.x = remap_clamp(v.x, -1.0..=1.0, self.range_x.clone());
            v.y = remap_clamp(v.y, -1.0..=1.0, self.range_y.clone());

            set(&mut self.get_set_value, v.into());
            response.mark_changed();
        }

        if response.drag_released() && self.auto_center {
            let x_center = lerp(self.range_x.clone(), 0.5);
            let y_center = lerp(self.range_y.clone(), 0.5);

            set(&mut self.get_set_value, (x_center, y_center));
            response.mark_changed();
        }

        if ui.is_rect_visible(rect) {
            let visuals = *ui.style().interact(&response);

            ui.painter().circle(
                rect.center(),
                self.diameter / 2.0,
                ui.style().visuals.faint_bg_color,
                ui.style().visuals.window_stroke(),
            );

            if self.show_axes {
                let paint_snap_axis = |angle| {
                    ui.painter().line_segment(
                        [
                            rect.center(),
                            rect.center() + Vec2::angled(angle) * (self.diameter / 2.0),
                        ],
                        ui.style().visuals.window_stroke(),
                    );
                };

                match self.snap {
                    ThumbstickSnap::None => {}
                    ThumbstickSnap::Strict { axes, rotation, .. } => {
                        for axis_index in 0..axes {
                            let angle = ((axis_index as f32) / (axes as f32)) * TAU + rotation;
                            paint_snap_axis(angle);
                        }
                    }
                };
            }

            {
                let (r, theta) = {
                    let mut v = if self.animated {
                        // Where's .animate_vec2_with_time()?
                        let (x, y) = get(&mut self.get_set_value);
                        vec2(
                            ui.ctx()
                                .animate_value_with_time(response.id.with("x"), x, 0.1),
                            ui.ctx()
                                .animate_value_with_time(response.id.with("y"), y, 0.1),
                        )
                    } else {
                        get(&mut self.get_set_value).into()
                    };

                    v.x = remap_clamp(v.x, self.range_x.clone(), -1.0..=1.0);
                    v.y = remap_clamp(v.y, self.range_y.clone(), -1.0..=1.0);

                    (v.length().clamp(0.0, 1.0), v.angle())
                };

                let tilt_factor = 0.9;

                let mut paint_thumbstick = |size| {
                    let ellipse_center = rect.center()
                        + Vec2::angled(theta)
                            * r
                            * ((self.diameter - (self.diameter * tilt_factor * size)) / 2.0);

                    let ellipse_size = Vec2::splat(self.diameter)
                        * size
                        * Vec2::new(1.0 - (1.0 - tilt_factor) * r, 1.0);

                    paint_ellipse(
                        ui,
                        ellipse_center,
                        ellipse_size,
                        visuals.bg_fill,
                        visuals.fg_stroke,
                        Rot2::from_angle(theta),
                    );
                };

                paint_thumbstick(0.750);
                paint_thumbstick(0.625);
            }
        }

        response
    }
}
