//! Every timing and sizing constant of the service, in one place.

use std::time::Duration;

/// A refresh that runs longer than this is cancelled and reported as a timeout.
pub const REFRESH_DEADLINE: Duration = Duration::from_secs(120);
/// Concurrent PR-list queries per org.
pub const LIST_CONCURRENCY: usize = 4;
/// Concurrent thread fetches per org.
pub const THREAD_CONCURRENCY: usize = 6;
/// Vanished PRs looked up by id per refresh; the rest are carried over as stale.
pub const VANISHED_RESOLVE_CAP: usize = 25;
/// Projects and repositories are refetched after this long.
pub const CATALOG_TTL: Duration = Duration::from_hours(1);
/// An org paused for sign-in is re-checked silently this often (at least the refresh interval).
pub const AUTH_PROBE_INTERVAL: Duration = Duration::from_mins(15);
/// Showing the window re-checks paused orgs, but not more often than this.
pub const PROBE_ON_SHOW_DEBOUNCE: Duration = Duration::from_secs(60);
/// The actor wakes at least this often, so suspend/resume is noticed.
pub const SCHEDULER_MAX_SLEEP: Duration = Duration::from_secs(60);
/// Wall time running ahead of monotonic time by more than this means the machine slept.
pub const RESUME_GAP: Duration = Duration::from_secs(90);
/// After a resume, overdue orgs refresh after this settle time (network comes back first).
pub const RESUME_SETTLE: Duration = Duration::from_secs(5);
/// Orgs due within this window refresh together, as one notification cycle.
pub const COALESCE_WINDOW: Duration = Duration::from_secs(5);
/// A manual refresh within this long of the previous one is ignored.
pub const MANUAL_REFRESH_DEBOUNCE: Duration = Duration::from_secs(2);
/// Progress-only state changes are published at most this often (10 Hz).
pub const PUBLISH_THROTTLE: Duration = Duration::from_millis(100);
/// Seen marks and UI state are written after this quiet period.
pub const PERSIST_DEBOUNCE: Duration = Duration::from_secs(2);
/// At shutdown, a running state save gets this long to finish before the final save.
pub const SHUTDOWN_SAVE_WAIT: Duration = Duration::from_secs(2);
/// First retry delay after a failed refresh; doubles per failure, capped by the interval.
pub const FAILURE_BACKOFF_BASE: Duration = Duration::from_secs(60);
/// Rate-limit slowdown never stretches the interval beyond this factor.
pub const MAX_RATE_MULTIPLIER: u32 = 8;
/// Rate-limit headroom below this doubles the interval.
pub const HEADROOM_LOW: f64 = 0.25;
/// Rate-limit headroom at or above this halves the slowdown again.
pub const HEADROOM_OK: f64 = 0.5;
/// Delay assumed when a 429 carries no `Retry-After`.
pub const DEFAULT_RETRY_AFTER: Duration = Duration::from_secs(30);
/// People kept in memory per org for the local picker.
pub const PEOPLE_CACHE_CAP: usize = 5_000;
/// People written to `state.json` per org.
pub const PEOPLE_PERSIST_CAP: usize = prm_config::limits::MAX_PERSISTED_PEOPLE;
/// Directory search starts at this many characters.
pub const PEOPLE_REMOTE_MIN_CHARS: usize = 3;
/// Directory search waits this long for more keystrokes.
pub const SEARCH_DEBOUNCE: Duration = Duration::from_millis(250);
/// Most candidates returned per search phase.
pub const SEARCH_LIMIT: usize = 20;
/// The same notice for the same org is shown at most once in this window.
pub const NOTICE_DEDUPE: Duration = Duration::from_secs(30);
/// How long an in-app notice stays visible.
pub const NOTICE_TTL: Duration = Duration::from_secs(8);
/// The org-wide query probe result is trusted this long.
pub const ORG_WIDE_PROBE_TTL: Duration = Duration::from_hours(24);
/// A discovered tenant is trusted this long.
pub const TENANT_TTL: Duration = Duration::from_hours(7 * 24);
/// A PAT expiring within this many days raises a banner.
pub const PAT_EXPIRY_WARNING_DAYS: i64 = 7;
/// Blocking-pool threads on the service runtime (toasts, registry, file writes).
pub const MAX_BLOCKING_THREADS: usize = 4;
