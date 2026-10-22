//! A thin progress line that moves the way a loader should. It fades in when a run starts,
//! glides toward each reported fraction instead of stepping (reports arrive ten times a
//! second at most), never runs backwards, and once the run is over sweeps to the end and
//! fades out, so even an instant refresh gets a clean acknowledgement. It asks for repaints
//! only while it is moving. With animations off it snaps to each report and vanishes with
//! the run.

use std::time::Duration;

use eframe::{
    egui::{self, Color32, Rect},
    emath::GuiRounding as _,
};
use prm_theme::metrics::Motion;

use crate::theme::UiTheme;

/// The line's thickness (points): it sits on a hairline divider.
pub(crate) const HEIGHT: f32 = 2.0;
/// Time constant of the glide toward a target (seconds): 95% of the gap closes in three.
const GLIDE: f32 = 0.12;
/// How long the sweep to the end takes once a run has finished (seconds).
const SWEEP: f32 = 0.20;
/// A glide this close to its target (a fraction of the line) has arrived.
const SETTLED: f32 = 0.0005;
/// Repaints while the line moves are paced to about this, not to every display frame.
const FRAME: Duration = Duration::from_millis(16);

#[derive(Debug, Default)]
pub(crate) struct ProgressLine {
    stage: Stage,
}

#[derive(Debug, Default)]
enum Stage {
    #[default]
    Hidden,
    /// A run is on: the fraction shown, when the run began and when the line last moved.
    Running { shown: f32, began: f64, last: f64 },
    /// The run is over: sweeping to the end from `from` since `finished`, then fading out.
    Finishing {
        from: f32,
        began: f64,
        finished: f64,
    },
}

/// What to paint: how much of the line and how strongly, and whether it is still moving (so
/// the next frame will differ and is worth asking for).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Shown {
    pub fraction: f32,
    pub opacity: f32,
    pub moving: bool,
}

impl ProgressLine {
    /// Ticks the line to the frame's time, paints it across `bar`, and keeps the window
    /// repainting while it moves.
    pub(crate) fn show(&mut self, ui: &egui::Ui, bar: Rect, target: Option<f32>, theme: &UiTheme) {
        let time = ui.input(|i| i.time);
        let Some(shown) = self.tick(target, time, theme) else {
            return;
        };
        paint(ui, bar, shown, theme.colors.accent_base);
        if shown.moving {
            ui.ctx().request_repaint_after(FRAME);
        }
    }

    /// Advances the line to `time` (seconds) toward `target`, the run's fraction (`None` once
    /// it is over), and returns what to paint, or `None` while there is nothing to show.
    pub(crate) fn tick(
        &mut self,
        target: Option<f32>,
        time: f64,
        theme: &UiTheme,
    ) -> Option<Shown> {
        if !theme.policy.animations {
            self.stage = Stage::Hidden;
            return target.map(|fraction| Shown {
                fraction: fraction.clamp(0.0, 1.0),
                opacity: 1.0,
                moving: false,
            });
        }
        let motion = &theme.tokens.metrics.motion;
        let (stage, shown) = match (std::mem::take(&mut self.stage), target) {
            (Stage::Hidden, None) => (Stage::Hidden, None),
            (Stage::Hidden, Some(_)) => {
                let stage = Stage::Running {
                    shown: 0.0,
                    began: time,
                    last: time,
                };
                (
                    stage,
                    Some(Shown {
                        fraction: 0.0,
                        opacity: 0.0,
                        moving: true,
                    }),
                )
            }
            (Stage::Running { shown, began, last }, Some(target)) => {
                // why: a phase, or another org joining, can report a lower fraction; the line
                // holds rather than runs backwards.
                let goal = target.clamp(shown, 1.0);
                let shown = glide(shown, goal, time - last, GLIDE);
                let opacity = ramp(time - began, motion.quick);
                let stage = Stage::Running {
                    shown,
                    began,
                    last: time,
                };
                let moving = opacity < 1.0 || goal - shown > SETTLED;
                (
                    stage,
                    Some(Shown {
                        fraction: shown,
                        opacity,
                        moving,
                    }),
                )
            }
            (Stage::Running { shown, began, .. }, None) => {
                finishing(shown, (began, time), time, motion)
            }
            // why: a run that starts while the last is still fading out waits for it; the line
            // completing and then starting afresh reads better than it jumping back.
            (
                Stage::Finishing {
                    from,
                    began,
                    finished,
                },
                _,
            ) => finishing(from, (began, finished), time, motion),
        };
        self.stage = stage;
        shown
    }
}

/// The finishing stage at `time`: the sweep to the end from `from`, then the fade, then gone.
fn finishing(
    from: f32,
    (began, finished): (f64, f64),
    time: f64,
    motion: &Motion,
) -> (Stage, Option<Shown>) {
    let since = time - finished;
    if since >= f64::from(SWEEP + motion.standard) {
        return (Stage::Hidden, None);
    }
    let shown = Shown {
        fraction: sweep(from, since),
        opacity: ramp(time - began, motion.quick)
            * (1.0 - ramp(since - f64::from(SWEEP), motion.standard)),
        moving: true,
    };
    let stage = Stage::Finishing {
        from,
        began,
        finished,
    };
    (stage, Some(shown))
}

/// `shown` moved toward `goal` over `dt` seconds, closing the gap exponentially with time
/// constant `tau`: the same motion whatever the frame rate.
fn glide(shown: f32, goal: f32, dt: f64, tau: f32) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a frame's length in seconds"
    )]
    let dt = dt.max(0.0) as f32;
    shown + (goal - shown) * (1.0 - (-dt / tau).exp())
}

/// The sweep to the end once a run is over: from `from`, easing out, at the end `SWEEP`
/// seconds after it started and never short of it.
fn sweep(from: f32, since: f64) -> f32 {
    let t = ramp(since, SWEEP);
    let eased = 1.0 - (1.0 - t).powi(3);
    from + (1.0 - from) * eased
}

/// 0 at the start of `duration` seconds, 1 from its end on.
fn ramp(elapsed: f64, duration: f32) -> f32 {
    if duration <= 0.0 {
        return 1.0;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "an animation's elapsed seconds"
    )]
    let elapsed = elapsed.max(0.0) as f32;
    (elapsed / duration).min(1.0)
}

/// The line from `bar`'s left edge, `fraction` of its width, snapped to whole pixels so its
/// end does not shimmer as it moves.
fn paint(ui: &egui::Ui, bar: Rect, shown: Shown, color: Color32) {
    let ppp = ui.ctx().pixels_per_point();
    let width = (bar.width() * shown.fraction).round_to_pixels(ppp);
    if width <= 0.0 || shown.opacity <= 0.0 {
        return;
    }
    let line = Rect::from_min_size(bar.min, egui::vec2(width, bar.height()));
    ui.painter()
        .rect_filled(line, 0.0, color.gamma_multiply(shown.opacity));
}

#[cfg(test)]
mod tests;
