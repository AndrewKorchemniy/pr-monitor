//! Retry policy for transient failures (network, 5xx, 429).

use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Total attempts, including the first.
    pub max_attempts: u32,
    pub base: Duration,
    pub max_backoff: Duration,
    /// Wait for a 429 that carries no `Retry-After`.
    pub default_429_wait: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base: Duration::from_millis(500),
            max_backoff: Duration::from_secs(8),
            default_429_wait: Duration::from_secs(30),
        }
    }
}

/// Jitter applied to each backoff, as a fraction of the delay.
const JITTER_FRACTION: f64 = 0.2;

/// `base · 2^attempt`, ±20% jitter, capped at `max_backoff`.
pub fn backoff_delay(policy: &RetryPolicy, attempt: u32, rng: &mut fastrand::Rng) -> Duration {
    let exponential = policy.base.saturating_mul(2u32.saturating_pow(attempt));
    let capped = exponential.min(policy.max_backoff);
    let jitter = 1.0 + JITTER_FRACTION * (rng.f64() * 2.0 - 1.0);
    capped.mul_f64(jitter).min(policy.max_backoff)
}

#[cfg(test)]
mod tests;
