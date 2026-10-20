//! Every clamp bound and default for numeric settings, in one place. The toast limit is a
//! notification-policy rule, so it lives in `prm_domain::notify` and is re-exported here.

use std::ops::RangeInclusive;

pub use prm_domain::notify::DEFAULT_MAX_TOASTS as DEFAULT_MAX_PER_REFRESH;

pub const INTERVAL_MINUTES: RangeInclusive<u32> = 1..=60;
pub const DEFAULT_INTERVAL_MINUTES: u32 = 5;

pub const JITTER_PERCENT: RangeInclusive<u8> = 0..=25;
pub const DEFAULT_JITTER_PERCENT: u8 = 10;

pub const STARTUP_DELAY_SECS: RangeInclusive<u32> = 0..=120;
pub const DEFAULT_STARTUP_DELAY_SECS: u32 = 0;

/// Toasts per refresh.
pub const MAX_PER_REFRESH: RangeInclusive<u8> = prm_domain::notify::MAX_TOASTS_RANGE;

pub const CONNECT_TIMEOUT_SECS: RangeInclusive<u32> = 2..=60;
pub const DEFAULT_CONNECT_TIMEOUT_SECS: u32 = 10;

pub const REQUEST_TIMEOUT_SECS: RangeInclusive<u32> = 5..=120;
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u32 = 30;

pub const MAX_CONCURRENCY: RangeInclusive<u8> = 1..=8;
pub const DEFAULT_MAX_CONCURRENCY: u8 = 6;

pub const FONT_SCALE: RangeInclusive<f32> = 0.75..=1.5;
pub const DEFAULT_FONT_SCALE: f32 = 1.0;

/// People remembered per org for the instant picker.
pub const MAX_PERSISTED_PEOPLE: usize = 2_000;
