//! Turns a downloaded picture into the small round RGBA image the UI draws, and back to the
//! compact PNG kept on disk. Pure; run on the blocking pool.

#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "pixel coordinates and 8-bit channels are small, clamped and non-negative"
)]

use std::io::Cursor;

use png::{BitDepth, ColorType, Decoder, Encoder, Transformations};
use prm_domain::avatar::{AVATAR_PX, AvatarImage};

use crate::consts::{AVATAR_MAX_BYTES, AVATAR_MAX_SOURCE_PX};

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
const OUT: usize = AVATAR_PX as usize;

/// A picture as downloaded: any PNG colour type, up to `AVATAR_MAX_SOURCE_PX` on a side.
/// `None` for anything else (other formats, corrupt or oversized data).
pub(crate) fn from_source(bytes: &[u8]) -> Option<AvatarImage> {
    if bytes.len() > AVATAR_MAX_BYTES || !bytes.starts_with(&PNG_SIGNATURE) {
        return None;
    }
    let (width, height, rgba) = unpack(bytes)?;
    let side = width.min(height);
    let mut pixels = if width == height {
        resample(&rgba, side)
    } else {
        resample(&crop_square(&rgba, width, side), side)
    };
    mask_circle(&mut pixels);
    AvatarImage::new(AVATAR_PX, pixels)
}

/// A picture written by [`to_png`]: already the right size and masked.
pub(crate) fn from_stored(bytes: &[u8]) -> Option<AvatarImage> {
    let (width, height, rgba) = unpack(bytes)?;
    if width != OUT || height != OUT {
        return None;
    }
    AvatarImage::new(AVATAR_PX, rgba)
}

/// The compact PNG kept on disk for `image`.
pub(crate) fn to_png(image: &AvatarImage) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let side = u32::from(image.size);
    let mut encoder = Encoder::new(&mut out, side, side);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header().ok()?;
    writer.write_image_data(&image.rgba).ok()?;
    writer.finish().ok()?;
    Some(out)
}

/// Width, height and straight-alpha RGBA8 pixels.
fn unpack(bytes: &[u8]) -> Option<(usize, usize, Vec<u8>)> {
    let mut decoder = Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let (width, height) = reader.info().size();
    if width == 0 || height == 0 || width > AVATAR_MAX_SOURCE_PX || height > AVATAR_MAX_SOURCE_PX {
        return None;
    }
    let mut buf = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut buf).ok()?;
    buf.truncate(frame.buffer_size());
    let rgba = to_rgba(buf, frame.color_type, frame.bit_depth)?;
    Some((width as usize, height as usize, rgba))
}

fn to_rgba(data: Vec<u8>, color: ColorType, depth: BitDepth) -> Option<Vec<u8>> {
    if depth != BitDepth::Eight {
        return None;
    }
    let rgba = match color {
        ColorType::Rgba => data,
        ColorType::Rgb => data
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        ColorType::GrayscaleAlpha => data
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        ColorType::Grayscale => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        // EXPAND turns indexed pictures into RGB(A), so this is not reached.
        ColorType::Indexed => return None,
    };
    Some(rgba)
}

/// The centred `side` x `side` part of a `width`-wide image.
fn crop_square(rgba: &[u8], width: usize, side: usize) -> Vec<u8> {
    let height = rgba.len() / 4 / width;
    let (left, top) = ((width - side) / 2, (height - side) / 2);
    let mut out = Vec::with_capacity(side * side * 4);
    for row in top..top + side {
        let start = (row * width + left) * 4;
        out.extend_from_slice(&rgba[start..start + side * 4]);
    }
    out
}

/// Premultiplied colour of the source pixel at byte offset `at`.
fn premultiplied(rgba: &[u8], at: usize) -> [f32; 4] {
    let a = f32::from(rgba[at + 3]) / 255.0;
    [
        f32::from(rgba[at]) * a,
        f32::from(rgba[at + 1]) * a,
        f32::from(rgba[at + 2]) * a,
        a,
    ]
}

/// `side` x `side` to `OUT` x `OUT`: an area average when shrinking, bilinear when growing.
fn resample(rgba: &[u8], side: usize) -> Vec<u8> {
    let scale = side as f32 / OUT as f32;
    let mut out = Vec::with_capacity(OUT * OUT * 4);
    for y in 0..OUT {
        for x in 0..OUT {
            let sum = if scale >= 1.0 {
                area_average(rgba, side, (x, y), scale)
            } else {
                bilinear(rgba, side, (x, y), scale)
            };
            out.extend_from_slice(&straight(sum));
        }
    }
    out
}

fn area_average(rgba: &[u8], side: usize, (x, y): (usize, usize), scale: f32) -> [f32; 4] {
    let (x0, y0) = (x as f32 * scale, y as f32 * scale);
    let (x1, y1) = (x0 + scale, y0 + scale);
    let mut sum = [0.0; 4];
    let mut weight = 0.0;
    for sy in y0 as usize..(y1.ceil() as usize).min(side) {
        let wy = (sy as f32 + 1.0).min(y1) - (sy as f32).max(y0);
        for sx in x0 as usize..(x1.ceil() as usize).min(side) {
            let w = wy * ((sx as f32 + 1.0).min(x1) - (sx as f32).max(x0));
            let px = premultiplied(rgba, (sy * side + sx) * 4);
            sum.iter_mut().zip(px).for_each(|(s, p)| *s += p * w);
            weight += w;
        }
    }
    sum.map(|s| s / weight)
}

fn bilinear(rgba: &[u8], side: usize, (x, y): (usize, usize), scale: f32) -> [f32; 4] {
    let max = (side - 1) as f32;
    let fx = ((x as f32 + 0.5) * scale - 0.5).clamp(0.0, max);
    let fy = ((y as f32 + 0.5) * scale - 0.5).clamp(0.0, max);
    let (left, top) = (fx as usize, fy as usize);
    let (tx, ty) = (fx - left as f32, fy - top as f32);
    let (right, bottom) = ((left + 1).min(side - 1), (top + 1).min(side - 1));
    let at = |sx: usize, sy: usize| premultiplied(rgba, (sy * side + sx) * 4);
    let [p00, p10, p01, p11] = [
        at(left, top),
        at(right, top),
        at(left, bottom),
        at(right, bottom),
    ];
    let mut out = [0.0; 4];
    for (i, slot) in out.iter_mut().enumerate() {
        let top = p00[i] + (p10[i] - p00[i]) * tx;
        let bottom = p01[i] + (p11[i] - p01[i]) * tx;
        *slot = top + (bottom - top) * ty;
    }
    out
}

/// Premultiplied floats back to straight-alpha bytes.
fn straight([r, g, b, a]: [f32; 4]) -> [u8; 4] {
    let unit = |v: f32| (v.clamp(0.0, 255.0) + 0.5) as u8;
    if a <= 0.0 {
        return [0; 4];
    }
    [unit(r / a), unit(g / a), unit(b / a), unit(a * 255.0)]
}

/// Fades alpha to zero outside the inscribed circle, with a one-pixel soft edge.
fn mask_circle(rgba: &mut [u8]) {
    let radius = OUT as f32 / 2.0;
    for (i, px) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let dx = (i % OUT) as f32 + 0.5 - radius;
        let dy = (i / OUT) as f32 + 0.5 - radius;
        let coverage = (radius - dx.hypot(dy) + 0.5).clamp(0.0, 1.0);
        px[3] = (f32::from(px[3]) * coverage + 0.5) as u8;
    }
}

#[cfg(test)]
mod tests;
