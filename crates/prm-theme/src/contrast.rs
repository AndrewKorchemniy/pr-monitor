//! WCAG 2.x contrast, and `ensure_contrast`, which moves a color's lightness until it reads.

use crate::{
    color::Rgba8,
    oklab::{Oklab, from_oklab, srgb_to_linear, to_oklab},
};

/// Bisection steps when searching for the smallest lightness change that meets a ratio.
const ENSURE_STEPS: u32 = 30;
/// Backgrounds darker than this luminance push foregrounds lighter, otherwise darker.
const DARK_BACKGROUND: f64 = 0.18;

/// WCAG relative luminance of an opaque color.
pub fn relative_luminance(color: Rgba8) -> f64 {
    0.2126 * srgb_to_linear(color.r)
        + 0.7152 * srgb_to_linear(color.g)
        + 0.0722 * srgb_to_linear(color.b)
}

/// `fg` composited over `bg` in sRGB, as a browser or egui would draw it.
fn over(fg: Rgba8, bg: Rgba8) -> Rgba8 {
    let alpha = f64::from(fg.a) / 255.0;
    let blend = |f: u8, b: u8| {
        let value = f64::from(f) * alpha + f64::from(b) * (1.0 - alpha);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a weighted mean of two bytes stays in 0..=255"
        )]
        let byte = value.round().clamp(0.0, 255.0) as u8;
        byte
    };
    Rgba8::rgb(blend(fg.r, bg.r), blend(fg.g, bg.g), blend(fg.b, bg.b))
}

/// WCAG contrast ratio (1..=21) of `fg` drawn on `bg`; `bg` is treated as opaque.
pub fn contrast_ratio(fg: Rgba8, bg: Rgba8) -> f64 {
    let bg = bg.with_alpha(u8::MAX);
    let (a, b) = (relative_luminance(over(fg, bg)), relative_luminance(bg));
    let (hi, lo) = if a > b { (a, b) } else { (b, a) };
    (hi + 0.05) / (lo + 0.05)
}

/// `fg` if it already meets `min` on `bg`; otherwise the smallest OKLab lightness change (away
/// from `bg`, hue kept) that does. White or black when no lightness can reach `min`.
pub fn ensure_contrast(fg: Rgba8, bg: Rgba8, min: f64) -> Rgba8 {
    if contrast_ratio(fg, bg) >= min {
        return fg;
    }
    let lab = to_oklab(fg);
    let lighter = relative_luminance(bg.with_alpha(u8::MAX)) < DARK_BACKGROUND;
    let target = if lighter { 1.0 } else { 0.0 };
    let at = |l: f64| from_oklab(Oklab { l, ..lab }, fg.a);
    if contrast_ratio(at(target), bg) < min {
        return if lighter { Rgba8::WHITE } else { Rgba8::BLACK };
    }
    let (mut failing, mut passing) = (lab.l, target);
    for _ in 0..ENSURE_STEPS {
        let mid = f64::midpoint(failing, passing);
        if contrast_ratio(at(mid), bg) >= min {
            passing = mid;
        } else {
            failing = mid;
        }
    }
    at(passing)
}

#[cfg(test)]
mod tests;
