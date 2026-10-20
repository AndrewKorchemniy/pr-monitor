//! Status rules. `ClassifyConfig::counts_vote` is the single source of truth for "this reviewer
//! entry counts as a review"; `classify` and `diff` both rely on it.

use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use serde::{Deserialize, Serialize};

use crate::{
    ids::{CommentId, ThreadId, Vsid},
    model::{CommentKind, PullRequest, Reviewer, ThreadDigest, ThreadStatus, Vote, VoteKind},
    time::Timestamp,
};

/// Individuals and/or group VSIDs whose votes count.
pub type ReviewerSet = HashSet<Vsid>;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmptyReviewerPolicy {
    /// With no reviewer list, nothing counts as a review.
    Nobody,
    /// With no reviewer list, any vote except the author's counts (the default).
    /// Last because `#[serde(other)]` must be the final variant.
    #[default]
    #[serde(other)]
    AnyNonAuthorVote,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ClassifyConfig {
    pub reviewers: ReviewerSet,
    pub empty_list: EmptyReviewerPolicy,
    pub ignored_comment_authors: HashSet<Vsid>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrStatus {
    NeedsReview,
    ActiveComments,
    Reviewed,
}

impl PrStatus {
    pub const ALL: [Self; 3] = [Self::NeedsReview, Self::ActiveComments, Self::Reviewed];

    /// Display order: needs review first.
    pub const fn attention_rank(self) -> u8 {
        match self {
            Self::NeedsReview => 0,
            Self::ActiveComments => 1,
            Self::Reviewed => 2,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::NeedsReview => "Needs review",
            Self::ActiveComments => "Active comments",
            Self::Reviewed => "Reviewed",
        }
    }

    /// Persisted: hashed into fingerprints stored in state.json. Never renumber.
    pub const fn stable_code(self) -> u8 {
        match self {
            Self::NeedsReview => 1,
            Self::ActiveComments => 2,
            Self::Reviewed => 3,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum ReviewState {
    NeedsReview,
    Reviewed { headline: VoteKind },
}

impl ReviewState {
    pub const fn is_reviewed(self) -> bool {
        matches!(self, Self::Reviewed { .. })
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct VoteTally {
    pub approved: u8,
    pub suggestions: u8,
    pub waiting: u8,
    pub rejected: u8,
}

impl VoteTally {
    pub const fn total(self) -> u32 {
        self.approved as u32 + self.suggestions as u32 + self.waiting as u32 + self.rejected as u32
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentMeta {
    pub author: Option<Vsid>,
    pub published: Option<Timestamp>,
    pub snippet: Option<Arc<str>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Classification {
    pub status: PrStatus,
    pub review: ReviewState,
    pub tally: VoteTally,
    pub active_threads: u32,
    pub total_threads: u32,
    /// Only votes that count (diff input).
    pub counted_votes: BTreeMap<Vsid, Vote>,
    /// Live, non-system comments in human threads.
    pub comments: BTreeMap<(ThreadId, CommentId), CommentMeta>,
}

/// Whether a vote counts as a review at all (before the reviewer list is applied).
pub const fn is_review_vote(v: Vote) -> bool {
    v.kind().is_review()
}

impl ClassifyConfig {
    pub fn counts_vote(&self, pr: &PullRequest, r: &Reviewer) -> bool {
        if r.who.id == pr.author.id || r.declined || !is_review_vote(r.vote) {
            return false;
        }
        if self.reviewers.is_empty() {
            return self.empty_list == EmptyReviewerPolicy::AnyNonAuthorVote && !r.who.group;
        }
        if !self.reviewers.contains(&r.who.id) {
            return false;
        }
        !(r.who.group && group_vote_only_from_author(pr, r.who.id))
    }
}

/// True iff at least one individual lists `group` in `voted_for` with a review vote and all such
/// voters are the PR author (a group vote that is really the author's self-vote).
fn group_vote_only_from_author(pr: &PullRequest, group: Vsid) -> bool {
    let voters: Vec<&Reviewer> = pr
        .reviewers
        .iter()
        .filter(|r| !r.who.group && is_review_vote(r.vote) && r.voted_for.contains(&group))
        .collect();
    !voters.is_empty() && voters.iter().all(|r| r.who.id == pr.author.id)
}

/// A thread written by people: not deleted, not a system thread, first live comment not a
/// system comment, and not started by an ignored author (bots, pipelines).
pub fn is_human_thread(t: &ThreadDigest, ignored: &HashSet<Vsid>) -> bool {
    if t.deleted || t.system_type.is_some() {
        return false;
    }
    let Some(first) = t.comments.iter().find(|c| !c.deleted) else {
        return false;
    };
    first.kind != CommentKind::System
        && first
            .author
            .as_ref()
            .is_none_or(|a| !ignored.contains(&a.id))
}

pub const fn is_unresolved(s: ThreadStatus) -> bool {
    matches!(s, ThreadStatus::Active | ThreadStatus::Pending)
}

pub fn classify(
    pr: &PullRequest,
    threads: &[ThreadDigest],
    cfg: &ClassifyConfig,
) -> Classification {
    let (counted_votes, tally) = count_votes(pr, cfg);
    let review = review_state(&counted_votes, tally);
    let threads = count_threads(threads, &cfg.ignored_comment_authors);
    let status = if threads.active > 0 {
        PrStatus::ActiveComments
    } else if review.is_reviewed() {
        PrStatus::Reviewed
    } else {
        PrStatus::NeedsReview
    };
    Classification {
        status,
        review,
        tally,
        active_threads: threads.active,
        total_threads: threads.total,
        counted_votes,
        comments: threads.comments,
    }
}

/// The votes that count as reviews, and how many of each kind.
fn count_votes(pr: &PullRequest, cfg: &ClassifyConfig) -> (BTreeMap<Vsid, Vote>, VoteTally) {
    let mut counted = BTreeMap::new();
    let mut tally = VoteTally::default();
    for reviewer in pr.reviewers.iter().filter(|r| cfg.counts_vote(pr, r)) {
        counted.insert(reviewer.who.id, reviewer.vote);
        let slot = match reviewer.vote.kind() {
            VoteKind::Approved => &mut tally.approved,
            VoteKind::ApprovedWithSuggestions => &mut tally.suggestions,
            VoteKind::WaitingForAuthor => &mut tally.waiting,
            VoteKind::Rejected => &mut tally.rejected,
            VoteKind::NoVote | VoteKind::NotApplicable | VoteKind::Other => continue,
        };
        *slot = slot.saturating_add(1);
    }
    (counted, tally)
}

/// The most severe counted vote is the headline: reject, then wait, then approve.
fn review_state(counted: &BTreeMap<Vsid, Vote>, tally: VoteTally) -> ReviewState {
    if counted.is_empty() {
        return ReviewState::NeedsReview;
    }
    let headline = if tally.rejected > 0 {
        VoteKind::Rejected
    } else if tally.waiting > 0 {
        VoteKind::WaitingForAuthor
    } else if tally.approved > 0 {
        VoteKind::Approved
    } else {
        VoteKind::ApprovedWithSuggestions
    };
    ReviewState::Reviewed { headline }
}

struct ThreadCounts {
    active: u32,
    total: u32,
    comments: BTreeMap<(ThreadId, CommentId), CommentMeta>,
}

/// Counts human threads (resolution is per thread, not per comment) and collects their live
/// comments for diffing.
fn count_threads(threads: &[ThreadDigest], ignored: &HashSet<Vsid>) -> ThreadCounts {
    let mut counts = ThreadCounts {
        active: 0,
        total: 0,
        comments: BTreeMap::new(),
    };
    for thread in threads.iter().filter(|t| is_human_thread(t, ignored)) {
        counts.total += 1;
        if is_unresolved(thread.status) {
            counts.active += 1;
        }
        let live = thread
            .comments
            .iter()
            .filter(|c| !c.deleted && c.kind != CommentKind::System);
        for comment in live {
            let meta = CommentMeta {
                author: comment.author.as_ref().map(|a| a.id),
                published: comment.published,
                snippet: comment.snippet.clone(),
            };
            counts.comments.insert((thread.id, comment.id), meta);
        }
    }
    counts
}

#[cfg(test)]
mod tests;
