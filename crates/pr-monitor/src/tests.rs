//! The icon encoders (shared with `build.rs`).

use crate::icon_gen::{IcoImage, ico, png};

fn square(size: u32, opaque: bool) -> Vec<u8> {
    let alpha = if opaque { 255 } else { 0 };
    (0..size * size).flat_map(|_| [10, 20, 30, alpha]).collect()
}

#[test]
fn png_round_trips_through_a_decoder() {
    let bytes = png(&square(4, true), 4).expect("encodes");
    let decoder = ::png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("valid png");
    let mut pixels = vec![0; reader.output_buffer_size().expect("sized")];
    reader.next_frame(&mut pixels).expect("frame");
    assert_eq!(&pixels[..4], &[10, 20, 30, 255]);
}

#[test]
fn ico_directory_points_at_each_image() {
    let images = [
        IcoImage {
            size: 16,
            rgba: square(16, false),
        },
        IcoImage {
            size: 256,
            rgba: square(256, true),
        },
    ];
    let bytes = ico(&images).expect("encodes");
    assert_eq!(&bytes[..6], &[0, 0, 1, 0, 2, 0]);
    let entry = |i: usize| &bytes[6 + 16 * i..6 + 16 * (i + 1)];
    assert_eq!(entry(0)[0], 16);
    assert_eq!(entry(1)[0], 0, "256 is written as 0");
    let offset =
        |i: usize| u32::from_le_bytes(entry(i)[12..16].try_into().expect("4 bytes")) as usize;
    assert_eq!(offset(0), 6 + 32);
    assert_eq!(
        &bytes[offset(1)..offset(1) + 4],
        b"\x89PNG",
        "the large image is a PNG"
    );
    // A 16 px DIB: 40-byte header, 16×16×4 pixels, 16 rows × 4 bytes of mask (all transparent).
    let dib = &bytes[offset(0)..offset(1)];
    assert_eq!(dib.len(), 40 + 16 * 16 * 4 + 16 * 4);
    assert_eq!(dib[40 + 16 * 16 * 4], 0xFF, "transparent pixels are masked");
}
