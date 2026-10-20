//! The brand mark, rasterized: a rounded "strip" card with two text lines and the change bar on
//! its left edge. One mark for the app icon, the window icon and the tray; the bar's color is
//! the state. Anti-aliased with signed-distance coverage; std only.

use crate::{color::Rgba8, palette::Palette};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MarkColors {
    pub body: Rgba8,
    pub outline: Rgba8,
    pub lines: Rgba8,
    pub bar: Rgba8,
}

impl MarkColors {
    /// The theme's tray colors with the given bar color.
    pub const fn from_palette(palette: &Palette, bar: Rgba8) -> Self {
        Self {
            body: palette.tray_body,
            outline: palette.tray_outline,
            lines: palette.tray_lines,
            bar,
        }
    }
}

/// An axis-aligned rounded rectangle in units of the icon size (0..=1).
#[derive(Copy, Clone)]
struct RoundRect {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    radius: f64,
}

const CARD: RoundRect = RoundRect {
    x0: 0.22,
    y0: 0.16,
    x1: 0.90,
    y1: 0.84,
    radius: 0.12,
};
const BAR: RoundRect = RoundRect {
    x0: 0.08,
    y0: 0.16,
    x1: 0.16,
    y1: 0.84,
    radius: 0.04,
};
const LINE_1: RoundRect = RoundRect {
    x0: 0.34,
    y0: 0.36,
    x1: 0.78,
    y1: 0.44,
    radius: 0.04,
};
const LINE_2: RoundRect = RoundRect {
    x0: 0.34,
    y0: 0.56,
    x1: 0.64,
    y1: 0.64,
    radius: 0.04,
};
/// The card outline is this fraction of the size, but never thinner than one pixel.
const OUTLINE: f64 = 1.0 / 32.0;

impl RoundRect {
    /// Signed distance in pixels from the pixel center `(px, py)`; negative inside.
    fn distance(self, px: f64, py: f64, size: f64) -> f64 {
        let (cx, cy) = (
            f64::midpoint(self.x0, self.x1) * size,
            f64::midpoint(self.y0, self.y1) * size,
        );
        let radius = self.radius * size;
        let half_w = (self.x1 - self.x0) * size / 2.0 - radius;
        let half_h = (self.y1 - self.y0) * size / 2.0 - radius;
        let qx = (px - cx).abs() - half_w;
        let qy = (py - cy).abs() - half_h;
        qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - radius
    }

    fn coverage(self, px: f64, py: f64, size: f64) -> f64 {
        (0.5 - self.distance(px, py, size)).clamp(0.0, 1.0)
    }
}

/// `size × size` straight-alpha RGBA, row-major (32 for the tray, 64 for the window icon,
/// 256 for the app icon).
pub fn render_mark(size: u32, colors: &MarkColors) -> Vec<u8> {
    let side = f64::from(size);
    let outline = (OUTLINE * side).max(1.0);
    let capacity = usize::try_from(size).unwrap_or(0).pow(2) * 4;
    let mut out = Vec::with_capacity(capacity);
    for y in 0..size {
        for x in 0..size {
            let (px, py) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
            let card = CARD.coverage(px, py, side);
            let inner = (0.5 - CARD.distance(px, py, side) - outline).clamp(0.0, 1.0);
            let mut pixel = [0.0_f64; 4];
            over(&mut pixel, colors.outline, card);
            over(&mut pixel, colors.body, inner);
            let lines = LINE_1
                .coverage(px, py, side)
                .max(LINE_2.coverage(px, py, side));
            over(&mut pixel, colors.lines, lines.min(inner));
            over(&mut pixel, colors.bar, BAR.coverage(px, py, side));
            out.extend(pixel.map(to_byte));
        }
    }
    out
}

/// Composites `color` at `coverage` over `dst` (straight alpha, channels 0..=1).
fn over(dst: &mut [f64; 4], color: Rgba8, coverage: f64) {
    let src_a = f64::from(color.a) / 255.0 * coverage;
    if src_a <= 0.0 {
        return;
    }
    let dst_a = dst[3];
    let out_a = src_a + dst_a * (1.0 - src_a);
    let src = [color.r, color.g, color.b].map(|c| f64::from(c) / 255.0);
    for (channel, value) in dst.iter_mut().take(3).zip(src) {
        *channel = (value * src_a + *channel * dst_a * (1.0 - src_a)) / out_a;
    }
    dst[3] = out_a;
}

fn to_byte(value: f64) -> u8 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 0..=255 before the cast"
    )]
    let byte = (value * 255.0).round().clamp(0.0, 255.0) as u8;
    byte
}

#[cfg(test)]
mod tests;
