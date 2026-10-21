//! Pure scheduling: when each org refreshes next. Deadline-based, so a missed deadline fires once
//! and is rescheduled from completion (never a burst).

use std::time::Duration;

use prm_ado::AdoError;
use prm_config::schema::RefreshConfig;
use prm_domain::{OrgId, Timestamp};
use tokio::time::Instant;

use crate::{
    consts::{
        AUTH_PROBE_INTERVAL, COALESCE_WINDOW, DEFAULT_RETRY_AFTER, FAILURE_BACKOFF_BASE,
        HEADROOM_LOW, HEADROOM_OK, MAX_RATE_MULTIPLIER, RESUME_GAP,
    },
    state::PauseReason,
};

/// One instant on both clocks: monotonic for timers, wall for display and resume detection.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct Moment {
    pub mono: Instant,
    pub wall: Timestamp,
}

impl Moment {
    /// The wall time of a monotonic instant, assuming no suspend in between.
    pub(crate) fn wall_of(self, at: Instant) -> Timestamp {
        if at >= self.mono {
            self.wall.saturating_add(at - self.mono)
        } else {
            self.wall.saturating_sub(self.mono - at)
        }
    }

    /// The monotonic instant of a wall time, assuming no suspend in between.
    pub(crate) fn mono_of(self, at: Timestamp) -> Instant {
        if at >= self.wall {
            self.mono + at.since(self.wall)
        } else {
            self.mono
                .checked_sub(self.wall.since(at))
                .unwrap_or(self.mono)
        }
    }

    fn after(self, delay: Duration) -> Self {
        Self {
            mono: self.mono + delay,
            wall: self.wall.saturating_add(delay),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Schedule {
    /// The next refresh, on both clocks (monotonic for the timer, wall for display).
    pub due: Option<Moment>,
    /// 1..=`MAX_RATE_MULTIPLIER`; stretches the interval while rate-limit headroom is low.
    pub multiplier: u32,
    pub failures: u32,
    pub paused: Option<PauseReason>,
    pub next_probe: Option<Instant>,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            due: None,
            multiplier: 1,
            failures: 0,
            paused: None,
            next_probe: None,
        }
    }
}

impl Schedule {
    pub(crate) fn due_at(&mut self, now: Moment, delay: Duration) {
        self.due = Some(now.after(delay));
    }

    /// Nothing scheduled (paused, disabled, or refreshing now).
    pub(crate) const fn clear_due(&mut self) {
        self.due = None;
    }

    /// When the timer fires.
    pub(crate) fn due_mono(&self) -> Option<Instant> {
        self.due.map(|due| due.mono)
    }

    /// When the refresh is shown as due.
    pub(crate) fn due_wall(&self) -> Option<Timestamp> {
        self.due.map(|due| due.wall)
    }

    pub(crate) fn due_now(&mut self, now: Moment) {
        self.due_at(now, Duration::ZERO);
    }

    /// Pulls the deadline in to `now + delay` if it is later (or unset).
    pub(crate) fn due_no_later_than(&mut self, now: Moment, delay: Duration) {
        if self.due_mono().is_none_or(|due| due > now.mono + delay) {
            self.due_at(now, delay);
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct Policy {
    pub interval: Duration,
    pub jitter_pct: u8,
}

impl Policy {
    pub(crate) fn from_config(cfg: &RefreshConfig) -> Self {
        Self {
            interval: Duration::from_mins(u64::from(cfg.interval_minutes)),
            jitter_pct: cfg.jitter_percent,
        }
    }

    /// How often an org paused for sign-in is probed: never more often than it would refresh.
    pub(crate) fn probe_interval(self) -> Duration {
        self.interval.max(AUTH_PROBE_INTERVAL)
    }
}

/// `base · (1 ± U(0, pct)/100)`.
pub(crate) fn jittered(base: Duration, pct: u8, rng: &mut fastrand::Rng) -> Duration {
    if pct == 0 {
        return base;
    }
    let spread = base.mul_f64(f64::from(rng.u8(0..=pct)) / 100.0);
    if rng.bool() {
        base.saturating_add(spread)
    } else {
        base.saturating_sub(spread)
    }
}

/// Low headroom doubles the multiplier; comfortable headroom halves it (hysteresis between).
pub(crate) fn adjust_multiplier(current: u32, headroom: Option<f64>) -> u32 {
    match headroom {
        Some(h) if h < HEADROOM_LOW => (current.saturating_mul(2)).min(MAX_RATE_MULTIPLIER),
        Some(h) if h >= HEADROOM_OK => (current / 2).max(1),
        _ => current.clamp(1, MAX_RATE_MULTIPLIER),
    }
}

fn stretched(policy: Policy, multiplier: u32) -> Duration {
    policy.interval.saturating_mul(multiplier)
}

pub(crate) fn after_success(
    policy: Policy,
    schedule: &mut Schedule,
    now: Moment,
    headroom: Option<f64>,
    rng: &mut fastrand::Rng,
) {
    schedule.failures = 0;
    schedule.multiplier = adjust_multiplier(schedule.multiplier, headroom);
    let delay = jittered(
        stretched(policy, schedule.multiplier),
        policy.jitter_pct,
        rng,
    );
    schedule.due_at(now, delay);
}

/// How a refresh failed, as far as scheduling cares.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum FailureKind {
    RateLimited { retry_after: Option<Duration> },
    Other,
}

impl FailureKind {
    pub(crate) fn of(error: &AdoError) -> Self {
        match error {
            AdoError::RateLimited { retry_after } => Self::RateLimited {
                retry_after: *retry_after,
            },
            AdoError::Auth(_)
            | AdoError::Forbidden { .. }
            | AdoError::NotFound { .. }
            | AdoError::Server { .. }
            | AdoError::Http { .. }
            | AdoError::ProxyAuthRequired
            | AdoError::Network(_)
            | AdoError::Timeout
            | AdoError::Decode(_)
            | AdoError::Cancelled => Self::Other,
        }
    }
}

pub(crate) fn after_failure(
    policy: Policy,
    schedule: &mut Schedule,
    now: Moment,
    failure: FailureKind,
    rng: &mut fastrand::Rng,
) {
    if let FailureKind::RateLimited { retry_after } = failure {
        schedule.multiplier = (schedule.multiplier.saturating_mul(2)).min(MAX_RATE_MULTIPLIER);
        let wait = retry_after.unwrap_or(DEFAULT_RETRY_AFTER);
        schedule.due_at(now, wait.max(stretched(policy, schedule.multiplier)));
        return;
    }
    schedule.failures = schedule.failures.saturating_add(1);
    let exponent = schedule.failures.saturating_sub(1).min(16);
    let backoff = FAILURE_BACKOFF_BASE.saturating_mul(1_u32 << exponent);
    let delay = jittered(backoff.min(policy.interval), policy.jitter_pct, rng);
    schedule.due_at(now, delay);
}

/// One org's view for [`due_orgs`].
pub(crate) struct DueCheck<'a> {
    pub org: &'a OrgId,
    pub schedule: &'a Schedule,
    pub in_flight: bool,
}

/// Orgs due now or within `COALESCE_WINDOW`, so near-simultaneous orgs share one cycle.
pub(crate) fn due_orgs<'a>(
    orgs: impl IntoIterator<Item = DueCheck<'a>>,
    now: Instant,
) -> Vec<OrgId> {
    orgs.into_iter()
        .filter(|c| c.schedule.paused.is_none() && !c.in_flight)
        .filter(|c| {
            c.schedule
                .due_mono()
                .is_some_and(|due| due <= now + COALESCE_WINDOW)
        })
        .map(|c| c.org.clone())
        .collect()
}

/// The machine slept since `previous`: wall time ran ahead of monotonic time by more than
/// `RESUME_GAP` (a monotonic clock that stops in suspend), or the timer that was due at
/// `deadline` fired more than `RESUME_GAP` late (one that keeps counting, as on Windows).
pub(crate) fn detect_resume(previous: Moment, now: Moment, deadline: Instant) -> bool {
    let wall_gap = now.wall.since(previous.wall);
    let mono_gap = now.mono.saturating_duration_since(previous.mono);
    let overslept = now.mono.saturating_duration_since(deadline);
    wall_gap.saturating_sub(mono_gap) > RESUME_GAP || overslept > RESUME_GAP
}

#[cfg(test)]
mod tests;
