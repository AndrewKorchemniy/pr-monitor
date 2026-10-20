//! Rate-limit awareness. Azure DevOps may send `Retry-After` on any response (even 200); the
//! gate makes every later request from this org wait it out.

use std::{
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use http::HeaderMap;
use prm_domain::Timestamp;
use tokio::time::Instant;

use crate::consts::{
    H_RETRY_AFTER, H_RL_DELAY, H_RL_LIMIT, H_RL_REMAINING, H_RL_RESET, H_RL_RESOURCE,
};

/// Longest `Retry-After` honoured; longer values are treated as this.
pub const MAX_RETRY_AFTER: Duration = Duration::from_secs(120);

/// The latest `X-RateLimit-*` values seen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RateSnapshot {
    /// Tokens (TSTUs) in the window.
    pub limit: Option<f64>,
    pub remaining: Option<f64>,
    pub reset: Option<Timestamp>,
    /// Seconds the server already delayed this request.
    pub delay_applied: Option<f64>,
    pub resource: Option<Arc<str>>,
    pub throttled_until: Option<Instant>,
}

impl RateSnapshot {
    /// `remaining / limit` (0..=1) when both are known.
    pub fn headroom(&self) -> Option<f64> {
        match (self.remaining, self.limit) {
            (Some(remaining), Some(limit)) if limit > 0.0 => {
                Some((remaining / limit).clamp(0.0, 1.0))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
struct GateState {
    not_before: Option<Instant>,
    snapshot: RateSnapshot,
}

/// Shared by all requests of one org. The lock is never held across an `.await`.
#[derive(Debug, Default)]
pub struct RateGate {
    state: Mutex<GateState>,
}

impl RateGate {
    /// Waits until any `Retry-After` window has passed.
    pub async fn wait(&self) {
        while let Some(until) = self.not_before().filter(|t| *t > Instant::now()) {
            tokio::time::sleep_until(until).await;
        }
    }

    /// Records rate headers from a response.
    pub fn observe(&self, headers: &HeaderMap) {
        if let Some(wait) = parse_retry_after(headers) {
            self.push(wait);
        }
        let mut state = self.lock();
        let snap = &mut state.snapshot;
        snap.limit = header_f64(headers, H_RL_LIMIT).or(snap.limit);
        snap.remaining = header_f64(headers, H_RL_REMAINING).or(snap.remaining);
        snap.delay_applied = header_f64(headers, H_RL_DELAY);
        snap.reset = header_f64(headers, H_RL_RESET)
            .and_then(|secs| float_to_i64(secs).and_then(Timestamp::from_unix))
            .or(snap.reset);
        if let Some(resource) = header_str(headers, H_RL_RESOURCE) {
            snap.resource = Some(Arc::from(resource));
        }
    }

    /// Makes every request wait at least `wait` from now.
    pub fn push(&self, wait: Duration) {
        let until = Instant::now() + wait.min(MAX_RETRY_AFTER);
        let mut state = self.lock();
        let later = state
            .not_before
            .map_or(until, |existing| existing.max(until));
        state.not_before = Some(later);
        state.snapshot.throttled_until = Some(later);
    }

    pub fn snapshot(&self) -> RateSnapshot {
        self.lock().snapshot.clone()
    }

    fn not_before(&self) -> Option<Instant> {
        self.lock().not_before
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, GateState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Integer or decimal seconds. HTTP dates are ignored (Azure DevOps sends seconds).
pub fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
    let secs = header_f64(headers, H_RETRY_AFTER)?;
    (secs.is_finite() && secs >= 0.0)
        .then(|| Duration::from_secs_f64(secs.min(MAX_RETRY_AFTER.as_secs_f64())))
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name)?.to_str().ok().map(str::trim)
}

fn header_f64(headers: &HeaderMap, name: &str) -> Option<f64> {
    header_str(headers, name)?.parse().ok()
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "unix seconds fit in i64; checked finite"
)]
fn float_to_i64(value: f64) -> Option<i64> {
    value.is_finite().then_some(value as i64)
}

#[cfg(test)]
mod tests;
