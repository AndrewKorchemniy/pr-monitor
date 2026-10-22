//! Touchpad scrolling on Windows, as it reaches egui: high-resolution wheel deltas applied in
//! the frame they arrive, the way browsers apply them, and the stray reversed event the
//! touchpad slips into a steady stream dropped before it can jolt the page.
//!
//! winit reports every Windows `WM_MOUSEWHEEL` as whole lines (the delta over 120) with an
//! unknown source and no gesture phases. A precision touchpad sends a stream of fractions of a
//! notch, and after the fingers lift Windows keeps sending its momentum the same way.

use eframe::egui::{Event, MouseWheelSource, MouseWheelUnit, TouchPhase, Vec2};

/// How many events one way before an event the other way is suspect.
const RUN: u32 = 3;
/// Frames without a touchpad event after which the stream is over: a held reversal goes out,
/// and a new stream may start either way.
const IDLE_FRAMES: u32 = 10;

/// The filter's memory between frames: the direction the stream has been running, for how many
/// events, and a reversal held back until the next event shows what it was.
#[derive(Debug, Default)]
pub(crate) struct TouchpadScroll {
    run: Option<Run>,
    held: Option<Held>,
    idle_frames: u32,
}

#[derive(Debug, Clone, Copy)]
struct Run {
    direction: Direction,
    len: u32,
}

#[derive(Debug)]
struct Held {
    event: Event,
    direction: Direction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
}

impl TouchpadScroll {
    /// Rewrites a frame's events: logs the wheel events (target `prm::scroll`, for diagnosing
    /// scroll trouble from a log), drops the stray reversal inside a steady stream, and marks
    /// the touchpad deltas as a gesture so egui applies them at once.
    pub(crate) fn rewrite(&mut self, events: &mut Vec<Event>) {
        log_wheel(events);
        self.drop_glitches(events);
        mark_gesture(events);
    }

    /// Held into the top or bottom of a page, the stream is all one way. Now and then one event
    /// arrives the other way and the stream goes straight on: a small stray mid-stream, or, as
    /// one finger lifts, a spike of up to 25 lines followed by a small step back the old way.
    /// Either moved the page off the edge for a frame, a flash. A real turn is different: more
    /// reversed events follow. So the first reversal after a run is held until the next event
    /// says which it was, and released on its own once the touchpad has gone quiet.
    fn drop_glitches(&mut self, events: &mut Vec<Event>) {
        let mut seen = false;
        let mut kept = Vec::with_capacity(events.len() + 1);
        for event in events.drain(..) {
            match direction_of(&event) {
                Some(direction) => {
                    seen = true;
                    self.place(event, direction, &mut kept);
                }
                None => kept.push(event),
            }
        }
        *events = kept;
        self.idle_frames = if seen { 0 } else { self.idle_frames + 1 };
        if self.idle_frames > IDLE_FRAMES {
            self.run = None;
            events.extend(self.held.take().map(|held| held.event));
        }
    }

    /// Keeps, holds or drops one touchpad event going `direction`, appending what passes.
    fn place(&mut self, event: Event, direction: Direction, kept: &mut Vec<Event>) {
        if let Some(held) = self.held.take() {
            if held.direction == direction {
                // A second reversed event: a real turn, and the first one goes out ahead.
                kept.push(held.event);
                self.run = Some(Run { direction, len: 2 });
            } else {
                // The stream went straight on: the held reversal was noise.
                self.extend_run(direction);
            }
            kept.push(event);
            return;
        }
        match self.run {
            Some(run) if run.direction == direction => {
                self.extend_run(direction);
                kept.push(event);
            }
            Some(run) if run.len >= RUN => {
                self.held = Some(Held { event, direction });
            }
            Some(_) | None => {
                self.run = Some(Run { direction, len: 1 });
                kept.push(event);
            }
        }
    }

    fn extend_run(&mut self, direction: Direction) {
        let len = self.run.map_or(0, |run| run.len) + 1;
        self.run = Some(Run { direction, len });
    }
}

/// egui eases every line delta in (90% within 100 ms) and repaints until the rest has drained,
/// as it would a coarse wheel notch. Eased a second time, a touchpad stream runs behind the
/// fingers, out of step with the momentum, and keeps moving after it stops. egui skips the
/// easing between a `Start` and the end of a touch gesture (which, without an `End`, it ends
/// after a short pause), so a zero-length `Start` ahead of the first touchpad delta gets the
/// stream applied as it lands. Whole notches, from an ordinary mouse wheel, keep the easing.
fn mark_gesture(events: &mut Vec<Event>) {
    let Some(index) = events.iter().position(is_touchpad) else {
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

/// A fractional line delta from a backend that does not say what drives the scrolling (macOS
/// does, and gets its trackpad phases from the OS): what the touchpad sends.
fn touchpad_delta(event: &Event) -> Option<Vec2> {
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
    (delta.x.fract() != 0.0 || delta.y.fract() != 0.0).then_some(*delta)
}

fn is_touchpad(event: &Event) -> bool {
    touchpad_delta(event).is_some()
}

/// The vertical direction of a touchpad event; `None` for other events and horizontal deltas.
fn direction_of(event: &Event) -> Option<Direction> {
    let delta = touchpad_delta(event)?;
    if delta.y > 0.0 {
        Some(Direction::Up)
    } else if delta.y < 0.0 {
        Some(Direction::Down)
    } else {
        None
    }
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
