//! PNG and ICO encoders for the brand mark (rendered by `prm_theme::render_mark`). Pure and
//! deterministic; also compiled into the bin's tests.

use std::error::Error;

/// RGBA pixels (`size`×`size`) as a PNG.
pub(crate) fn png(rgba: &[u8], size: u32) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, size, size);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(rgba)?;
    }
    Ok(out)
}

/// One ICO image: `size` and its RGBA pixels.
pub(crate) struct IcoImage {
    pub(crate) size: u32,
    pub(crate) rgba: Vec<u8>,
}

/// An ICO with every image; 256 px is stored as PNG, smaller sizes as 32-bpp DIBs.
pub(crate) fn ico(images: &[IcoImage]) -> Result<Vec<u8>, Box<dyn Error>> {
    let payloads: Vec<Vec<u8>> = images
        .iter()
        .map(|image| {
            if image.size >= 256 {
                png(&image.rgba, image.size)
            } else {
                Ok(dib(&image.rgba, image.size))
            }
        })
        .collect::<Result<_, _>>()?;
    let count = u16::try_from(images.len())?;
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    let mut offset = 6 + 16 * u32::from(count);
    for (image, payload) in images.iter().zip(&payloads) {
        let side = u8::try_from(image.size).unwrap_or(0);
        out.extend_from_slice(&[side, side, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        let len = u32::try_from(payload.len())?;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += len;
    }
    for payload in payloads {
        out.extend_from_slice(&payload);
    }
    Ok(out)
}

/// `BITMAPINFOHEADER`, bottom-up BGRA rows, then the 1-bpp AND mask (set where transparent).
fn dib(rgba: &[u8], size: u32) -> Vec<u8> {
    let side = size as usize;
    let mut out = Vec::new();
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&(size * 2).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&[0; 24]);
    for row in rgba.chunks(side * 4).rev() {
        for px in row.chunks(4) {
            out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
    }
    let stride = side.div_ceil(32) * 4;
    for row in rgba.chunks(side * 4).rev() {
        let mut mask = vec![0u8; stride];
        for (x, px) in row.chunks(4).enumerate() {
            if px[3] == 0 {
                mask[x / 8] |= 0x80 >> (x % 8);
            }
        }
        out.extend_from_slice(&mask);
    }
    out
}
