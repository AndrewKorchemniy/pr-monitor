use std::sync::Arc;

use super::*;
use crate::testing::person;

fn comments(count: u32, authors: &[&str], snippet: Option<&str>) -> ChangeKind {
    ChangeKind::NewComments {
        count,
        authors: authors.iter().map(|a| Arc::from(*a)).collect(),
        snippet: snippet.map(Arc::from),
    }
}

#[test]
fn policy_templates_snapshot() {
    let rev = person(2, "Rae Reviewer");
    let cases = [
        ChangeKind::NewPr,
        ChangeKind::Reactivated,
        ChangeKind::ReadyForReview,
        ChangeKind::Closed {
            outcome: CloseOutcome::Completed,
        },
        ChangeKind::Closed {
            outcome: CloseOutcome::Abandoned,
        },
        ChangeKind::Closed {
            outcome: CloseOutcome::Unknown,
        },
        ChangeKind::Reviewed {
            headline: VoteKind::Approved,
        },
        ChangeKind::Reviewed {
            headline: VoteKind::WaitingForAuthor,
        },
        ChangeKind::NeedsReReview,
        ChangeKind::ChangesRequested {
            by: rev.clone(),
            vote: VoteKind::WaitingForAuthor,
        },
        ChangeKind::ChangesRequested {
            by: rev.clone(),
            vote: VoteKind::Rejected,
        },
        ChangeKind::NewVote {
            by: rev.clone(),
            vote: VoteKind::Approved,
        },
        ChangeKind::NewVote {
            by: rev,
            vote: VoteKind::ApprovedWithSuggestions,
        },
        comments(1, &["Rae"], Some("Typo here")),
        comments(4, &["Rae", "Sam"], Some("Typo here")),
        comments(4, &["Rae", "Sam"], None),
        ChangeKind::CommentsResolved,
        ChangeKind::NewPush,
    ];
    let rendered: Vec<String> = cases
        .iter()
        .map(|kind| {
            let h = headline(kind, 4711, 0);
            format!("{} | {}", h.title, h.detail.unwrap_or_default())
        })
        .collect();
    insta::assert_snapshot!(rendered.join("\n"));
}

#[test]
fn headline_counts_extra_changes() {
    assert_eq!(
        headline(&ChangeKind::NewPush, 1, 2).title,
        "New changes pushed · !1 (+2 more)"
    );
}

#[test]
fn summary_body_caps_titles_and_length() {
    let body = summary_body(["a", "b", "c", "d"]);
    assert_eq!(body, "a · b · c");
    assert!(summary_body([&*"x".repeat(500)]).chars().count() <= MAX_SUMMARY_CHARS);
}

/// "and N more" used to count extra comments as extra people: one author with five comments
/// read "Ann and 4 more".
#[test]
fn comments_detail_counts_people_not_comments() {
    for (authors, expected) in [
        (&["Ann"][..], "Ann"),
        (&["Ann", "Bob"][..], "Ann, Bob"),
        (&["Ann", "Bob", "Cat"][..], "Ann, Bob and others"),
    ] {
        let detail = headline(&comments(5, authors, None), 1, 0).detail;
        assert_eq!(detail.as_deref(), Some(expected), "{authors:?}");
    }
}

#[test]
fn test_body_names_the_app() {
    assert!(test_body().starts_with(APP_DISPLAY_NAME));
}

#[test]
fn auth_title_singular_plural() {
    assert_eq!(auth_title(1), "Sign-in needed");
    assert_eq!(auth_title(3), "Sign-in needed for 3 organizations");
}
