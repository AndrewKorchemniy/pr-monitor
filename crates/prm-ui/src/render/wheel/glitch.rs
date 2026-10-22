//! Drops the lone reversed wheel event a precision touchpad sends in the middle of a steady
//! stream. Held into the top or bottom of a page, the stream is all one way, yet now and then
//! one small event arrives the other way; the content jumped away from the edge for a frame
//! and snapped back with the next event, a flash the user sees as the page bouncing.

use eframe::egui::Event;

/// How many events one way before a single event the other way counts as a glitch.
const RUN: u32 = 3;
/// A reversed event larger than this many lines is a real change of direction.
const SMALL: f32 = 1.0;

/// The filter's memory between frames: how long the stream has run one way, and a reversed
/// event held back until the next event shows whether the stream went on without it.
#[derive(Debug, Default)]
pub(crate) struct GlitchFilter {
    /// The stream's direction (the sign of its deltas' y) and how many events it has run.
    run: Option<Run>,
    held: Option<Event>,
}

#[derive(Debug, Clone, Copy)]
struct Run {
    /// `Direction::Up` for positive deltas, `Down` for negative.
    dir: Direction,
    len: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
}

impl Direction {
    fn of(y: f32) -> Option<Self> {
        if y > 0.0 {
            Some(Self::Up)
        } else if y < 0.0 {
            Some(Self::Down)
        } else {
            None
        }
    }
}

/// What becomes of one event.
enum Step {
    Keep(Event),
    Hold,
    /// The held event, then this one.
    Release(Event, Event),
}

impl GlitchFilter {
    /// Rewrites a frame's events. A lone small reversal inside a run is held back; it is dropped
    /// when the stream carries on the old way, released ahead of the next event when the stream
    /// follows it, and released on its own when the stream stops.
    pub(crate) fn apply(&mut self, events: &mut Vec<Event>) {
        if self.held.is_some() && !events.iter().any(is_precise) {
            // why: the stream stopped on the reversal; a real, if small, scroll is kept.
            let held = self.held.take();
            events.extend(held);
            self.run = None;
        }
        let mut kept = Vec::with_capacity(events.len() + 1);
        for event in events.drain(..) {
            match self.step(event) {
                Step::Keep(event) => kept.push(event),
                Step::Hold => {}
                Step::Release(held, event) => {
                    kept.push(held);
                    kept.push(event);
                }
            }
        }
        *events = kept;
    }

    fn step(&mut self, event: Event) -> Step {
        let Some(dir) = precise_direction(&event) else {
            return self.flush_with(event);
        };
        let held = self.held.take();
        match self.run {
            Some(run) if run.dir == dir => {
                // The stream carries on; a held reversal (if any) was noise and is dropped.
                self.run = Some(Run {
                    dir,
                    len: run.len + 1,
                });
                Step::Keep(event)
            }
            Some(run) if held.is_none() && run.len >= RUN && is_small(&event) => {
                self.held = Some(event);
                Step::Hold
            }
            Some(_) | None => {
                // A real change of direction (or a stream too young to judge).
                let len = if held.is_some() { 2 } else { 1 };
                self.run = Some(Run { dir, len });
                match held {
                    Some(held) => Step::Release(held, event),
                    None => Step::Keep(event),
                }
            }
        }
    }

    /// Anything that is not a precise wheel event (a pointer move, a gesture start, a mouse
    /// notch) passes through; a held reversal goes out ahead of a wheel event of another kind.
    fn flush_with(&mut self, event: Event) -> Step {
        if matches!(event, Event::MouseWheel { .. }) {
            self.run = None;
            if let Some(held) = self.held.take() {
                return Step::Release(held, event);
            }
        }
        Step::Keep(event)
    }
}

fn is_precise(event: &Event) -> bool {
    precise_direction(event).is_some()
}

/// The direction of a fractional line delta's y, for the events the touchpad sends.
fn precise_direction(event: &Event) -> Option<Direction> {
    if !super::is_precise_wheel(event) {
        return None;
    }
    if let Event::MouseWheel { delta, .. } = event {
        Direction::of(delta.y)
    } else {
        None
    }
}

fn is_small(event: &Event) -> bool {
    if let Event::MouseWheel { delta, .. } = event {
        delta.y.abs() < SMALL
    } else {
        false
    }
}

#[cfg(test)]
mod tests;
