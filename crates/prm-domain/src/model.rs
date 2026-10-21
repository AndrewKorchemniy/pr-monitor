//! The domain view of Azure DevOps data. DTOs map into these in exactly one place (`prm-ado::map`).

use std::{sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};

use crate::{
    ids::{CommentId, PrKey, ProjectId, RepoId, ThreadId, Vsid},
    person::Person,
    time::Timestamp,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repository {
    pub id: RepoId,
    pub name: Arc<str>,
    pub project: ProjectId,
    pub project_name: Arc<str>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub in_maintenance: bool,
    #[serde(default)]
    pub is_fork: bool,
    #[serde(default)]
    pub web_url: Option<Arc<str>>,
}

impl Repository {
    /// Disabled and in-maintenance repos are never queried.
    pub const fn is_usable(&self) -> bool {
        !self.disabled && !self.in_maintenance
    }
}

/// The org's projects and repositories, cached for about an hour.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    pub fetched_at: Timestamp,
    pub projects: Vec<Project>,
    pub repos: Vec<Repository>,
}

impl Catalog {
    pub fn is_fresh(&self, now: Timestamp, ttl: Duration) -> bool {
        now.since(self.fetched_at) < ttl && self.fetched_at <= now
    }

    pub fn repo(&self, id: RepoId) -> Option<&Repository> {
        self.repos.iter().find(|r| r.id == id)
    }

    pub fn project(&self, id: ProjectId) -> Option<&Project> {
        self.projects.iter().find(|p| p.id == id)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrState {
    Active,
    Completed,
    Abandoned,
    Other,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeStatus {
    #[default]
    NotSet,
    Queued,
    Conflicts,
    Succeeded,
    RejectedByPolicy,
    Failure,
    Unknown,
}

/// A reviewer vote as Azure DevOps encodes it (`IdentityRefWithVote.vote`).
/// [`Vote::kind`] is the only place that interprets the numbers.
#[derive(
    Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Vote(pub i16);

impl Vote {
    pub const APPROVED: Self = Self(10);
    pub const APPROVED_WITH_SUGGESTIONS: Self = Self(5);
    pub const NO_VOTE: Self = Self(0);
    pub const WAITING_FOR_AUTHOR: Self = Self(-5);
    pub const REJECTED: Self = Self(-10);
    /// "Not applicable / bypassed", only documented from api-version 7.2.
    pub const NOT_APPLICABLE: Self = Self(15);

    pub const fn kind(self) -> VoteKind {
        match self {
            Self::APPROVED => VoteKind::Approved,
            Self::APPROVED_WITH_SUGGESTIONS => VoteKind::ApprovedWithSuggestions,
            Self::NO_VOTE => VoteKind::NoVote,
            Self::WAITING_FOR_AUTHOR => VoteKind::WaitingForAuthor,
            Self::REJECTED => VoteKind::Rejected,
            Self::NOT_APPLICABLE => VoteKind::NotApplicable,
            // why: ADO may add vote values; unknown ones never count as reviews.
            _ => VoteKind::Other,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoteKind {
    Approved,
    ApprovedWithSuggestions,
    NoVote,
    WaitingForAuthor,
    Rejected,
    NotApplicable,
    Other,
}

impl VoteKind {
    /// Approve, approve-with-suggestions, wait-for-author and reject are reviews;
    /// no vote, not applicable and unknown values are not.
    pub const fn is_review(self) -> bool {
        match self {
            Self::Approved
            | Self::ApprovedWithSuggestions
            | Self::WaitingForAuthor
            | Self::Rejected => true,
            Self::NoVote | Self::NotApplicable | Self::Other => false,
        }
    }

    /// Wait-for-author and reject block completion: the author has work to do.
    pub const fn requests_changes(self) -> bool {
        matches!(self, Self::WaitingForAuthor | Self::Rejected)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reviewer {
    pub who: Person,
    pub vote: Vote,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub declined: bool,
    #[serde(default)]
    pub flagged: bool,
    /// Groups this individual's vote rolls up into.
    #[serde(default)]
    pub voted_for: Vec<Vsid>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequest {
    pub key: PrKey,
    pub project: ProjectId,
    pub project_name: Arc<str>,
    pub repo_name: Arc<str>,
    pub title: Arc<str>,
    pub author: Person,
    pub created_at: Timestamp,
    #[serde(default)]
    pub closed_at: Option<Timestamp>,
    pub state: PrState,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub source_branch: Arc<str>,
    #[serde(default)]
    pub target_branch: Arc<str>,
    #[serde(default)]
    pub merge_status: MergeStatus,
    #[serde(default)]
    pub head_commit: Option<Arc<str>>,
    #[serde(default)]
    pub auto_complete: bool,
    #[serde(default)]
    pub reviewers: Vec<Reviewer>,
    pub web_url: Arc<str>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadStatus {
    Active,
    Pending,
    Fixed,
    WontFix,
    Closed,
    ByDesign,
    Unknown,
    /// The thread has no status field (system threads).
    Missing,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentKind {
    Text,
    CodeChange,
    System,
    /// `"unknown"` and unrecognized values; counts as human.
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentDigest {
    pub id: CommentId,
    #[serde(default)]
    pub parent: CommentId,
    pub kind: CommentKind,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub author: Option<Person>,
    #[serde(default)]
    pub published: Option<Timestamp>,
    /// First line of the comment, set only for live, non-system comments.
    #[serde(default)]
    pub snippet: Option<Arc<str>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadDigest {
    pub id: ThreadId,
    pub status: ThreadStatus,
    #[serde(default)]
    pub deleted: bool,
    /// `properties.CodeReviewThreadType.$value` (`VoteUpdate`, `RefUpdate`, ...).
    #[serde(default)]
    pub system_type: Option<Arc<str>>,
    #[serde(default)]
    pub file_path: Option<Arc<str>>,
    #[serde(default)]
    pub last_updated: Option<Timestamp>,
    #[serde(default)]
    pub comments: Vec<CommentDigest>,
}

/// A PR with its threads, as last fetched.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrRecord {
    pub pr: PullRequest,
    #[serde(default)]
    pub threads: Vec<ThreadDigest>,
    pub fetched_at: Timestamp,
}

#[cfg(test)]
mod tests;
