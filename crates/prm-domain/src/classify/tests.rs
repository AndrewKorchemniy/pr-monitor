use super::*;
use crate::testing::{PrBuilder, ThreadBuilder, key, person, vsid};

fn cfg(reviewers: &[u128]) -> ClassifyConfig {
    ClassifyConfig {
        reviewers: reviewers.iter().map(|n| vsid(*n)).collect(),
        ..ClassifyConfig::default()
    }
}

fn pr() -> PrBuilder {
    PrBuilder::new(key("contoso", 1, 1)).author(person(1, "Author"))
}

fn statuses(threads: &[ThreadDigest]) -> (u32, u32) {
    let c = classify(&pr().build(), threads, &cfg(&[2]));
    (c.active_threads, c.total_threads)
}

#[test]
fn classify_system_thread_with_property_excluded() {
    let t = ThreadBuilder::human(1)
        .comment(person(2, "R"))
        .system("VoteUpdate")
        .build();
    assert_eq!(statuses(&[t]), (0, 0));
}

#[test]
fn classify_system_comment_type_without_property_excluded() {
    let t = ThreadBuilder::human(1)
        .comment_kind(person(2, "R"), CommentKind::System)
        .build();
    assert_eq!(statuses(&[t]), (0, 0));
}

#[test]
fn classify_pending_counts_as_active() {
    let t = ThreadBuilder::human(1)
        .status(ThreadStatus::Pending)
        .comment(person(2, "R"))
        .build();
    assert_eq!(statuses(&[t]), (1, 1));
}

#[test]
fn classify_resolved_statuses_not_active() {
    for s in [
        ThreadStatus::Fixed,
        ThreadStatus::WontFix,
        ThreadStatus::Closed,
        ThreadStatus::ByDesign,
    ] {
        let t = ThreadBuilder::human(1)
            .status(s)
            .comment(person(2, "R"))
            .build();
        assert_eq!(statuses(&[t]), (0, 1), "{s:?}");
    }
}

#[test]
fn classify_missing_status_counts_total_not_active() {
    for s in [ThreadStatus::Missing, ThreadStatus::Unknown] {
        let t = ThreadBuilder::human(1)
            .status(s)
            .comment(person(2, "R"))
            .build();
        assert_eq!(statuses(&[t]), (0, 1));
    }
}

#[test]
fn classify_deleted_thread_excluded() {
    let t = ThreadBuilder::human(1)
        .comment(person(2, "R"))
        .deleted()
        .build();
    assert_eq!(statuses(&[t]), (0, 0));
}

#[test]
fn classify_thread_with_only_deleted_comments_excluded() {
    let t = ThreadBuilder::human(1)
        .deleted_comment(person(2, "R"))
        .build();
    assert_eq!(statuses(&[t]), (0, 0));
}

#[test]
fn classify_first_live_comment_decides_kind() {
    let human_first = ThreadBuilder::human(1)
        .deleted_comment(person(9, "Bot"))
        .comment(person(2, "R"))
        .build();
    assert_eq!(statuses(&[human_first]), (1, 1));
}

#[test]
fn classify_ignored_author_thread_excluded() {
    let t = ThreadBuilder::human(1)
        .comment(person(7, "Copilot"))
        .build();
    let mut c = cfg(&[2]);
    c.ignored_comment_authors.insert(vsid(7));
    let r = classify(&pr().build(), &[t], &c);
    assert_eq!((r.active_threads, r.total_threads), (0, 0));
}

#[test]
fn classify_unknown_comment_type_counts_human() {
    let t = ThreadBuilder::human(1)
        .comment_kind(person(2, "R"), CommentKind::Unknown)
        .build();
    assert_eq!(statuses(&[t]), (1, 1));
}

#[test]
fn classify_author_self_vote_ignored() {
    let p = pr().reviewer(person(1, "Author"), 10).build();
    let c = classify(&p, &[], &cfg(&[1]));
    assert_eq!(c.status, PrStatus::NeedsReview);
}

#[test]
fn classify_declined_reviewer_ignored() {
    let p = pr().declined(person(2, "R"), 10).build();
    assert_eq!(classify(&p, &[], &cfg(&[2])).status, PrStatus::NeedsReview);
}

#[test]
fn classify_vote_15_not_review() {
    let p = pr().reviewer(person(2, "R"), 15).build();
    assert_eq!(classify(&p, &[], &cfg(&[2])).status, PrStatus::NeedsReview);
    let p = pr().reviewer(person(2, "R"), 0).build();
    assert_eq!(classify(&p, &[], &cfg(&[2])).status, PrStatus::NeedsReview);
}

#[test]
fn classify_review_votes_10_5_m5_m10() {
    let table = [
        (10, VoteKind::Approved),
        (5, VoteKind::ApprovedWithSuggestions),
        (-5, VoteKind::WaitingForAuthor),
        (-10, VoteKind::Rejected),
    ];
    for (v, headline) in table {
        let p = pr().reviewer(person(2, "R"), v).build();
        let c = classify(&p, &[], &cfg(&[2]));
        assert_eq!(c.status, PrStatus::Reviewed, "{v}");
        assert_eq!(c.review, ReviewState::Reviewed { headline }, "{v}");
        assert_eq!(c.counted_votes.get(&vsid(2)), Some(&Vote(v)));
    }
}

#[test]
fn classify_unlisted_reviewer_ignored() {
    let p = pr().reviewer(person(3, "Other"), 10).build();
    assert_eq!(classify(&p, &[], &cfg(&[2])).status, PrStatus::NeedsReview);
}

#[test]
fn classify_group_vsid_in_list_counts_rolled_up_vote() {
    let p = pr()
        .group_reviewer(person(50, "[P]\\Team"), 10)
        .reviewer_for(person(3, "Member"), 10, &[50])
        .build();
    let c = classify(&p, &[], &cfg(&[50]));
    assert_eq!(c.status, PrStatus::Reviewed);
    assert_eq!(c.counted_votes.len(), 1);
}

#[test]
fn classify_group_vote_only_from_author_ignored() {
    let p = pr()
        .group_reviewer(person(50, "[P]\\Team"), 10)
        .reviewer_for(person(1, "Author"), 10, &[50])
        .build();
    assert_eq!(classify(&p, &[], &cfg(&[50])).status, PrStatus::NeedsReview);
}

#[test]
fn classify_listed_individual_with_voted_for_counts() {
    let p = pr()
        .group_reviewer(person(50, "[P]\\Team"), 10)
        .reviewer_for(person(2, "R"), 10, &[50])
        .build();
    let c = classify(&p, &[], &cfg(&[2]));
    assert_eq!(c.status, PrStatus::Reviewed);
    assert_eq!(
        c.counted_votes.keys().copied().collect::<Vec<_>>(),
        vec![vsid(2)]
    );
}

#[test]
fn classify_empty_list_any_non_author_vote() {
    let p = pr()
        .reviewer(person(1, "Author"), 10)
        .group_reviewer(person(50, "Team"), 10)
        .build();
    assert_eq!(classify(&p, &[], &cfg(&[])).status, PrStatus::NeedsReview);
    let p = pr().reviewer(person(3, "Anyone"), -5).build();
    assert_eq!(classify(&p, &[], &cfg(&[])).status, PrStatus::Reviewed);
}

#[test]
fn classify_empty_list_nobody() {
    let p = pr().reviewer(person(3, "Anyone"), 10).build();
    let c = ClassifyConfig {
        empty_list: EmptyReviewerPolicy::Nobody,
        ..ClassifyConfig::default()
    };
    assert_eq!(classify(&p, &[], &c).status, PrStatus::NeedsReview);
}

#[test]
fn classify_headline_negative_wins() {
    let p = pr()
        .reviewer(person(2, "A"), 10)
        .reviewer(person(3, "B"), -5)
        .build();
    let c = classify(&p, &[], &cfg(&[2, 3]));
    assert_eq!(
        c.review,
        ReviewState::Reviewed {
            headline: VoteKind::WaitingForAuthor
        }
    );
    assert_eq!(
        c.tally,
        VoteTally {
            approved: 1,
            suggestions: 0,
            waiting: 1,
            rejected: 0
        }
    );
    let p = pr()
        .reviewer(person(2, "A"), -10)
        .reviewer(person(3, "B"), -5)
        .build();
    let c = classify(&p, &[], &cfg(&[2, 3]));
    assert_eq!(
        c.review,
        ReviewState::Reviewed {
            headline: VoteKind::Rejected
        }
    );
}

#[test]
fn classify_active_comments_precedes_reviewed() {
    let p = pr().reviewer(person(2, "R"), 10).build();
    let t = ThreadBuilder::human(1).comment(person(2, "R")).build();
    let c = classify(&p, &[t], &cfg(&[2]));
    assert_eq!(c.status, PrStatus::ActiveComments);
    assert!(c.review.is_reviewed());
}

#[test]
fn classify_comments_column_counts_threads_not_comments() {
    let t1 = ThreadBuilder::human(1)
        .comment(person(2, "R"))
        .comment(person(1, "Author"))
        .comment(person(2, "R"))
        .build();
    let t2 = ThreadBuilder::human(2)
        .status(ThreadStatus::Fixed)
        .comment(person(2, "R"))
        .build();
    let c = classify(&pr().build(), &[t1, t2], &cfg(&[2]));
    assert_eq!((c.active_threads, c.total_threads), (1, 2));
    assert_eq!(c.comments.len(), 4);
}

#[test]
fn classify_draft_passthrough() {
    let c = classify(&pr().draft().build(), &[], &cfg(&[2]));
    assert_eq!(c.status, PrStatus::NeedsReview);
}
