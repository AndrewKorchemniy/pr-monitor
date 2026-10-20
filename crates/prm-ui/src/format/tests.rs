use std::time::Duration;

use prm_domain::testing::ts;
use time::UtcOffset;

use super::*;

#[test]
fn plural_uses_singular_only_for_one() {
    assert_eq!(plural(1, "thread", "threads"), "1 thread");
    assert_eq!(plural(0, "thread", "threads"), "0 threads");
    assert_eq!(plural(3, "thread", "threads"), "3 threads");
}

#[test]
fn clock_and_date_time_use_the_offset() {
    // 2026-10-19 12:05:00 UTC
    let at = ts(1_792_411_500);
    let plus_two = UtcOffset::from_hms(2, 0, 0).expect("valid offset");
    assert_eq!(clock(at, UtcOffset::UTC), "12:05");
    assert_eq!(clock(at, plus_two), "14:05");
    assert_eq!(date_time(at, plus_two), "2026-10-19 14:05");
}

#[test]
fn countdown_rounds_minutes_up_and_shows_seconds_only_in_the_last_minute() {
    assert!(matches!(
        countdown(Duration::from_secs(61), true),
        Countdown::Minutes(2)
    ));
    assert!(matches!(
        countdown(Duration::from_secs(59), true),
        Countdown::Seconds(59)
    ));
    assert!(matches!(
        countdown(Duration::from_secs(59), false),
        Countdown::Minutes(1)
    ));
    assert!(matches!(
        countdown(Duration::ZERO, false),
        Countdown::Minutes(0)
    ));
}

#[test]
fn markdown_link_escapes_brackets_and_closing_parens() {
    assert_eq!(
        markdown_link("!7 Fix [x]", "https://a/b(1)"),
        r"[!7 Fix \[x\]](https://a/b(1%29)"
    );
    assert_eq!(markdown_link(r"a\b", "u"), r"[a\\b](u)");
}
