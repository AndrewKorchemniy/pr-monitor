//! `GitPullRequestCommentThread`.

use prm_domain::Timestamp;
use serde::Deserialize;

use super::{
    common::{IdentityRefDto, PropValDto},
    date::opt_ts,
};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadDto {
    pub(crate) id: i32,
    #[serde(default)]
    pub(crate) status: Option<ThreadStatusDto>,
    #[serde(default)]
    pub(crate) is_deleted: bool,
    #[serde(default, deserialize_with = "opt_ts")]
    pub(crate) last_updated_date: Option<Timestamp>,
    #[serde(default)]
    pub(crate) comments: Vec<CommentDto>,
    /// `null` in some responses.
    #[serde(default)]
    pub(crate) properties: Option<ThreadPropsDto>,
    #[serde(default)]
    pub(crate) thread_context: Option<ThreadContextDto>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ThreadPropsDto {
    /// Present only on system threads (`VoteUpdate`, `RefUpdate`, ...).
    #[serde(rename = "CodeReviewThreadType", default)]
    pub(crate) code_review_thread_type: Option<PropValDto>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadContextDto {
    #[serde(default)]
    pub(crate) file_path: Option<String>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ThreadStatusDto {
    Unknown,
    Active,
    Fixed,
    WontFix,
    Closed,
    ByDesign,
    Pending,
    #[serde(other)]
    Other,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommentDto {
    pub(crate) id: i32,
    #[serde(default)]
    pub(crate) parent_comment_id: i32,
    #[serde(default)]
    pub(crate) comment_type: Option<CommentTypeDto>,
    #[serde(default)]
    pub(crate) content: Option<String>,
    #[serde(default)]
    pub(crate) is_deleted: bool,
    #[serde(default, deserialize_with = "opt_ts")]
    pub(crate) published_date: Option<Timestamp>,
    #[serde(default)]
    pub(crate) author: Option<IdentityRefDto>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum CommentTypeDto {
    Unknown,
    Text,
    CodeChange,
    System,
    #[serde(other)]
    Other,
}
