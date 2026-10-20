//! When the next repaint is due while visible. Hidden windows never schedule repaints.

use std::time::Duration;

use prm_domain::Timestamp;

/// One second: the countdown tick in the last minute before a refresh.
const SECOND: Duration = Duration::from_secs(1);
const MINUTE_SECS: i64 = 60;

#[derive(Clone, Debug, Default)]
pub(crate) struct RepaintInputs {
    pub visible: bool,
    pub focused: bool,
    pub low_power: bool,
    pub now: Option<Timestamp>,
    pub next_refresh: Option<Timestamp>,
    /// Other deadlines: selection dwell, notice expiry, UI-state save.
    pub deadlines: Vec<Timestamp>,
}

/// The earliest of: the next wall-clock minute (ages, "Updated 2m ago"), one second while a
/// focused countdown is under a minute, and every deadline.
pub(crate) fn next_repaint(input: &RepaintInputs) -> Option<Duration> {
    let now = input.now?;
    if !input.visible {
        return None;
    }
    let into_minute = now.unix().rem_euclid(MINUTE_SECS);
    let mut next = Duration::from_secs(u64::try_from(MINUTE_SECS - into_minute).unwrap_or(60));
    let countdown = input.focused
        && !input.low_power
        && input
            .next_refresh
            .is_some_and(|at| at > now && at.since(now) < Duration::from_secs(60));
    if countdown {
        next = next.min(SECOND);
    }
    for deadline in &input.deadlines {
        let wait = if *deadline > now {
            deadline.since(now)
        } else {
            Duration::ZERO
        };
        next = next.min(wait);
    }
    Some(next)
}

#[cfg(test)]
mod tests;
