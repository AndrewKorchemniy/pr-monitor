//! A `MakeWriter` that redacts each formatted event before it reaches the file or console, and
//! optionally stops writing once a day's byte budget is spent (one notice line, then silence).

use std::{
    io::{self, Write},
    sync::{Arc, Mutex, PoisonError},
};

use tracing_subscriber::fmt::MakeWriter;

use super::redact::redact;

/// Bytes the log file takes per UTC day before it goes quiet.
pub(super) const MAX_DAY_BYTES: u64 = 32 << 20;
const SECS_PER_DAY: u64 = 86_400;
const CAPPED_LINE: &[u8] = b"[log capped for today]\n";

/// What to do with one event.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum Admit {
    Write,
    /// The event crosses the cap: write the notice instead.
    Capped,
    Drop,
}

#[derive(Default)]
struct DayUse {
    day: u64,
    used: u64,
    capped: bool,
}

/// Bytes written today, shared by every writer the layer makes.
pub(super) struct DayBudget {
    cap: u64,
    today: fn() -> u64,
    state: Mutex<DayUse>,
}

/// Days since the Unix epoch (UTC, like the daily file rotation).
fn utc_day() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / SECS_PER_DAY)
}

impl DayBudget {
    /// `cap` bytes per UTC day.
    pub(super) fn daily(cap: u64) -> Arc<Self> {
        Self::with_clock(cap, utc_day)
    }

    pub(super) fn with_clock(cap: u64, today: fn() -> u64) -> Arc<Self> {
        Arc::new(Self {
            cap,
            today,
            state: Mutex::new(DayUse::default()),
        })
    }

    /// Accounts for `len` bytes on the current day.
    pub(super) fn admit(&self, len: usize) -> Admit {
        let day = (self.today)();
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.day != day {
            *state = DayUse {
                day,
                ..DayUse::default()
            };
        }
        if state.capped {
            return Admit::Drop;
        }
        let used = state
            .used
            .saturating_add(u64::try_from(len).unwrap_or(u64::MAX));
        if used > self.cap {
            state.capped = true;
            return Admit::Capped;
        }
        state.used = used;
        Admit::Write
    }
}

pub(super) struct RedactingMakeWriter<M> {
    inner: M,
    budget: Option<Arc<DayBudget>>,
}

impl<M> RedactingMakeWriter<M> {
    pub(super) const fn new(inner: M) -> Self {
        Self {
            inner,
            budget: None,
        }
    }

    /// Stops writing for the rest of the day once `budget` is spent.
    pub(super) fn with_budget(self, budget: Arc<DayBudget>) -> Self {
        Self {
            budget: Some(budget),
            ..self
        }
    }
}

pub(super) struct RedactingWriter<W> {
    inner: W,
    budget: Option<Arc<DayBudget>>,
}

impl<W: Write> Write for RedactingWriter<W> {
    /// The fmt layer writes one whole event per call, so secrets never straddle two writes.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let admit = self
            .budget
            .as_ref()
            .map_or(Admit::Write, |b| b.admit(buf.len()));
        match admit {
            Admit::Write => {
                let text = String::from_utf8_lossy(buf);
                self.inner.write_all(redact(&text).as_bytes())?;
            }
            Admit::Capped => self.inner.write_all(CAPPED_LINE)?,
            Admit::Drop => {}
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<'a, M: MakeWriter<'a>> MakeWriter<'a> for RedactingMakeWriter<M> {
    type Writer = RedactingWriter<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        RedactingWriter {
            inner: self.inner.make_writer(),
            budget: self.budget.clone(),
        }
    }
}

#[cfg(test)]
mod tests;
