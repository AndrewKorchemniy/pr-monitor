use std::sync::Arc;

use prm_domain::{
    diff::{ChangeEvent, ChangeKind},
    snapshot::PrHeader,
    testing::{key, person, ts},
};

use super::*;

fn event(pr: i32, kind: ChangeKind) -> ChangeEvent {
    ChangeEvent {
        pr: PrHeader {
            key: key("contoso", 1, pr),
            title: Arc::from("t"),
            repo_name: Arc::from("api"),
            project_name: Arc::from("p"),
            author: person(1, "Ana"),
            web_url: Arc::from("https://dev.azure.com/contoso"),
            draft: false,
        },
        kind,
        at: ts(1_000),
    }
}

#[test]
fn the_highest_priority_event_per_pr_wins() {
    let mut log = ChangeLog::default();
    log.record(&[
        event(1, ChangeKind::NewPush),
        event(1, ChangeKind::NewPr),
        event(2, ChangeKind::CommentsResolved),
    ]);
    assert_eq!(
        log.note(&key("contoso", 1, 1)).map(|n| n.text.as_ref()),
        Some("New pull request")
    );
    assert_eq!(
        log.note(&key("contoso", 1, 2)).map(|n| n.text.as_ref()),
        Some("All comments resolved")
    );
}

#[test]
fn a_later_batch_replaces_the_note_and_forget_clears_it() {
    let mut log = ChangeLog::default();
    log.record(&[event(1, ChangeKind::NewPr)]);
    log.record(&[event(1, ChangeKind::NewPush)]);
    let pr = key("contoso", 1, 1);
    assert_eq!(
        log.note(&pr).map(|n| n.text.as_ref()),
        Some("New changes pushed")
    );
    log.forget(&pr);
    assert_eq!(log.note(&pr), None);
}
