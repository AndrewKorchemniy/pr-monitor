//! Touchpad scrolling: high-resolution wheel deltas are applied in the frame they arrive, the
//! way browsers apply them, instead of being eased in like the notches of a mouse wheel.

use eframe::egui::{Event, MouseWheelSource, MouseWheelUnit, TouchPhase, Vec2};

/// Marks this frame's high-resolution wheel deltas as a touch gesture, so egui applies them
/// at once.
///
/// winit reports every Windows `WM_MOUSEWHEEL` as whole lines (the delta over 120) with an
/// unknown source and no gesture phases, and egui eases every line delta in (90% within
/// 100 ms, through `unprocessed_wheel_delta`), repainting until the rest has drained, as it
/// would a coarse wheel notch. A precision touchpad sends a stream of fractions of a notch,
/// and after the fingers lift Windows keeps sending its momentum the same way, already eased.
/// Eased a second time, the content runs behind the fingers, out of step with the momentum,
/// and keeps moving after the stream stops. egui skips the easing between a `Start` and the
/// end of a touch gesture (which, without an `End`, it ends after a short pause), so a
/// zero-length `Start` ahead of the first fractional delta gets the stream applied as it
/// lands, the way browsers apply precise deltas. Whole notches, from an ordinary mouse wheel,
/// keep egui's easing.
pub(crate) fn mark_precise_scroll(events: &mut Vec<Event>) {
    let Some(index) = events.iter().position(is_precise_wheel) else {
        return;
    };
    let Some(Event::MouseWheel {
        unit,
        source,
        modifiers,
        ..
    }) = events.get(index)
    else {
        return;
    };
    let start = Event::MouseWheel {
        unit: *unit,
        delta: Vec2::ZERO,
        phase: TouchPhase::Start,
        source: *source,
        modifiers: *modifiers,
    };
    events.insert(index, start);
}

/// A line delta that is not a whole number of notches, from a backend that does not say what
/// drives the scrolling (macOS does, and gets its trackpad phases from the OS).
fn is_precise_wheel(event: &Event) -> bool {
    let Event::MouseWheel {
        unit,
        delta,
        phase,
        source,
        ..
    } = event
    else {
        return false;
    };
    *unit == MouseWheelUnit::Line
        && *phase == TouchPhase::Move
        && *source == MouseWheelSource::Unknown
        && (has_fraction(delta.x) || has_fraction(delta.y))
}

fn has_fraction(lines: f32) -> bool {
    lines.fract().abs() > 0.0
}

#[cfg(test)]
mod tests;
