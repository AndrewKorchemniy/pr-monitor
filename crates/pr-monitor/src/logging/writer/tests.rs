use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

/// The day the budget-reset test pretends it is (only that test moves it).
static DAY: AtomicU64 = AtomicU64::new(1);

fn fixed_day() -> u64 {
    7
}

fn moving_day() -> u64 {
    DAY.load(Ordering::Relaxed)
}

fn capped_writer(cap: u64) -> RedactingWriter<Vec<u8>> {
    RedactingWriter {
        inner: Vec::new(),
        budget: Some(DayBudget::with_clock(cap, fixed_day)),
    }
}

#[test]
fn log_cap_writes_one_notice_then_drops_the_rest_of_the_day() {
    let mut writer = capped_writer(11);
    for event in ["12345\n", "6789\n", "too much\n", "later\n"] {
        let written = writer.write(event.as_bytes()).expect("write");
        assert_eq!(
            written,
            event.len(),
            "the layer always sees success: {event}"
        );
    }
    assert_eq!(
        String::from_utf8_lossy(&writer.inner),
        "12345\n6789\n[log capped for today]\n"
    );
}

#[test]
fn log_cap_resets_on_a_new_day() {
    let budget = DayBudget::with_clock(4, moving_day);
    let steps = [
        (1, 3, Admit::Write),
        (1, 3, Admit::Capped),
        (1, 1, Admit::Drop),
        (2, 3, Admit::Write),
    ];
    for (day, len, expected) in steps {
        DAY.store(day, Ordering::Relaxed);
        assert_eq!(budget.admit(len), expected, "day={day} len={len}");
    }
}

#[test]
fn writer_without_a_budget_redacts_and_never_caps() {
    let mut writer = RedactingWriter {
        inner: Vec::new(),
        budget: None,
    };
    let event = "Authorization: Basic c2VjcmV0OnRva2Vu\n";
    for _ in 0..3 {
        writer.write_all(event.as_bytes()).expect("write");
    }
    let text = String::from_utf8_lossy(&writer.inner);
    assert_eq!(text.lines().count(), 3, "{text}");
    assert!(!text.contains("c2VjcmV0OnRva2Vu"), "{text}");
}
