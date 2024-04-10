use strum::{Display, EnumIter};

#[derive(Clone, Copy)]
pub struct DisplayMetrics {
    pub segment_spacing: f32,
    pub segment_thickness: f32,

    pub digit_median: f32,
    pub digit_ratio: f32,
    pub digit_shearing: f32,
    pub digit_spacing: f32,

    pub margin_horizontal: f32,
    pub margin_vertical: f32,

    pub colon_separation: f32,
}

impl Default for DisplayMetrics {
    fn default() -> Self {
        DisplayMetricsPreset::Default.metrics()
    }
}

// ----------------------------------------------------------------------------

#[non_exhaustive]
#[derive(Clone, Copy, Display, EnumIter, Eq, PartialEq)]
pub enum DisplayMetricsPreset {
    #[strum(to_string = "Default")]
    Default,

    #[strum(to_string = "Wide")]
    Wide,

    #[strum(to_string = "Calculator")]
    Calculator,
}

impl DisplayMetricsPreset {
    #[must_use]
    pub fn metrics(&self) -> DisplayMetrics {
        match *self {
            DisplayMetricsPreset::Default => DisplayMetrics {
                segment_spacing: 0.01,
                segment_thickness: 0.1,
                digit_median: -0.05,
                digit_ratio: 0.6,
                digit_shearing: 0.1,
                digit_spacing: 0.35,
                margin_horizontal: 0.3,
                margin_vertical: 0.1,
                colon_separation: 0.25,
            },
            DisplayMetricsPreset::Wide => DisplayMetrics {
                segment_spacing: 0.02,
                segment_thickness: 0.12,
                digit_median: -0.05,
                digit_ratio: 1.0,
                digit_shearing: 0.1,
                digit_spacing: 0.20,
                margin_horizontal: 0.3,
                margin_vertical: 0.1,
                colon_separation: 0.25,
            },
            DisplayMetricsPreset::Calculator => DisplayMetrics {
                segment_spacing: 0.01,
                segment_thickness: 0.11,
                digit_median: -0.05,
                digit_ratio: 0.4,
                digit_shearing: 0.1,
                digit_spacing: 0.38,
                margin_horizontal: 0.3,
                margin_vertical: 0.1,
                colon_separation: 0.25,
            },
        }
    }
}
