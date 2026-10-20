//! The production wall clock.

use prm_domain::{Timestamp, ports::Clock};
use time::{OffsetDateTime, UtcOffset};

/// UTC now, plus the local offset for display. The offset is captured at startup because
/// reading it later can fail once other threads exist (the `time` crate's soundness rule).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SystemClock {
    startup_offset: UtcOffset,
}

impl SystemClock {
    /// Call on the main thread before spawning other threads.
    pub fn new() -> Self {
        Self {
            startup_offset: UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_offset(OffsetDateTime::now_utc())
    }

    fn local_offset(&self) -> UtcOffset {
        UtcOffset::current_local_offset().unwrap_or(self.startup_offset)
    }
}
