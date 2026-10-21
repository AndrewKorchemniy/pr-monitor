//! Low-power hover: mouse moves inside the same table row are dropped before they wake egui,
//! so a hover sweep costs one frame per row crossed. The UI publishes the table geometry; the
//! bin's window-message hook asks `should_deliver`.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, Ordering};

/// Table geometry in physical pixels; `pitch_milli` is the row pitch × 1000.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TableGeometry {
    pub x0: i32,
    pub x1: i32,
    pub top: i32,
    pub bottom: i32,
    pub origin_y: i32,
    pub pitch_milli: i32,
}

impl TableGeometry {
    /// From logical points and the scale factor; `pitch` is how far apart rows start (their
    /// height plus the spacing between them).
    #[expect(
        clippy::cast_possible_truncation,
        reason = "window coordinates fit in i32"
    )]
    pub fn new(viewport: [f32; 4], scroll_offset: f32, pitch: f32, pixels_per_point: f32) -> Self {
        let px = |v: f32| (v * pixels_per_point).round() as i32;
        let [x0, top, x1, bottom] = viewport;
        Self {
            x0: px(x0),
            x1: px(x1),
            top: px(top),
            bottom: px(bottom),
            origin_y: px(top - scroll_offset),
            pitch_milli: (pitch * pixels_per_point * 1000.0).round().max(1.0) as i32,
        }
    }
}

/// The row band a point falls in; `None` outside the table.
pub(crate) fn band_of(geometry: &TableGeometry, x: i32, y: i32) -> Option<i64> {
    let inside =
        (geometry.x0..geometry.x1).contains(&x) && (geometry.top..geometry.bottom).contains(&y);
    if !inside || geometry.pitch_milli <= 0 {
        return None;
    }
    let offset = i64::from(y - geometry.origin_y) * 1000;
    Some(offset.div_euclid(i64::from(geometry.pitch_milli)))
}

/// No band yet (or the pointer is outside).
const NO_BAND: i64 = i64::MIN;

/// Atomics only so the hook (same UI thread) can read through a shared `Arc`.
#[derive(Debug)]
pub struct HoverBands {
    enabled: AtomicBool,
    x0: AtomicI32,
    x1: AtomicI32,
    top: AtomicI32,
    bottom: AtomicI32,
    origin_y: AtomicI32,
    pitch_milli: AtomicI32,
    last_band: AtomicI64,
}

impl Default for HoverBands {
    fn default() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            x0: AtomicI32::new(0),
            x1: AtomicI32::new(0),
            top: AtomicI32::new(0),
            bottom: AtomicI32::new(0),
            origin_y: AtomicI32::new(0),
            pitch_milli: AtomicI32::new(0),
            last_band: AtomicI64::new(NO_BAND),
        }
    }
}

impl HoverBands {
    pub fn publish(&self, geometry: Option<TableGeometry>, enabled: bool) {
        let on = enabled && geometry.is_some();
        self.enabled.store(on, Ordering::Relaxed);
        if let Some(g) = geometry {
            self.x0.store(g.x0, Ordering::Relaxed);
            self.x1.store(g.x1, Ordering::Relaxed);
            self.top.store(g.top, Ordering::Relaxed);
            self.bottom.store(g.bottom, Ordering::Relaxed);
            self.origin_y.store(g.origin_y, Ordering::Relaxed);
            self.pitch_milli.store(g.pitch_milli, Ordering::Relaxed);
        }
    }

    fn geometry(&self) -> TableGeometry {
        TableGeometry {
            x0: self.x0.load(Ordering::Relaxed),
            x1: self.x1.load(Ordering::Relaxed),
            top: self.top.load(Ordering::Relaxed),
            bottom: self.bottom.load(Ordering::Relaxed),
            origin_y: self.origin_y.load(Ordering::Relaxed),
            pitch_milli: self.pitch_milli.load(Ordering::Relaxed),
        }
    }

    /// Buttons held or outside the table: deliver. Same row band as last time: drop.
    pub fn should_deliver(&self, x: i32, y: i32, buttons: u32) -> bool {
        if !self.enabled.load(Ordering::Relaxed) || buttons != 0 {
            self.last_band.store(NO_BAND, Ordering::Relaxed);
            return true;
        }
        let Some(band) = band_of(&self.geometry(), x, y) else {
            self.last_band.store(NO_BAND, Ordering::Relaxed);
            return true;
        };
        self.last_band.swap(band, Ordering::Relaxed) != band
    }
}

#[cfg(test)]
mod tests;
