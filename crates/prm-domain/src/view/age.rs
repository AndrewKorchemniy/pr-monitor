//! Relative ages ("5m", "2d") for the dense table, recomputed at most once per minute.

use std::{sync::Arc, time::Duration};

use crate::{time::Timestamp, view::row::PrRow};

const MINUTE: u64 = 60;
const MINUTE_I64: i64 = 60;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;
const WEEK: u64 = 7 * DAY;
const MONTH: u64 = 30 * DAY;
const YEAR: u64 = 365 * DAY;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AgeBucket {
    /// Under an hour.
    Fresh,
    /// Under a day.
    Today,
    /// Under a week.
    Week,
    Older,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgeLabel {
    pub bucket: AgeBucket,
    /// "now", "5m", "3h", "2d", "3w", "4mo", "1y".
    pub text: Arc<str>,
}

pub fn age_label(now: Timestamp, then: Timestamp) -> AgeLabel {
    let secs = now.since(then).as_secs();
    let bucket = match secs {
        s if s < HOUR => AgeBucket::Fresh,
        s if s < DAY => AgeBucket::Today,
        s if s < WEEK => AgeBucket::Week,
        _ => AgeBucket::Older,
    };
    let text = match secs {
        s if s < MINUTE => "now".to_owned(),
        s if s < HOUR => format!("{}m", s / MINUTE),
        s if s < DAY => format!("{}h", s / HOUR),
        s if s < WEEK => format!("{}d", s / DAY),
        s if s < MONTH => format!("{}w", s / WEEK),
        s if s < YEAR => format!("{}mo", s / MONTH),
        s => format!("{}y", s / YEAR),
    };
    AgeLabel {
        bucket,
        text: text.into(),
    }
}

/// Age labels for the current rows, refreshed once per minute or when rows change, so the
/// UI never formats ages per frame.
#[derive(Clone, Debug, Default)]
pub struct AgeCache {
    minute: i64,
    generation: u64,
    labels: Vec<AgeLabel>,
}

impl AgeCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true when labels were recomputed.
    pub fn update(&mut self, rows: &[PrRow], rows_generation: u64, now: Timestamp) -> bool {
        let minute = now.unix().div_euclid(MINUTE_I64);
        let fresh = self.labels.len() == rows.len()
            && self.generation == rows_generation
            && self.minute == minute;
        if fresh {
            return false;
        }
        self.labels = rows.iter().map(|r| age_label(now, r.created_at)).collect();
        self.generation = rows_generation;
        self.minute = minute;
        true
    }

    pub fn get(&self, row_index: usize) -> Option<&AgeLabel> {
        self.labels.get(row_index)
    }
}

/// How long until an age label could change, for scheduling a repaint.
pub const AGE_TICK: Duration = Duration::from_secs(MINUTE);
