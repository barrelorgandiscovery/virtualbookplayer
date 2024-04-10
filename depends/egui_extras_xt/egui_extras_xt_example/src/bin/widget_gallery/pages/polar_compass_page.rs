use eframe::egui::{DragValue, Grid, Ui};
use eframe::epaint::Color32;
use egui_extras_xt::common::{Orientation, Winding, WrapMode};
use egui_extras_xt::compasses::{
    CompassAxisLabels, CompassMarker, CompassMarkerShape, DefaultCompassMarkerColor, PolarCompass,
    PolarCompassOverflow,
};
use egui_extras_xt::ui::optional_value_widget::OptionalValueWidget;
use egui_extras_xt::ui::widgets_from_iter::SelectableValueFromIter;
use strum::IntoEnumIterator;

use crate::pages::ui::{
    compass_axis_labels_ui, default_compass_marker_color_ui, default_compass_marker_shape_ui,
    widget_orientation_ui,
};
use crate::pages::PageImpl;

pub struct PolarCompassPage {
    value: f32,
    interactive: bool,
    orientation: Orientation,
    winding: Winding,
    overflow: PolarCompassOverflow,
    diameter: f32,
    wrap: WrapMode,
    min: Option<f32>,
    max: Option<f32>,
    snap: Option<f32>,
    shift_snap: Option<f32>,
    animated: bool,
    axis_labels: Vec<String>,
    axis_label_height: f32,
    max_distance: f32,
    scale_log_base: f32,
    scale_log_mult: f32,
    marker_near_size: f32,
    marker_far_size: f32,
    show_axes: bool,
    show_rings: bool,
    show_cursor: bool,
    show_marker_labels: bool,
    show_marker_lines: bool,
    default_marker_color: DefaultCompassMarkerColor,
    default_marker_shape: CompassMarkerShape,
}

impl Default for PolarCompassPage {
    fn default() -> PolarCompassPage {
        PolarCompassPage {
            value: 0.0,
            interactive: true,
            orientation: Orientation::Top,
            winding: Winding::Clockwise,
            overflow: PolarCompassOverflow::Saturate,
            diameter: 256.0,
            wrap: WrapMode::Unsigned,
            min: None,
            max: None,
            snap: None,
            animated: false,
            shift_snap: Some(15.0f32.to_radians()),
            axis_labels: vec![
                "N".to_owned(),
                "E".to_owned(),
                "S".to_owned(),
                "W".to_owned(),
            ],
            axis_label_height: 24.0,
            max_distance: 10000.0,
            scale_log_base: 10.0,
            scale_log_mult: 1.0,
            marker_near_size: 16.0,
            marker_far_size: 8.0,
            show_axes: true,
            show_rings: true,
            show_cursor: true,
            show_marker_labels: true,
            show_marker_lines: true,
            default_marker_color: DefaultCompassMarkerColor::HsvByAngle {
                hue_phase: 0.0,
                saturation: 1.0,
                value: 1.0,
            },
            default_marker_shape: CompassMarkerShape::Square,
        }
    }
}

impl PageImpl for PolarCompassPage {
    fn ui(&mut self, ui: &mut Ui) {
        ui.add(
            PolarCompass::new(&mut self.value)
                .interactive(self.interactive)
                .orientation(self.orientation)
                .winding(self.winding)
                .overflow(self.overflow)
                .diameter(self.diameter)
                .wrap(self.wrap)
                .min(self.min)
                .max(self.max)
                .snap(self.snap)
                .shift_snap(self.shift_snap)
                .axis_labels(CompassAxisLabels::from_slice(&self.axis_labels))
                .animated(self.animated)
                .axis_label_height(self.axis_label_height)
                .max_distance(self.max_distance)
                .scale_log_base(self.scale_log_base)
                .scale_log_mult(self.scale_log_mult)
                .marker_near_size(self.marker_near_size)
                .marker_far_size(self.marker_far_size)
                .show_axes(self.show_axes)
                .show_rings(self.show_rings)
                .show_cursor(self.show_cursor)
                .show_marker_labels(self.show_marker_labels)
                .show_marker_lines(self.show_marker_lines)
                .default_marker_color(self.default_marker_color)
                .default_marker_shape(self.default_marker_shape)
                .markers(&[
                    CompassMarker::new(0.0f32.to_radians())
                        .distance(10.0)
                        .color(Color32::from_rgb(0xF0, 0xBF, 0x89))
                        .shape(CompassMarkerShape::Diamond)
                        .label("Haibara"),
                    CompassMarker::new(15.0f32.to_radians())
                        .distance(100.0)
                        .color(Color32::from_rgb(0x9C, 0xCF, 0xEE))
                        .shape(CompassMarkerShape::DownArrow)
                        .label("Mitsuhiko"),
                    CompassMarker::new(30.0f32.to_radians())
                        .distance(1000.0)
                        .color(Color32::from_rgb(0x8A, 0xDC, 0x71))
                        .shape(CompassMarkerShape::Circle)
                        .label("Genta"),
                    CompassMarker::new(45.0f32.to_radians())
                        .distance(10000.0)
                        .color(Color32::from_rgb(0xEF, 0xBB, 0xC4))
                        .shape(CompassMarkerShape::UpArrow)
                        .label("Ayumi"),
                    // Markers with unset colors and shapes
                    CompassMarker::new(135.0f32.to_radians())
                        .distance(100.0)
                        .label("A"),
                    CompassMarker::new(157.5f32.to_radians())
                        .distance(200.0)
                        .label("B"),
                    CompassMarker::new(180.0f32.to_radians())
                        .distance(300.0)
                        .label("C"),
                    CompassMarker::new(202.5f32.to_radians())
                        .distance(400.0)
                        .label("D"),
                    CompassMarker::new(225.0f32.to_radians())
                        .distance(500.0)
                        .label("E"),
                    CompassMarker::new(247.5f32.to_radians())
                        .distance(600.0)
                        .label("F"),
                    CompassMarker::new(270.0f32.to_radians())
                        .distance(700.0)
                        .label("G"),
                    CompassMarker::new(292.5f32.to_radians())
                        .distance(800.0)
                        .label("H"),
                    CompassMarker::new(315.0f32.to_radians())
                        .distance(900.0)
                        .label("I"),
                    CompassMarker::new(337.5f32.to_radians())
                        .distance(1000.0)
                        .label("J"),
                ]),
        );
        ui.separator();

        Grid::new("polar_compass_properties")
            .num_columns(2)
            .spacing([20.0, 10.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Value");
                ui.drag_angle(&mut self.value);
                ui.end_row();

                ui.label("Interactive");
                ui.checkbox(&mut self.interactive, "");
                ui.end_row();

                ui.label("Orientation");
                widget_orientation_ui(ui, &mut self.orientation);
                ui.end_row();

                ui.label("Winding");
                ui.horizontal(|ui| {
                    ui.selectable_value_from_iter(&mut self.winding, Winding::iter());
                });
                ui.end_row();

                ui.label("Overflow");
                ui.horizontal(|ui| {
                    ui.selectable_value_from_iter(&mut self.overflow, PolarCompassOverflow::iter());
                });
                ui.end_row();

                ui.label("Diameter");
                ui.add(DragValue::new(&mut self.diameter));
                ui.end_row();

                ui.label("Wrap");
                ui.horizontal(|ui| {
                    ui.selectable_value_from_iter(&mut self.wrap, WrapMode::iter());
                });
                ui.end_row();

                ui.label("Minimum");
                ui.optional_value_widget(&mut self.min, Ui::drag_angle);
                ui.end_row();

                ui.label("Maximum");
                ui.optional_value_widget(&mut self.max, Ui::drag_angle);
                ui.end_row();

                ui.label("Snap");
                ui.optional_value_widget(&mut self.snap, Ui::drag_angle);
                ui.end_row();

                ui.label("Shift snap");
                ui.optional_value_widget(&mut self.shift_snap, Ui::drag_angle);
                ui.end_row();

                ui.label("Animated");
                ui.checkbox(&mut self.animated, "");
                ui.end_row();

                ui.label("Axis labels");
                compass_axis_labels_ui(ui, &mut self.axis_labels);
                ui.end_row();

                ui.label("Axis label height");
                ui.add(DragValue::new(&mut self.axis_label_height));
                ui.end_row();

                ui.label("Maximum distance");
                ui.add(DragValue::new(&mut self.max_distance));
                ui.end_row();

                ui.label("Scale logarithm base");
                ui.add(DragValue::new(&mut self.scale_log_base));
                ui.end_row();

                ui.label("Scale logarithm multiplier");
                ui.add(DragValue::new(&mut self.scale_log_mult));
                ui.end_row();

                ui.label("Near marker size");
                ui.add(DragValue::new(&mut self.marker_near_size));
                ui.end_row();

                ui.label("Far marker size");
                ui.add(DragValue::new(&mut self.marker_far_size));
                ui.end_row();

                ui.label("Show axes");
                ui.checkbox(&mut self.show_axes, "");
                ui.end_row();

                ui.label("Show rings");
                ui.checkbox(&mut self.show_rings, "");
                ui.end_row();

                ui.label("Show cursor");
                ui.checkbox(&mut self.show_cursor, "");
                ui.end_row();

                ui.label("Show marker labels");
                ui.checkbox(&mut self.show_marker_labels, "");
                ui.end_row();

                ui.label("Show marker lines");
                ui.checkbox(&mut self.show_marker_lines, "");
                ui.end_row();

                ui.label("Default marker color");
                default_compass_marker_color_ui(ui, &mut self.default_marker_color);
                ui.end_row();

                ui.label("Default marker shape");
                default_compass_marker_shape_ui(ui, &mut self.default_marker_shape);
                ui.end_row();
            });
    }
}
