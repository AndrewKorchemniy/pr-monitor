//! Touchpad scrolling on Windows, as it reaches egui: high-resolution wheel deltas applied in
//! the frame they arrive, the way browsers apply them, and the stray reversed event the
//! touchpad slips into a steady stream dropped before it can jolt the page.
//!
//! winit reports every Windows `WM_MOUSEWHEEL` as whole lines (the delta over 120) with an
//! unknown source and no gesture phases. A precision touchpad sends a stream of fractions of a
//! notch, and after the fingers lift Windows keeps sending its momentum the same way.

use eframe::egui::{Event, MouseWheelSource, MouseWheelUnit, TouchPhase, Vec2};

/// How many events one way before a single small event the other way counts as a glitch.
const RUN: u32 = 3;
/// A reversed event of this many lines or more is a flick back, never a glitch.
const SMALL: f32 = 1.0;
/// Frames without a touchpad event after which the stream is over and a new one may start
/// either way.
const IDLE_FRAMES: u32 = 10;

/// The filter's memory between frames: the direction the stream has been running, for how many
/// events, and whether the last event was a reversal it dropped.
#[derive(Debug, Default)]
pub(crate) struct TouchpadScroll {
    run: Option<Run>,
    /// The last event was a small reversal that was dropped; a second one confirms a turn.
    dropped: bool,
    idle_frames: u32,
}

#[derive(Debug, Clone, Copy)]
struct Run {
    direction: Direction,
    len: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
}

/// A wheel event the touchpad sends: a fractional line delta from a backend that does not say
/// what drives the scrolling (macOS does, and gets its trackpad phases from the OS).
struct Precise {
    /// Vertical direction; `None` for a purely horizontal delta.
    direction: Option<Direction>,
    small: bool,
}

impl TouchpadScroll {
    /// Rewrites a frame's events: logs the wheel events (target `prm::scroll`, for diagnosing
    /// scroll trouble from a log), drops a lone reversal inside a steady stream, and marks the
    /// touchpad deltas as a gesture so egui applies them at once.
    pub(crate) fn rewrite(&mut self, events: &mut Vec<Event>) {
        log_wheel(events);
        self.drop_glitches(events);
        mark_gesture(events);
    }

    /// Held into the top or bottom of a page, the stream is all one way, yet now and then one
    /// small event arrives the other way (often as a finger lifts); the page jumped away from
    /// the edge and snapped back, a flash. The first small reversal after a run is dropped; a
    /// second reversed event is a real turn and passes, so a turn loses at most one small step.
    fn drop_glitches(&mut self, events: &mut Vec<Event>) {
        let mut seen = false;
        events.retain(|event| {
            let Some(direction) = precise(event).and_then(|p| p.direction) else {
                return true;
            };
            seen = true;
            let small = precise(event).is_some_and(|p| p.small);
            self.keep(direction, small)
        });
        self.idle_frames = if seen { 0 } else { self.idle_frames + 1 };
        if self.idle_frames > IDLE_FRAMES {
            self.run = None;
            self.dropped = false;
        }
    }

    /// Whether to keep an event going `direction`, and the run after it.
    fn keep(&mut self, direction: Direction, small: bool) -> bool {
        match self.run {
            Some(run) if run.direction == direction => {
                self.run = Some(Run {
                    direction,
                    len: run.len + 1,
                });
                self.dropped = false;
                true
            }
            Some(run) if run.len >= RUN && small && !self.dropped => {
                self.dropped = true;
                false
            }
            Some(_) | None => {
                self.run = Some(Run { direction, len: 1 });
                self.dropped = false;
                true
            }
        }
    }
}

/// egui eases every line delta in (90% within 100 ms) and repaints until the rest has drained,
/// as it would a coarse wheel notch. Eased a second time, a touchpad stream runs behind the
/// fingers, out of step with the momentum, and keeps moving after it stops. egui skips the
/// easing between a `Start` and the end of a touch gesture (which, without an `End`, it ends
/// after a short pause), so a zero-length `Start` ahead of the first touchpad delta gets the
/// stream applied as it lands. Whole notches, from an ordinary mouse wheel, keep the easing.
fn mark_gesture(events: &mut Vec<Event>) {
    let Some(index) = events.iter().position(|e| precise(e).is_some()) else {
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

fn precise(event: &Event) -> Option<Precise> {
    let Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta,
        phase: TouchPhase::Move,
        source: MouseWheelSource::Unknown,
        ..
    } = event
    else {
        return None;
    };
    let fractional = delta.x.fract() != 0.0 || delta.y.fract() != 0.0;
    fractional.then(|| Precise {
        direction: if delta.y > 0.0 {
            Some(Direction::Up)
        } else if delta.y < 0.0 {
            Some(Direction::Down)
        } else {
            None
        },
        small: delta.y.abs() < SMALL,
    })
}

fn log_wheel(events: &[Event]) {
    for event in events {
        if let Event::MouseWheel {
            unit,
            delta,
            phase,
            source,
            ..
        } = event
        {
            tracing::debug!(
                target: "prm::scroll",
                ?unit, dx = delta.x, dy = delta.y, ?phase, ?source,
                "wheel"
            );
        }
    }
}

#[cfg(test)]
mod tests;
