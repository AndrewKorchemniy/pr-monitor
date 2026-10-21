use super::*;
use crate::oklab::to_oklch;

#[test]
fn contrast_ratio_known_pairs() {
    let ratio = contrast_ratio(Rgba8::BLACK, Rgba8::WHITE);
    assert!((ratio - 21.0).abs() < 1e-9, "{ratio}");
    assert!(contrast_ratio(Rgba8::rgb(0x76, 0x76, 0x76), Rgba8::WHITE) >= 4.5);
    assert!((contrast_ratio(Rgba8::WHITE, Rgba8::WHITE) - 1.0).abs() < 1e-9);
}

#[test]
fn contrast_ratio_composites_translucent_foreground() {
    let half_white = Rgba8::WHITE.with_alpha(128);
    let ratio = contrast_ratio(half_white, Rgba8::BLACK);
    assert!(ratio > 4.0 && ratio < 6.0, "{ratio}");
}

#[test]
fn ensure_contrast_reaches_min_keeps_hue() {
    let bg = Rgba8::rgb(0x16, 0x18, 0x1C);
    let fg = Rgba8::rgb(0x2E, 0x7D, 0x52);
    let fixed = ensure_contrast(fg, bg, 7.0);
    assert!(contrast_ratio(fixed, bg) >= 7.0);
    assert!((to_oklch(fixed).h - to_oklch(fg).h).abs() < 3.0);
    assert!(
        contrast_ratio(fixed, bg) < 7.3,
        "smallest change, not the extreme"
    );
}

#[test]
fn ensure_contrast_keeps_passing_colors() {
    let bg = Rgba8::rgb(0x16, 0x18, 0x1C);
    let fg = Rgba8::rgb(0xDF, 0xE2, 0xE7);
    assert_eq!(ensure_contrast(fg, bg, 7.0), fg);
}

#[test]
fn ensure_contrast_unreachable_returns_extreme() {
    let mid_gray = Rgba8::rgb(0x76, 0x76, 0x76);
    assert_eq!(ensure_contrast(mid_gray, mid_gray, 21.0), Rgba8::BLACK);
    let dark = Rgba8::rgb(0x20, 0x20, 0x20);
    assert_eq!(ensure_contrast(dark, dark, 21.0), Rgba8::WHITE);
}

#[test]
fn is_dark_splits_at_the_dark_background_luminance() {
    let cases = [
        (Rgba8::rgb(0x16, 0x18, 0x1C), true),
        (Rgba8::rgb(0x70, 0x70, 0x70), true),
        (Rgba8::rgb(0x80, 0x80, 0x80), false),
        (Rgba8::rgb(0xEE, 0xF1, 0xF4), false),
        (Rgba8::BLACK.with_alpha(0), true),
    ];
    for (color, dark) in cases {
        assert_eq!(is_dark(color), dark, "{color:?}");
    }
}
