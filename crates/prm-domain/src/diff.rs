//! Snapshot diff into change events. Deterministic: sorted by PR key, then by event kind.

use std::{collections::HashSet, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::{
    classify::{CommentMeta, ReviewState},
    ids::{PrKey, Vsid},
    model::VoteKind,
    person::Person,
    snapshot::{ClassifiedPr, OrgSnapshot, PrHeader},
    time::Timestamp,
    tombstone::{CloseOutcome, Tombstones},
};

/// How many distinct commenter names a `NewComments` event carries.
pub const MAX_NAMED_AUTHORS: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeEvent {
    pub pr: PrHeader,
    pub kind: ChangeKind,
    pub at: Timestamp,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChangeKind {
    NewPr,
    Reactivated,
    ReadyForReview,
    Closed {
        outcome: CloseOutcome,
    },
    Reviewed {
        headline: VoteKind,
    },
    NeedsReReview,
    ChangesRequested {
        by: Person,
        vote: VoteKind,
    },
    NewVote {
        by: Person,
        vote: VoteKind,
    },
    NewComments {
        count: u32,
        authors: Arc<[Arc<str>]>,
        snippet: Option<Arc<str>>,
    },
    CommentsResolved,
    NewPush,
}

impl ChangeKind {
    /// Order of kinds within one PR's events. Not persisted.
    pub const fn sort_order(&self) -> u8 {
        match self {
            Self::NewPr => 0,
            Self::Reactivated => 1,
            Self::ReadyForReview => 2,
            Self::Closed { .. } => 3,
            Self::Reviewed { .. } => 4,
            Self::NeedsReReview => 5,
            Self::ChangesRequested { .. } => 6,
            Self::NewVote { .. } => 7,
            Self::NewComments { .. } => 8,
            Self::CommentsResolved => 9,
            Self::NewPush => 10,
        }
    }
}

pub struct DiffInput<'a> {
    /// `None` is the baseline (first refresh without a saved snapshot): no events.
    pub prev: Option<&'a OrgSnapshot>,
    pub next: &'a OrgSnapshot,
    /// PRs that vanished from the active list, resolved via a by-id lookup.
    pub closed: &'a [(PrKey, CloseOutcome)],
    pub tombstones: &'a Tombstones,
    /// The signed-in user: their own comments and votes never notify them.
    pub me: Option<Vsid>,
    /// Same set as `ClassifyConfig::ignored_comment_authors` (bots, pipelines).
    pub ignored: &'a HashSet<Vsid>,
    /// Set after a scope or tracked-author change (rebaseline): no `NewPr` events.
    pub suppress_new: bool,
    pub now: Timestamp,
}

/// Who is excluded from "new comments": the PR author, the signed-in user and ignored authors.
/// The signed-in user's votes are excluded too.
struct CommentFilter<'a> {
    me: Option<Vsid>,
    ignored: &'a HashSet<Vsid>,
}

pub fn diff(input: &DiffInput<'_>) -> Vec<ChangeEvent> {
    let Some(prev) = input.prev else {
        return Vec::new();
    };
    let filter = CommentFilter {
        me: input.me,
        ignored: input.ignored,
    };
    let mut events = Vec::new();
    let mut push = |pr: &PrHeader, kind: ChangeKind| {
        events.push(ChangeEvent {
            pr: pr.clone(),
            kind,
            at: input.now,
        });
    };

    for (key, new) in input.next.iter().filter(|(_, pr)| !pr.stale) {
        match prev.get(key) {
            Some(old) => {
                for kind in pr_changes(old, new, &filter) {
                    push(&new.header, kind);
                }
            }
            None if input.suppress_new => {}
            None if input.tombstones.contains_key(key) => {
                push(&new.header, ChangeKind::Reactivated);
            }
            None => push(&new.header, ChangeKind::NewPr),
        }
    }
    for (key, outcome) in input.closed {
        if let (Some(old), false) = (prev.get(key), input.next.contains_key(key)) {
            push(&old.header, ChangeKind::Closed { outcome: *outcome });
        }
    }

    events.sort_by(|a, b| {
        a.pr.key
            .cmp(&b.pr.key)
            .then(a.kind.sort_order().cmp(&b.kind.sort_order()))
    });
    events
}

/// Everything that changed on one PR present in both snapshots. A title change alone is not
/// an event; a PR that disappears without a resolved close is dropped silently.
fn pr_changes(
    old: &ClassifiedPr,
    new: &ClassifiedPr,
    filter: &CommentFilter<'_>,
) -> Vec<ChangeKind> {
    let mut changes = Vec::new();
    if old.header.draft && !new.header.draft {
        changes.push(ChangeKind::ReadyForReview);
    }
    changes.extend(review_change(
        old.classification.review,
        new.classification.review,
    ));
    changes.extend(vote_changes(old, new, filter.me));
    changes.extend(new_comments(old, new, filter));
    if old.classification.active_threads > 0 && new.classification.active_threads == 0 {
        changes.push(ChangeKind::CommentsResolved);
    }
    if is_new_push(old.head_commit.as_deref(), new.head_commit.as_deref()) {
        changes.push(ChangeKind::NewPush);
    }
    changes
}

fn review_change(old: ReviewState, new: ReviewState) -> Option<ChangeKind> {
    match (old, new) {
        (ReviewState::NeedsReview, ReviewState::Reviewed { headline }) => {
            Some(ChangeKind::Reviewed { headline })
        }
        // Votes were reset by a push policy, or the last counting reviewer was removed.
        (ReviewState::Reviewed { .. }, ReviewState::NeedsReview) => Some(ChangeKind::NeedsReReview),
        (ReviewState::NeedsReview, ReviewState::NeedsReview)
        | (ReviewState::Reviewed { .. }, ReviewState::Reviewed { .. }) => None,
    }
}

/// One event per counted vote that is new or changed, except the signed-in user's own.
/// Their vote still moves the review state (`review_change`), which describes the PR.
fn vote_changes(old: &ClassifiedPr, new: &ClassifiedPr, me: Option<Vsid>) -> Vec<ChangeKind> {
    new.classification
        .counted_votes
        .iter()
        .filter(|(voter, _)| Some(**voter) != me)
        .filter(|(voter, vote)| old.classification.counted_votes.get(voter) != Some(vote))
        .map(|(voter, vote)| {
            let by = new.person(*voter);
            let kind = vote.kind();
            if kind.requests_changes() {
                ChangeKind::ChangesRequested { by, vote: kind }
            } else {
                ChangeKind::NewVote { by, vote: kind }
            }
        })
        .collect()
}

fn new_comments(
    old: &ClassifiedPr,
    new: &ClassifiedPr,
    filter: &CommentFilter<'_>,
) -> Option<ChangeKind> {
    let author = new.header.author.id;
    let counts = |meta: &CommentMeta| {
        meta.author
            .is_some_and(|a| a != author && Some(a) != filter.me && !filter.ignored.contains(&a))
    };
    let fresh: Vec<&CommentMeta> = new
        .classification
        .comments
        .iter()
        .filter(|(key, meta)| !old.classification.comments.contains_key(key) && counts(meta))
        .map(|(_, meta)| meta)
        .collect();
    if fresh.is_empty() {
        return None;
    }
    let newest = fresh.iter().copied().max_by_key(|meta| meta.published);
    // why: the toast quotes the newest comment and credits `authors[0]`, so its author leads.
    let ordered = newest.into_iter().chain(fresh.iter().copied());
    Some(ChangeKind::NewComments {
        count: u32::try_from(fresh.len()).unwrap_or(u32::MAX),
        authors: author_names(new, ordered).into(),
        snippet: newest.and_then(|meta| meta.snippet.clone()),
    })
}

/// Up to [`MAX_NAMED_AUTHORS`] distinct names, in the given comment order.
fn author_names<'a>(
    pr: &ClassifiedPr,
    comments: impl Iterator<Item = &'a CommentMeta>,
) -> Vec<Arc<str>> {
    let mut names: Vec<Arc<str>> = Vec::new();
    for id in comments.filter_map(|meta| meta.author) {
        let name = pr.person(id).display_name();
        if !names.contains(&name) {
            names.push(name);
        }
        if names.len() == MAX_NAMED_AUTHORS {
            break;
        }
    }
    names
}

/// Only a change between two known commits is a push; a missing head commit is not.
fn is_new_push(old: Option<&str>, new: Option<&str>) -> bool {
    matches!((old, new), (Some(a), Some(b)) if a != b)
}

#[cfg(test)]
mod tests;
