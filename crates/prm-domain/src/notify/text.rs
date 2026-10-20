//! Every user-visible toast string, in one place (English only for v1).

use crate::{
    diff::ChangeKind, model::VoteKind, tombstone::CloseOutcome, view::text::truncate_chars,
};

/// Names in toast titles are cut to this many characters.
pub const MAX_NAME_CHARS: usize = 32;
/// Summary toast bodies are cut to this many characters.
pub const MAX_SUMMARY_CHARS: usize = 200;
/// How many PR titles a summary toast lists.
pub const SUMMARY_TITLES: usize = 3;

pub const ACTION_OPEN_PR: &str = "Open PR";
pub const ACTION_SHOW: &str = "Show";

pub const TEST_TITLE: &str = "Notifications are working";
pub const TEST_BODY: &str = "PR Monitor will tell you here when a pull request needs you.";

/// Title and optional detail line for one PR's headline change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Headline {
    pub title: String,
    pub detail: Option<String>,
}

/// The toast title for a PR's most important change; `extra` more changes are summarized.
pub fn headline(kind: &ChangeKind, number: i32, extra: usize) -> Headline {
    let (what, detail) = describe(kind);
    let more = if extra > 0 {
        format!(" (+{extra} more)")
    } else {
        String::new()
    };
    Headline {
        title: format!("{what} · !{number}{more}"),
        detail,
    }
}

fn name(n: &str) -> String {
    truncate_chars(n, MAX_NAME_CHARS)
}

fn describe(kind: &ChangeKind) -> (String, Option<String>) {
    match kind {
        ChangeKind::NewPr => ("New pull request".into(), None),
        ChangeKind::Reactivated => ("Reopened".into(), None),
        ChangeKind::ReadyForReview => ("Ready for review".into(), None),
        ChangeKind::Closed { outcome } => (closed_word(*outcome).into(), None),
        ChangeKind::Reviewed { headline } => (vote_phrase(*headline).into(), None),
        ChangeKind::NeedsReReview => (
            "Needs review again".into(),
            Some("Earlier votes no longer count.".into()),
        ),
        ChangeKind::ChangesRequested { by, vote } | ChangeKind::NewVote { by, vote } => (
            format!("{} {}", name(&by.display_name()), voter_phrase(*vote)),
            None,
        ),
        ChangeKind::NewComments {
            count,
            authors,
            snippet,
        } => comments_text(*count, authors, snippet.as_deref()),
        ChangeKind::CommentsResolved => ("All comments resolved".into(), None),
        ChangeKind::NewPush => ("New changes pushed".into(), None),
    }
}

const fn closed_word(outcome: CloseOutcome) -> &'static str {
    match outcome {
        CloseOutcome::Completed => "Completed",
        CloseOutcome::Abandoned => "Abandoned",
        CloseOutcome::Unknown => "Closed",
    }
}

/// "Approved", "Waiting for the author", ...
pub const fn vote_phrase(vote: VoteKind) -> &'static str {
    match vote {
        VoteKind::Approved => "Approved",
        VoteKind::ApprovedWithSuggestions => "Approved with suggestions",
        VoteKind::WaitingForAuthor => "Waiting for the author",
        VoteKind::Rejected => "Rejected",
        VoteKind::NoVote | VoteKind::NotApplicable | VoteKind::Other => "Voted",
    }
}

/// Follows a reviewer's name: "{by} approved".
const fn voter_phrase(vote: VoteKind) -> &'static str {
    match vote {
        VoteKind::Approved => "approved",
        VoteKind::ApprovedWithSuggestions => "approved with suggestions",
        VoteKind::WaitingForAuthor => "is waiting for the author",
        VoteKind::Rejected => "rejected",
        VoteKind::NoVote | VoteKind::NotApplicable | VoteKind::Other => "voted",
    }
}

fn comments_text(
    count: u32,
    authors: &[std::sync::Arc<str>],
    snippet: Option<&str>,
) -> (String, Option<String>) {
    let first = authors.first().map_or_else(String::new, |a| name(a));
    if count == 1 {
        let detail = snippet.map(|s| format!("“{s}”"));
        return (format!("{first} commented"), detail);
    }
    let detail = match snippet {
        Some(s) => Some(format!("“{s}” — {first}")),
        None => Some(author_list(authors, count)),
    };
    (format!("{count} new comments"), detail)
}

/// "Ann, Bob and 3 more", from up to two names.
fn author_list(authors: &[std::sync::Arc<str>], count: u32) -> String {
    let named: Vec<String> = authors.iter().take(2).map(|a| name(a)).collect();
    let rest = count.saturating_sub(u32::try_from(named.len()).unwrap_or(u32::MAX));
    let joined = named.join(", ");
    if rest > 0 {
        format!("{joined} and {rest} more")
    } else {
        joined
    }
}

/// Title for the overflow toast.
pub fn summary_title(count: usize) -> String {
    format!("{count} more pull requests updated")
}

/// Up to [`SUMMARY_TITLES`] PR titles joined with " · ", cut to [`MAX_SUMMARY_CHARS`].
pub fn summary_body<'a>(titles: impl IntoIterator<Item = &'a str>) -> String {
    let joined = titles
        .into_iter()
        .take(SUMMARY_TITLES)
        .collect::<Vec<_>>()
        .join(" · ");
    truncate_chars(&joined, MAX_SUMMARY_CHARS)
}

pub fn auth_title(orgs: usize) -> String {
    if orgs == 1 {
        "Sign-in needed".into()
    } else {
        format!("Sign-in needed for {orgs} organizations")
    }
}

/// "{repo} · {author}", plus " · {org}" when several orgs are configured.
pub fn attribution(repo: &str, author: &str, org: Option<&str>) -> String {
    match org {
        Some(org) => format!("{repo} · {} · {org}", name(author)),
        None => format!("{repo} · {}", name(author)),
    }
}

#[cfg(test)]
mod tests;
