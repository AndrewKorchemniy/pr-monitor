use super::*;

fn close(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
}

#[test]
fn oklab_roundtrip_srgb_grid() {
    for r in (0..=255).step_by(17) {
        for g in (0..=255).step_by(17) {
            for b in (0..=255).step_by(17) {
                let color = Rgba8::rgb(r, g, b);
                assert_eq!(from_oklab(to_oklab(color), 255), color, "{color}");
            }
        }
    }
}

#[test]
fn oklch_red_vector() {
    let red = to_oklch(Rgba8::rgb(255, 0, 0));
    assert!(close(red.l, 0.627_96, 1e-3), "{red:?}");
    assert!(close(red.c, 0.257_68, 1e-3), "{red:?}");
    assert!(close(red.h, 29.23, 1e-1), "{red:?}");
}

#[test]
fn oklab_white_black() {
    let white = to_oklab(Rgba8::WHITE);
    let black = to_oklab(Rgba8::BLACK);
    assert!(close(white.l, 1.0, 1e-4) && close(white.a, 0.0, 1e-4) && close(white.b, 0.0, 1e-4));
    assert!(close(black.l, 0.0, 1e-9));
}

#[test]
fn mix_endpoints() {
    let (x, y) = (Rgba8::rgb(0x16, 0x18, 0x1C), Rgba8::rgb(0x86, 0xC8, 0xEA));
    assert_eq!(mix(x, y, 0.0), x);
    assert_eq!(mix(x, y, 1.0), y);
    let middle = to_oklab(mix(x, y, 0.5));
    let expected = f64::midpoint(to_oklab(x).l, to_oklab(y).l);
    assert!(close(middle.l, expected, 0.01));
}

#[test]
fn lift_changes_only_lightness() {
    let base = Rgba8::rgb(0x2E, 0x7D, 0x52);
    let lighter = to_oklch(lift(base, 0.05));
    let original = to_oklch(base);
    assert!(close(lighter.l, original.l + 0.05, 0.005));
    assert!(close(lighter.h, original.h, 1.0));
}

#[test]
fn out_of_gamut_keeps_lightness() {
    let vivid = Oklab {
        l: 0.7,
        a: 0.4,
        b: 0.1,
    };
    let mapped = to_oklab(from_oklab(vivid, 255));
    assert!(close(mapped.l, 0.7, 0.01), "{mapped:?}");
}
