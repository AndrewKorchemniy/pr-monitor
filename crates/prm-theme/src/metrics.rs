//! Sizes, type, motion and strokes. Row heights are fixed; the user's `Corners` choice sets every
//! radius, and the striped-rows choice rides along.

use serde::{Deserialize, Serialize};

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

/// The corner shape picked in Settings › Appearance; it applies to every theme.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Corners {
    /// Radius 0 everywhere, pills included.
    Square,
    #[default]
    #[serde(other)]
    Rounded,
}

impl Corners {
    pub const fn is_rounded(self) -> bool {
        matches!(self, Self::Rounded)
    }
}

/// The appearance choices the user makes in Settings; they apply to every theme.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Look {
    pub corners: Corners,
    /// Alternating row tint in the table and cards.
    pub stripes: bool,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            corners: Corners::default(),
            stripes: true,
        }
    }
}

/// Corner radii in points.
#[derive(Clone, Debug, PartialEq)]
pub struct Radius {
    /// Slivers: selection rings, thin bars, slider tracks.
    pub xs: u8,
    pub sm: u8,
    pub md: u8,
    pub lg: u8,
    /// Fully rounded ends (pills).
    pub pill: u8,
}

impl Radius {
    pub const fn new(corners: Corners) -> Self {
        match corners {
            Corners::Rounded => Self {
                xs: 2,
                sm: 3,
                md: 5,
                lg: 8,
                pill: 255,
            },
            Corners::Square => Self {
                xs: 0,
                sm: 0,
                md: 0,
                lg: 0,
                pill: 0,
            },
        }
    }
}

/// Heights and paddings, in points.
#[derive(Clone, Debug, PartialEq)]
pub struct RowMetrics {
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

pub const ROW: RowMetrics = RowMetrics {
    row_height: 30.0,
    header_height: 30.0,
    toolbar_height: 46.0,
    statusbar_height: 32.0,
    control_height: 30.0,
    cell_pad_x: 10.0,
    pill_height: 20.0,
    glyph: 11.0,
    margin_col: 8.0,
    revision_bar: 3.0,
};

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
    pub corners: Corners,
    pub space: Space,
    pub radius: Radius,
    pub row: RowMetrics,
    pub motion: Motion,
    pub stroke: Strokes,
    pub elevation: Elevation,
    pub stripes: bool,
}

impl Metrics {
    pub fn new(look: Look) -> Self {
        let Look { corners, stripes } = look;
        Self {
            corners,
            space: SPACE,
            radius: Radius::new(corners),
            row: ROW,
            motion: MOTION,
            stroke: STROKES,
            elevation: ELEVATION,
            stripes,
        }
    }
}

#[cfg(test)]
mod tests;
