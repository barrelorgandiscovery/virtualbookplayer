use std::ops::RangeInclusive;

use eframe::egui::{DragValue, Grid, Ui};
use egui_extras_xt::knobs::{ThumbstickDeadZone, ThumbstickSnap, ThumbstickWidget};
use egui_extras_xt::ui::drag_rangeinclusive::DragRangeInclusive;

use crate::pages::ui::{thumbstick_dead_zone_ui, thumbstick_snap_ui};
use crate::pages::PageImpl;

pub struct ThumbstickWidgetPage {
    position: (f32, f32),
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

impl Default for ThumbstickWidgetPage {
    fn default() -> ThumbstickWidgetPage {
        ThumbstickWidgetPage {
            position: (0.0, 0.0),
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
}

impl PageImpl for ThumbstickWidgetPage {
    fn ui(&mut self, ui: &mut Ui) {
        ui.add(
            ThumbstickWidget::new(&mut self.position)
                .range_x(self.range_x.clone())
                .range_y(self.range_y.clone())
                .precision(self.precision)
                .interactive(self.interactive)
                .diameter(self.diameter)
                .animated(self.animated)
                .auto_center(self.auto_center)
                .show_axes(self.show_axes)
                .snap(self.snap)
                .dead_zone(self.dead_zone),
        );
        ui.separator();

        Grid::new("thumbstick_widget_properties")
            .num_columns(2)
            .spacing([20.0, 10.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("X position");
                ui.add(DragValue::new(&mut self.position.0));
                ui.end_row();

                ui.label("Y position");
                ui.add(DragValue::new(&mut self.position.1));
                ui.end_row();

                ui.label("X range");
                ui.drag_rangeinclusive(&mut self.range_x);
                ui.end_row();

                ui.label("Y range");
                ui.drag_rangeinclusive(&mut self.range_y);
                ui.end_row();

                ui.label("Precision");
                ui.add(DragValue::new(&mut self.precision));
                ui.end_row();

                ui.label("Interactive");
                ui.checkbox(&mut self.interactive, "");
                ui.end_row();

                ui.label("Diameter");
                ui.add(DragValue::new(&mut self.diameter));
                ui.end_row();

                ui.label("Animated");
                ui.checkbox(&mut self.animated, "");
                ui.end_row();

                ui.label("Auto-center");
                ui.checkbox(&mut self.auto_center, "");
                ui.end_row();

                ui.label("Show axes");
                ui.checkbox(&mut self.show_axes, "");
                ui.end_row();

                ui.label("Snap");
                thumbstick_snap_ui(ui, &mut self.snap);
                ui.end_row();

                ui.label("Dead zone");
                thumbstick_dead_zone_ui(ui, &mut self.dead_zone);
                ui.end_row();
            });
    }
}
