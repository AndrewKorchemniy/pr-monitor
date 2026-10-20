//! OKLab / OKLCH (Björn Ottosson, 2020), hand-rolled: conversion, mixing, lightness shifts and
//! chroma-reducing gamut mapping. About as much color science as the derivation needs.

use crate::color::Rgba8;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Oklab {
    pub l: f64,
    pub a: f64,
    pub b: f64,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Oklch {
    pub l: f64,
    pub c: f64,
    /// Degrees, 0..360.
    pub h: f64,
}

/// Bisection steps for gamut mapping; 2^-24 of the chroma range is far below one 8-bit step.
const GAMUT_STEPS: u32 = 24;
/// Linear channels this far outside 0..=1 still count as in gamut (rounding noise).
const GAMUT_EPSILON: f64 = 1e-6;

pub fn srgb_to_linear(channel: u8) -> f64 {
    let c = f64::from(channel) / 255.0;
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear_to_srgb(linear: f64) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let encoded = if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 0..=255 before the cast"
    )]
    let byte = (encoded * 255.0).round().clamp(0.0, 255.0) as u8;
    byte
}

#[expect(
    clippy::many_single_char_names,
    reason = "Ottosson's published notation: r g b channels, l m s cone responses"
)]
pub fn to_oklab(color: Rgba8) -> Oklab {
    let (r, g, b) = (
        srgb_to_linear(color.r),
        srgb_to_linear(color.g),
        srgb_to_linear(color.b),
    );
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    Oklab {
        l: 0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        a: 1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        b: 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    }
}

fn to_linear_rgb(lab: Oklab) -> [f64; 3] {
    let l = (lab.l + 0.396_337_777_4 * lab.a + 0.215_803_757_3 * lab.b).powi(3);
    let m = (lab.l - 0.105_561_345_8 * lab.a - 0.063_854_172_8 * lab.b).powi(3);
    let s = (lab.l - 0.089_484_177_5 * lab.a - 1.291_485_548 * lab.b).powi(3);
    [
        4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
        -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
        -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s,
    ]
}

fn in_gamut(rgb: [f64; 3]) -> bool {
    rgb.iter()
        .all(|c| (-GAMUT_EPSILON..=1.0 + GAMUT_EPSILON).contains(c))
}

/// Back to sRGB. Out-of-gamut colors keep L and hue and lose chroma until they fit.
pub fn from_oklab(lab: Oklab, alpha: u8) -> Rgba8 {
    let lab = Oklab {
        l: lab.l.clamp(0.0, 1.0),
        ..lab
    };
    let mut rgb = to_linear_rgb(lab);
    if !in_gamut(rgb) {
        let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
        for _ in 0..GAMUT_STEPS {
            let mid = f64::midpoint(lo, hi);
            let scaled = Oklab {
                a: lab.a * mid,
                b: lab.b * mid,
                ..lab
            };
            if in_gamut(to_linear_rgb(scaled)) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        rgb = to_linear_rgb(Oklab {
            a: lab.a * lo,
            b: lab.b * lo,
            ..lab
        });
    }
    Rgba8 {
        r: linear_to_srgb(rgb[0]),
        g: linear_to_srgb(rgb[1]),
        b: linear_to_srgb(rgb[2]),
        a: alpha,
    }
}

pub fn to_oklch(color: Rgba8) -> Oklch {
    let lab = to_oklab(color);
    Oklch {
        l: lab.l,
        c: lab.a.hypot(lab.b),
        h: lab.b.atan2(lab.a).to_degrees().rem_euclid(360.0),
    }
}

/// OKLab lerp; `t` may extrapolate (L is clamped). Alpha is interpolated too.
pub fn mix(x: Rgba8, y: Rgba8, t: f64) -> Rgba8 {
    let (from, to) = (to_oklab(x), to_oklab(y));
    let lab = Oklab {
        l: from.l + (to.l - from.l) * t,
        a: from.a + (to.a - from.a) * t,
        b: from.b + (to.b - from.b) * t,
    };
    let alpha = f64::from(x.a) + (f64::from(y.a) - f64::from(x.a)) * t;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 0..=255 before the cast"
    )]
    let alpha = alpha.round().clamp(0.0, 255.0) as u8;
    from_oklab(lab, alpha)
}

/// Shifts OKLab lightness by `dl` (clamped), keeping hue and chroma where the gamut allows.
pub fn lift(color: Rgba8, dl: f64) -> Rgba8 {
    let lab = to_oklab(color);
    from_oklab(
        Oklab {
            l: lab.l + dl,
            ..lab
        },
        color.a,
    )
}

#[cfg(test)]
mod tests;
