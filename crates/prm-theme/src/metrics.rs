//! Sizes, type, motion and strokes. Density picks row heights; the theme only scales corner radii
//! and toggles stripes.

use crate::file::MetricTweaks;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum Density {
    #[default]
    Compact,
    Comfortable,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FontFace {
    /// Atkinson Hyperlegible Next.
    Ui,
    /// Atkinson Hyperlegible Mono: numbers, ids, branches.
    Mono,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypeRole {
    Display,
    Title,
    Body,
    BodyStrong,
    Button,
    Label,
    Small,
    Mono,
    MonoStrong,
}

impl TypeRole {
    pub const ALL: [Self; 9] = [
        Self::Display,
        Self::Title,
        Self::Body,
        Self::BodyStrong,
        Self::Button,
        Self::Label,
        Self::Small,
        Self::Mono,
        Self::MonoStrong,
    ];
}

/// Sizes in points at zoom 1.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TypeSpec {
    pub face: FontFace,
    pub weight: u16,
    pub size: f32,
    pub line_height: f32,
    pub tracking: f32,
}

const fn spec(face: FontFace, weight: u16, size: f32, line_height: f32) -> TypeSpec {
    TypeSpec {
        face,
        weight,
        size,
        line_height,
        tracking: 0.0,
    }
}

impl TypeRole {
    /// The type ramp: 26/15/13/12, nothing smaller than 12, no italics.
    pub const fn spec(self) -> TypeSpec {
        use FontFace::{Mono, Ui};
        match self {
            Self::Display => TypeSpec {
                tracking: -0.4,
                ..spec(Mono, 800, 26.0, 32.0)
            },
            Self::Title => spec(Ui, 600, 15.0, 20.0),
            Self::Body => spec(Ui, 400, 13.0, 18.0),
            Self::BodyStrong => spec(Ui, 600, 13.0, 18.0),
            Self::Button => spec(Ui, 500, 13.0, 18.0),
            Self::Label => spec(Ui, 500, 12.0, 16.0),
            Self::Small => spec(Ui, 400, 12.0, 16.0),
            Self::Mono => spec(Mono, 400, 12.0, 16.0),
            Self::MonoStrong => spec(Mono, 600, 12.0, 16.0),
        }
    }
}

/// The spacing scale in points.
#[derive(Clone, Debug, PartialEq)]
pub struct Space {
    pub xxs: f32,
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub xxl: f32,
    pub xxxl: f32,
}

pub const SPACE: Space = Space {
    xxs: 2.0,
    xs: 4.0,
    sm: 6.0,
    md: 8.0,
    lg: 12.0,
    xl: 16.0,
    xxl: 24.0,
    xxxl: 32.0,
};

/// Corner radii in points, already multiplied by the theme's `radius_scale`.
#[derive(Clone, Debug, PartialEq)]
pub struct Radius {
    pub sm: u8,
    pub md: u8,
    pub lg: u8,
    /// Fully rounded ends (pills); not scaled except to 0.
    pub pill: u8,
}

/// Unscaled radii: sm, md, lg, pill.
const BASE_RADIUS: [f32; 3] = [3.0, 5.0, 8.0];
const PILL_RADIUS: u8 = 255;

/// Heights and paddings per density, in points.
#[derive(Clone, Debug, PartialEq)]
pub struct DensityMetrics {
    pub row_height: f32,
    pub header_height: f32,
    pub toolbar_height: f32,
    pub statusbar_height: f32,
    pub control_height: f32,
    pub cell_pad_x: f32,
    pub pill_height: f32,
    pub glyph: f32,
    pub margin_col: f32,
    pub revision_bar: f32,
}

impl Density {
    pub const fn metrics(self) -> DensityMetrics {
        match self {
            Self::Compact => DensityMetrics {
                row_height: 24.0,
                header_height: 26.0,
                toolbar_height: 40.0,
                statusbar_height: 26.0,
                control_height: 26.0,
                cell_pad_x: 8.0,
                pill_height: 18.0,
                glyph: 10.0,
                margin_col: 6.0,
                revision_bar: 3.0,
            },
            Self::Comfortable => DensityMetrics {
                row_height: 30.0,
                header_height: 30.0,
                toolbar_height: 46.0,
                statusbar_height: 28.0,
                control_height: 30.0,
                cell_pad_x: 10.0,
                pill_height: 20.0,
                glyph: 11.0,
                margin_col: 8.0,
                revision_bar: 3.0,
            },
        }
    }
}

/// Durations in seconds; the UI zeroes them when animations are off.
#[derive(Clone, Debug, PartialEq)]
pub struct Motion {
    /// Hover and press.
    pub quick: f32,
    /// Palette, peek and modal entrance.
    pub standard: f32,
    /// The change bar drawing in.
    pub reveal: f32,
    /// The change bar retracting.
    pub retract: f32,
}

pub const MOTION: Motion = Motion {
    quick: 0.083,
    standard: 0.167,
    reveal: 0.180,
    retract: 0.120,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Strokes {
    pub hairline: f32,
    pub emphasis: f32,
    pub focus: f32,
}

pub const STROKES: Strokes = Strokes {
    hairline: 1.0,
    emphasis: 1.5,
    focus: 2.0,
};

/// Floating surfaces only (palette, menus, peek, modals).
#[derive(Clone, Debug, PartialEq)]
pub struct Elevation {
    pub popup_offset: [i8; 2],
    pub popup_blur: u8,
}

pub const ELEVATION: Elevation = Elevation {
    popup_offset: [0, 4],
    popup_blur: 16,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Metrics {
    pub density: Density,
    pub space: Space,
    pub radius: Radius,
    pub row: DensityMetrics,
    pub motion: Motion,
    pub stroke: Strokes,
    pub elevation: Elevation,
    pub stripes: bool,
}

impl Metrics {
    pub fn new(density: Density, tweaks: &MetricTweaks) -> Self {
        let scaled = |r: f32| {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "radius_scale is clamped to 0..=2, so the result fits a u8"
            )]
            let radius = (r * tweaks.radius_scale).round().clamp(0.0, 255.0) as u8;
            radius
        };
        Self {
            density,
            space: SPACE,
            radius: Radius {
                sm: scaled(BASE_RADIUS[0]),
                md: scaled(BASE_RADIUS[1]),
                lg: scaled(BASE_RADIUS[2]),
                pill: if tweaks.radius_scale > 0.0 {
                    PILL_RADIUS
                } else {
                    0
                },
            },
            row: density.metrics(),
            motion: MOTION,
            stroke: STROKES,
            elevation: ELEVATION,
            stripes: tweaks.stripes,
        }
    }
}

#[cfg(test)]
mod tests;
