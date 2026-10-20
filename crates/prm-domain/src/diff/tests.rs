use std::collections::BTreeMap;

use super::*;
use crate::{
    classify::ClassifyConfig,
    model::{PrRecord, ThreadStatus},
    snapshot::classify_record,
    testing::{PrBuilder, ThreadBuilder, key, person, record, ts, vsid},
    tombstone::Tombstone,
};

const REVIEWER: u128 = 2;
const AUTHOR: u128 = 1;
const ME: u128 = 9;
const BOT: u128 = 7;

fn config() -> ClassifyConfig {
    ClassifyConfig {
        reviewers: [vsid(REVIEWER), vsid(ME)].into_iter().collect(),
        ..ClassifyConfig::default()
    }
}

fn pr(number: i32) -> PrBuilder {
    PrBuilder::new(key("contoso", 1, number)).author(person(AUTHOR, "Author"))
}

fn snapshot(records: &[PrRecord]) -> OrgSnapshot {
    records
        .iter()
        .map(|rec| (rec.pr.key.clone(), classify_record(rec, &config(), false)))
        .collect()
}

struct Case {
    prev: Option<OrgSnapshot>,
    next: OrgSnapshot,
    closed: Vec<(PrKey, CloseOutcome)>,
    tombstones: Tombstones,
    suppress_new: bool,
}

impl Case {
    fn new(prev: &[PrRecord], next: &[PrRecord]) -> Self {
        Self {
            prev: Some(snapshot(prev)),
            next: snapshot(next),
            closed: Vec::new(),
            tombstones: Tombstones::new(),
            suppress_new: false,
        }
    }

    fn run(&self) -> Vec<ChangeEvent> {
        let ignored = [vsid(BOT)].into_iter().collect();
        diff(&DiffInput {
            prev: self.prev.as_ref(),
            next: &self.next,
            closed: &self.closed,
            tombstones: &self.tombstones,
            me: Some(vsid(ME)),
            ignored: &ignored,
            suppress_new: self.suppress_new,
            now: ts(1_000),
        })
    }

    fn kinds(&self) -> Vec<ChangeKind> {
        self.run().into_iter().map(|e| e.kind).collect()
    }
}

fn thread(id: i32, commenters: &[(u128, &str)]) -> crate::model::ThreadDigest {
    commenters
        .iter()
        .fold(ThreadBuilder::human(id), |t, (n, name)| {
            t.comment(person(*n, name))
        })
        .build()
}

#[test]
fn diff_no_prev_emits_nothing() {
    let mut case = Case::new(&[], &[record(pr(1).build(), vec![])]);
    case.prev = None;
    assert_no_events(case.run());
}

#[test]
fn diff_new_pr() {
    let case = Case::new(&[], &[record(pr(1).build(), vec![])]);
    assert_eq!(case.kinds(), vec![ChangeKind::NewPr]);
}

#[test]
fn diff_new_pr_suppressed_on_rebaseline() {
    let mut case = Case::new(&[], &[record(pr(1).build(), vec![])]);
    case.suppress_new = true;
    assert_no_events(case.run());
}

#[test]
fn diff_reactivated_via_tombstone() {
    let rec = record(pr(1).build(), vec![]);
    let mut case = Case::new(&[], std::slice::from_ref(&rec));
    case.tombstones.insert(
        rec.pr.key.clone(),
        Tombstone {
            closed_at: ts(500),
            outcome: CloseOutcome::Abandoned,
            header: PrHeader::from_record(&rec),
        },
    );
    assert_eq!(case.kinds(), vec![ChangeKind::Reactivated]);
}

#[test]
fn diff_closed_completed_and_abandoned() {
    for outcome in [CloseOutcome::Completed, CloseOutcome::Abandoned] {
        let rec = record(pr(1).build(), vec![]);
        let mut case = Case::new(std::slice::from_ref(&rec), &[]);
        case.closed = vec![(rec.pr.key.clone(), outcome)];
        assert_eq!(
            case.kinds(),
            vec![ChangeKind::Closed { outcome }],
            "{outcome:?}"
        );
    }
}

#[test]
fn diff_vanished_without_resolution_silent() {
    let case = Case::new(&[record(pr(1).build(), vec![])], &[]);
    assert_no_events(case.run());
}

#[test]
fn diff_stale_pr_emits_nothing() {
    let mut case = Case::new(&[], &[record(pr(1).build(), vec![])]);
    for classified in case.next.values_mut() {
        classified.stale = true;
    }
    assert_no_events(case.run());
}

#[test]
fn diff_new_comments_excludes_author_me_ignored_system() {
    let before = record(pr(1).build(), vec![]);
    let after = record(
        pr(1).build(),
        vec![
            thread(1, &[(AUTHOR, "Author")]),
            thread(2, &[(ME, "Me")]),
            thread(3, &[(BOT, "Bot")]),
            ThreadBuilder::human(4)
                .comment(person(3, "R"))
                .system("VoteUpdate")
                .build(),
        ],
    );
    assert_no_events(Case::new(&[before], &[after]).run());
}

#[test]
fn diff_reply_in_existing_thread_counts() {
    let before = record(pr(1).build(), vec![thread(1, &[(AUTHOR, "Author")])]);
    let after = record(
        pr(1).build(),
        vec![thread(1, &[(AUTHOR, "Author"), (3, "Rae")])],
    );
    let kinds = Case::new(&[before], &[after]).kinds();
    let [ChangeKind::NewComments { count, authors, .. }] = kinds.as_slice() else {
        panic!("expected one NewComments, got {kinds:?}");
    };
    assert_eq!(*count, 1);
    assert_eq!(authors.as_ref(), [Arc::<str>::from("Rae")]);
}

#[test]
fn diff_deleted_comment_not_counted() {
    let before = record(pr(1).build(), vec![thread(1, &[(3, "Rae")])]);
    let after = record(
        pr(1).build(),
        vec![
            ThreadBuilder::human(1)
                .comment(person(3, "Rae"))
                .deleted_comment(person(4, "Sam"))
                .build(),
        ],
    );
    assert_no_events(Case::new(&[before], &[after]).run());
}

#[test]
fn diff_first_review_reviewed() {
    let before = record(pr(1).build(), vec![]);
    let after = record(pr(1).reviewer(person(REVIEWER, "Rev"), 10).build(), vec![]);
    let kinds = Case::new(&[before], &[after]).kinds();
    assert_eq!(kinds.len(), 2);
    assert_eq!(
        kinds[0],
        ChangeKind::Reviewed {
            headline: VoteKind::Approved
        }
    );
    assert!(matches!(
        kinds[1],
        ChangeKind::NewVote {
            vote: VoteKind::Approved,
            ..
        }
    ));
}

#[test]
fn diff_vote_reset_needs_re_review() {
    let before = record(pr(1).reviewer(person(REVIEWER, "Rev"), 10).build(), vec![]);
    let after = record(pr(1).reviewer(person(REVIEWER, "Rev"), 0).build(), vec![]);
    assert_eq!(
        Case::new(&[before], &[after]).kinds(),
        vec![ChangeKind::NeedsReReview]
    );
}

#[test]
fn diff_changes_requested_by_listed() {
    let before = record(pr(1).reviewer(person(REVIEWER, "Rev"), 10).build(), vec![]);
    let after = record(pr(1).reviewer(person(REVIEWER, "Rev"), -5).build(), vec![]);
    let kinds = Case::new(&[before], &[after]).kinds();
    let [ChangeKind::ChangesRequested { by, vote }] = kinds.as_slice() else {
        panic!("expected ChangesRequested, got {kinds:?}");
    };
    assert_eq!(&*by.name, "Rev");
    assert_eq!(*vote, VoteKind::WaitingForAuthor);
}

#[test]
fn diff_unlisted_vote_ignored() {
    let before = record(pr(1).build(), vec![]);
    let after = record(pr(1).reviewer(person(5, "Other"), 10).build(), vec![]);
    assert_no_events(Case::new(&[before], &[after]).run());
}

#[test]
fn diff_author_self_vote_ignored() {
    let before = record(pr(1).build(), vec![]);
    let after = record(pr(1).reviewer(person(AUTHOR, "Author"), 10).build(), vec![]);
    assert_no_events(Case::new(&[before], &[after]).run());
}

/// The user's own "Wait for author" vote used to raise a top-priority toast about themselves.
#[test]
fn diff_my_own_vote_raises_no_vote_event() {
    let before = record(pr(1).build(), vec![]);
    let after = record(pr(1).reviewer(person(ME, "Me"), -5).build(), vec![]);
    assert_eq!(
        Case::new(&[before], &[after]).kinds(),
        vec![ChangeKind::Reviewed {
            headline: VoteKind::WaitingForAuthor
        }]
    );
}

/// The toast quotes the newest comment next to `authors[0]`; with several commenters the
/// quote used to be credited to whoever commented first in thread order.
#[test]
fn diff_new_comments_newest_author_leads() {
    let before = record(pr(1).build(), vec![]);
    let after = record(
        pr(1).build(),
        vec![
            thread(1, &[(3, "Ann")]),
            thread(5, &[(AUTHOR, "Author"), (4, "Bob")]),
        ],
    );
    let kinds = Case::new(&[before], &[after]).kinds();
    let [
        ChangeKind::NewComments {
            authors, snippet, ..
        },
    ] = kinds.as_slice()
    else {
        panic!("expected one NewComments, got {kinds:?}");
    };
    assert_eq!(snippet.as_deref(), Some("comment 2"));
    assert_eq!(
        authors.as_ref(),
        [Arc::<str>::from("Bob"), Arc::<str>::from("Ann")]
    );
}

#[test]
fn diff_comments_resolved() {
    let open = ThreadBuilder::human(1).comment(person(3, "Rae")).build();
    let fixed = ThreadBuilder::human(1)
        .status(ThreadStatus::Fixed)
        .comment(person(3, "Rae"))
        .build();
    let kinds = Case::new(
        &[record(pr(1).build(), vec![open])],
        &[record(pr(1).build(), vec![fixed])],
    )
    .kinds();
    assert_eq!(kinds, vec![ChangeKind::CommentsResolved]);
}

#[test]
fn diff_ready_for_review() {
    let before = record(pr(1).draft().build(), vec![]);
    let after = record(pr(1).build(), vec![]);
    assert_eq!(
        Case::new(&[before], &[after]).kinds(),
        vec![ChangeKind::ReadyForReview]
    );
}

#[test]
fn diff_title_change_no_event() {
    let before = record(pr(1).title("Old").build(), vec![]);
    let after = record(pr(1).title("New").build(), vec![]);
    assert_no_events(Case::new(&[before], &[after]).run());
}

#[test]
fn diff_new_push() {
    let before = record(pr(1).head("aaa").build(), vec![]);
    let after = record(pr(1).head("bbb").build(), vec![]);
    assert_eq!(
        Case::new(&[before], &[after]).kinds(),
        vec![ChangeKind::NewPush]
    );
}

#[test]
fn diff_missing_head_commit_is_not_a_push() {
    let before = record(pr(1).head("aaa").build(), vec![]);
    let after = record(pr(1).build(), vec![]);
    assert_no_events(Case::new(&[before], &[after]).run());
}

#[test]
fn diff_events_sorted_by_key_then_kind() {
    let before = [record(pr(2).draft().build(), vec![])];
    let after = [
        record(pr(2).head("x").build(), vec![thread(1, &[(3, "Rae")])]),
        record(pr(1).build(), vec![]),
    ];
    let events = Case::new(&before, &after).run();
    let order: Vec<(i32, u8)> = events
        .iter()
        .map(|e| (e.pr.key.number.0, e.kind.sort_order()))
        .collect();
    let mut sorted = order.clone();
    sorted.sort_unstable();
    assert_eq!(order, sorted);
    assert_eq!(events.len(), 3);
}

#[test]
fn diff_complex_scenario_snapshot() {
    let before = [
        record(pr(1).build(), vec![]),
        record(
            pr(2)
                .reviewer(person(REVIEWER, "Rev"), 10)
                .head("a")
                .build(),
            vec![thread(1, &[(3, "Rae")])],
        ),
        record(pr(3).build(), vec![]),
    ];
    let after = [
        record(pr(1).reviewer(person(REVIEWER, "Rev"), -10).build(), vec![]),
        record(
            pr(2).reviewer(person(REVIEWER, "Rev"), 0).head("b").build(),
            vec![thread(1, &[(3, "Rae"), (4, "Sam")])],
        ),
        record(pr(4).build(), vec![]),
    ];
    let mut case = Case::new(&before, &after);
    case.closed = vec![(key("contoso", 1, 3), CloseOutcome::Completed)];
    let summary: BTreeMap<String, Vec<ChangeKind>> =
        case.run().into_iter().fold(BTreeMap::new(), |mut acc, e| {
            acc.entry(e.pr.key.to_string()).or_default().push(e.kind);
            acc
        });
    insta::assert_json_snapshot!(summary);
}

fn assert_no_events(events: Vec<ChangeEvent>) {
    assert_eq!(events, Vec::<ChangeEvent>::new());
}
