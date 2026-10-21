use prm_domain::testing::ts;

use super::*;

#[test]
fn notice_stack_info_is_a_short_confirmation() {
    let mut stack = NoticeStack::default();
    stack.info("Link copied", ts(100));
    let [shown] = stack.items() else {
        panic!("one notice: {:?}", stack.items());
    };
    assert_eq!(shown.severity, Severity::Info);
    assert_eq!(shown.expires, ts(100).saturating_add(INFO_TTL));
}

/// A failed save used to look like "Link copied" (info tone, 4 s) and was easy to miss.
#[test]
fn notice_stack_error_outlasts_and_outranks_info() {
    let mut stack = NoticeStack::default();
    stack.error("Changes couldn't be saved", ts(100));
    let [shown] = stack.items() else {
        panic!("one notice: {:?}", stack.items());
    };
    assert_eq!(shown.severity, Severity::Error);
    assert_eq!(shown.expires, ts(100).saturating_add(ERROR_TTL));
    assert!(ERROR_TTL > INFO_TTL);
}

#[test]
fn notice_stack_keeps_the_newest_three_and_prunes_expired() {
    let mut stack = NoticeStack::default();
    for text in ["one", "two", "three", "four"] {
        stack.info(text, ts(100));
    }
    let texts: Vec<&str> = stack.items().iter().map(|n| n.text.as_ref()).collect();
    assert_eq!(texts, ["two", "three", "four"]);
    stack.prune(ts(100).saturating_add(INFO_TTL));
    assert_eq!(stack.items(), []);
}
