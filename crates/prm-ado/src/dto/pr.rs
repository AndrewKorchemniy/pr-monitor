//! `GitPullRequest`.

use prm_domain::Timestamp;
use serde::Deserialize;
use uuid::Uuid;

use super::{
    common::{IdentityRefDto, ProjectRefDto},
    date::opt_ts,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PullRequestDto {
    pub(crate) pull_request_id: i32,
    #[serde(default)]
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) status: PrStatusDto,
    #[serde(default)]
    pub(crate) is_draft: bool,
    pub(crate) created_by: IdentityRefDto,
    #[serde(default, deserialize_with = "opt_ts")]
    pub(crate) creation_date: Option<Timestamp>,
    #[serde(default, deserialize_with = "opt_ts")]
    pub(crate) closed_date: Option<Timestamp>,
    pub(crate) repository: GitRepoRefDto,
    #[serde(default)]
    pub(crate) reviewers: Vec<ReviewerDto>,
    #[serde(default)]
    pub(crate) source_ref_name: String,
    #[serde(default)]
    pub(crate) target_ref_name: String,
    #[serde(default)]
    pub(crate) merge_status: Option<MergeStatusDto>,
    #[serde(default)]
    pub(crate) last_merge_source_commit: Option<CommitRefDto>,
    #[serde(default)]
    pub(crate) auto_complete_set_by: Option<IdentityRefDto>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PrStatusDto {
    NotSet,
    Active,
    Abandoned,
    Completed,
    All,
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MergeStatusDto {
    NotSet,
    Queued,
    Conflicts,
    Succeeded,
    RejectedByPolicy,
    Failure,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReviewerDto {
    pub(crate) id: Uuid,
    #[serde(default)]
    pub(crate) display_name: String,
    #[serde(default)]
    pub(crate) unique_name: Option<String>,
    #[serde(default)]
    pub(crate) is_container: bool,
    #[serde(default)]
    pub(crate) vote: i16,
    #[serde(default)]
    pub(crate) has_declined: bool,
    #[serde(default)]
    pub(crate) is_required: bool,
    #[serde(default)]
    pub(crate) is_flagged: bool,
    #[serde(default)]
    pub(crate) voted_for: Vec<VotedForDto>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct VotedForDto {
    pub(crate) id: Uuid,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GitRepoRefDto {
    pub(crate) id: Uuid,
    #[serde(default)]
    pub(crate) name: String,
    pub(crate) project: ProjectRefDto,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommitRefDto {
    #[serde(default)]
    pub(crate) commit_id: String,
}
