//! Small formatting rules: plurals, clock times, countdowns and Markdown links.

use std::time::Duration;

use prm_domain::Timestamp;
use time::UtcOffset;

/// "1 thread" / "3 threads".
pub(crate) fn plural(n: u32, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// "14:05" in the given offset.
pub(crate) fn clock(at: Timestamp, offset: UtcOffset) -> String {
    let local = at.inner().to_offset(offset);
    format!("{:02}:{:02}", local.hour(), local.minute())
}

/// "2026-10-19 14:05" in the given offset (tooltips).
pub(crate) fn date_time(at: Timestamp, offset: UtcOffset) -> String {
    let local = at.inner().to_offset(offset);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        local.year(),
        u8::from(local.month()),
        local.day(),
        local.hour(),
        local.minute()
    )
}

/// The countdown granularity: seconds only in the last minute.
pub(crate) enum Countdown {
    Minutes(u64),
    Seconds(u64),
}

/// Minutes are rounded up, so "Next in 1m" never shows for a refresh 61 s away.
pub(crate) const fn countdown(remaining: Duration, show_seconds: bool) -> Countdown {
    let secs = remaining.as_secs();
    if show_seconds && secs < 60 {
        Countdown::Seconds(secs)
    } else {
        Countdown::Minutes(secs.div_ceil(60))
    }
}

/// `[title](url)` with Markdown-significant brackets in the title escaped.
pub(crate) fn markdown_link(title: &str, url: &str) -> String {
    let mut escaped = String::with_capacity(title.len() + 8);
    for c in title.chars() {
        if matches!(c, '[' | ']' | '\\') {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    let url = url.replace(')', "%29");
    format!("[{escaped}]({url})")
}

#[cfg(test)]
mod tests;
