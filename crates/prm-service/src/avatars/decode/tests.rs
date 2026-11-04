use png::{BitDepth, ColorType, Encoder};

use super::*;

/// A PNG of the given shape; `data` is already in that colour type and depth.
fn encode(
    (width, height): (u32, u32),
    color: ColorType,
    depth: BitDepth,
    data: &[u8],
    palette: Option<&[u8]>,
) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = Encoder::new(&mut out, width, height);
    encoder.set_color(color);
    encoder.set_depth(depth);
    if let Some(palette) = palette {
        encoder.set_palette(palette.to_vec());
    }
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(data).unwrap();
    writer.finish().unwrap();
    out
}

fn solid(side: u32, px: [u8; 4]) -> Vec<u8> {
    let data: Vec<u8> = (0..side * side).flat_map(|_| px).collect();
    encode((side, side), ColorType::Rgba, BitDepth::Eight, &data, None)
}

fn pixel(image: &AvatarImage, x: usize, y: usize) -> [u8; 4] {
    let at = (y * usize::from(image.size) + x) * 4;
    image.rgba[at..at + 4].try_into().unwrap()
}

#[test]
fn decode_rgba_keeps_the_colour_and_masks_the_corners() {
    let image = from_source(&solid(128, [200, 40, 40, 255])).unwrap();
    assert_eq!(image.size, AVATAR_PX);
    assert_eq!(image.rgba.len(), OUT * OUT * 4);
    assert_eq!(pixel(&image, 32, 32), [200, 40, 40, 255]);
    for (x, y) in [(0, 0), (OUT - 1, 0), (0, OUT - 1), (OUT - 1, OUT - 1)] {
        assert_eq!(pixel(&image, x, y)[3], 0, "corner {x},{y}");
    }
    let edge = pixel(&image, 32, 0)[3];
    assert!(edge > 0 && edge < 255, "the rim is soft: {edge}");
}

#[test]
fn decode_every_colour_type_becomes_rgba() {
    let rgb = encode(
        (2, 2),
        ColorType::Rgb,
        BitDepth::Eight,
        &[10, 20, 30].repeat(4),
        None,
    );
    let gray = encode(
        (2, 2),
        ColorType::Grayscale,
        BitDepth::Eight,
        &[90; 4],
        None,
    );
    let gray_alpha = encode(
        (2, 2),
        ColorType::GrayscaleAlpha,
        BitDepth::Eight,
        &[90, 255].repeat(4),
        None,
    );
    let indexed = encode(
        (2, 2),
        ColorType::Indexed,
        BitDepth::Eight,
        &[0, 0, 0, 0],
        Some(&[5, 6, 7]),
    );
    let sixteen = encode(
        (2, 2),
        ColorType::Rgb,
        BitDepth::Sixteen,
        &[0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc].repeat(4),
        None,
    );
    let low_depth = encode(
        (2, 2),
        ColorType::Grayscale,
        BitDepth::One,
        &[0b0100_0000, 0b1000_0000],
        None,
    );
    let cases = [
        ("rgb", rgb, [10, 20, 30]),
        ("gray", gray, [90, 90, 90]),
        ("gray alpha", gray_alpha, [90, 90, 90]),
        ("indexed", indexed, [5, 6, 7]),
        ("16 bit", sixteen, [0x12, 0x56, 0x9a]),
    ];
    for (name, bytes, expected) in cases {
        let image = from_source(&bytes).unwrap_or_else(|| panic!("{name}"));
        let [r, g, b, a] = pixel(&image, 32, 32);
        assert_eq!([r, g, b], expected, "{name}");
        assert_eq!(a, 255, "{name}");
    }
    assert!(from_source(&low_depth).is_some(), "1-bit gray");
}

#[test]
fn decode_non_square_crops_the_centre() {
    // 192 x 64: left third red, middle blue, right third green.
    let mut data = Vec::new();
    for _ in 0..64 {
        for x in 0..192 {
            data.extend_from_slice(match x / 64 {
                0 => &[255, 0, 0, 255],
                1 => &[0, 0, 255, 255],
                _ => &[0, 255, 0, 255],
            });
        }
    }
    let bytes = encode((192, 64), ColorType::Rgba, BitDepth::Eight, &data, None);
    let image = from_source(&bytes).unwrap();
    assert_eq!(pixel(&image, 32, 32), [0, 0, 255, 255]);
    assert_eq!(pixel(&image, 12, 32), [0, 0, 255, 255]);
}

#[test]
fn decode_small_pictures_grow_smoothly() {
    let image = from_source(&solid(8, [1, 2, 3, 255])).unwrap();
    assert_eq!(pixel(&image, 32, 32), [1, 2, 3, 255]);
}

#[test]
fn decode_transparent_pixels_stay_transparent() {
    let image = from_source(&solid(64, [255, 255, 255, 0])).unwrap();
    assert_eq!(pixel(&image, 32, 32)[3], 0);
}

#[test]
fn decode_rejects_what_it_cannot_use() {
    assert!(from_source(b"").is_none(), "empty");
    assert!(from_source(b"GIF89a....").is_none(), "other format");
    assert!(
        from_source(b"<svg xmlns='http://www.w3.org/2000/svg'/>").is_none(),
        "svg"
    );
    let mut truncated = solid(32, [1, 2, 3, 255]);
    truncated.truncate(truncated.len() / 2);
    assert!(from_source(&truncated).is_none(), "truncated");
    let wide = encode(
        (2_000, 1),
        ColorType::Rgba,
        BitDepth::Eight,
        &vec![0; 8_000],
        None,
    );
    assert!(from_source(&wide).is_none(), "too wide");
    let big: Vec<u8> = (0..=AVATAR_MAX_BYTES as u32)
        .map(|i| (i % 251) as u8)
        .collect();
    let mut oversized = PNG_SIGNATURE.to_vec();
    oversized.extend(big);
    assert!(from_source(&oversized).is_none(), "too many bytes");
}

#[test]
fn stored_pictures_round_trip_without_a_second_mask() {
    let image = from_source(&solid(100, [9, 99, 199, 255])).unwrap();
    let png = to_png(&image).unwrap();
    assert!(png.len() < 16 * 1024);
    assert_eq!(from_stored(&png).unwrap(), image);
}

#[test]
fn stored_pictures_of_another_size_are_dropped() {
    assert!(from_stored(&solid(32, [1, 2, 3, 255])).is_none());
    assert!(from_stored(b"junk").is_none());
}
