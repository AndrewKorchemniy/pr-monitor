use super::*;

fn colors() -> MarkColors {
    MarkColors {
        body: Rgba8::rgb(0x16, 0x18, 0x1C),
        outline: Rgba8::rgb(0xDF, 0xE2, 0xE7),
        lines: Rgba8::rgb(0xA5, 0xA7, 0xAC),
        bar: Rgba8::rgb(0x86, 0xC8, 0xEA),
    }
}

/// FNV-1a over the raster: a stable fingerprint for the golden value.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn pixel(raster: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
    let i = usize::try_from((y * size + x) * 4).unwrap();
    [raster[i], raster[i + 1], raster[i + 2], raster[i + 3]]
}

#[test]
fn mark_deterministic_fnv() {
    let hashes: Vec<String> = [16, 32, 64, 256]
        .map(|size| format!("{size}: {:016x}", fnv(&render_mark(size, &colors()))))
        .to_vec();
    insta::assert_snapshot!(hashes.join("\n"));
}

#[test]
fn mark_layout_regions() {
    let size = 64;
    let raster = render_mark(size, &colors());
    assert_eq!(raster.len(), 64 * 64 * 4);
    assert_eq!(pixel(&raster, size, 0, 0)[3], 0, "corner is transparent");
    let bar = colors().bar;
    assert_eq!(
        pixel(&raster, size, 7, 32),
        [bar.r, bar.g, bar.b, 255],
        "bar"
    );
    let body = colors().body;
    assert_eq!(
        pixel(&raster, size, 40, 48),
        [body.r, body.g, body.b, 255],
        "card body"
    );
    let lines = colors().lines;
    assert_eq!(
        pixel(&raster, size, 36, 25),
        [lines.r, lines.g, lines.b, 255],
        "text line"
    );
}

#[test]
fn mark_colors_from_palette() {
    let tokens = crate::ThemeRegistry::builtin()
        .tokens("graphite")
        .unwrap()
        .0;
    let colors = MarkColors::from_palette(&tokens.palette, tokens.palette.signature_revision);
    assert_eq!(colors.body, tokens.palette.tray_body);
    assert_eq!(colors.bar, tokens.palette.signature_revision);
}
