//! The only place wire DTOs become domain types.

use std::sync::Arc;

use prm_domain::{
    Person, PrKey, Timestamp, Vsid,
    ids::{CommentId, PrNumber, ProjectId, RepoId, ThreadId},
    model::{
        CommentDigest, CommentKind, MergeStatus, PrState, Project, PullRequest, Repository,
        Reviewer, ThreadDigest, ThreadStatus, Vote,
    },
    person::{CandidateSource, PersonCandidate, non_blank},
    view::text::{snippet, strip_ref_prefix},
};

use crate::{
    dto::{
        common::IdentityRefDto,
        core::{ProjectDto, RepositoryDto},
        identity::{GraphSubjectDto, ImsIdentityDto},
        pr::{MergeStatusDto, PrStatusDto, PullRequestDto, ReviewerDto},
        thread::{CommentDto, CommentTypeDto, ThreadDto, ThreadStatusDto},
    },
    urls::OrgEndpoint,
};

fn text(s: String) -> Arc<str> {
    Arc::from(s)
}

fn opt_text(s: Option<String>) -> Option<Arc<str>> {
    s.filter(|v| non_blank(v)).map(Arc::from)
}

pub(crate) fn person(d: IdentityRefDto) -> Person {
    Person {
        id: Vsid(d.id),
        name: text(d.display_name),
        unique_name: opt_text(d.unique_name),
        group: d.is_container,
        descriptor: opt_text(d.descriptor),
    }
}

pub(crate) fn pull_request(endpoint: &OrgEndpoint, d: PullRequestDto) -> PullRequest {
    let number = PrNumber(d.pull_request_id);
    let repo = d.repository;
    let web_url = endpoint.pr_web(&repo.project.name, &repo.name, number);
    let created_at = d.creation_date.unwrap_or_else(|| {
        tracing::warn!(pr = number.0, "pull request without a creation date");
        Timestamp::UNIX_EPOCH
    });
    PullRequest {
        key: PrKey::new(endpoint.org().clone(), RepoId(repo.id), number),
        project: ProjectId(repo.project.id),
        project_name: text(repo.project.name),
        repo_name: text(repo.name),
        title: text(d.title),
        author: person(d.created_by),
        created_at,
        closed_at: d.closed_date,
        state: pr_state(d.status),
        draft: d.is_draft,
        source_branch: strip_ref_prefix(&d.source_ref_name).into(),
        target_branch: strip_ref_prefix(&d.target_ref_name).into(),
        merge_status: d.merge_status.map_or(MergeStatus::NotSet, merge_status),
        head_commit: d
            .last_merge_source_commit
            .map(|c| c.commit_id)
            .filter(|id| !id.is_empty())
            .map(Arc::from),
        auto_complete: d.auto_complete_set_by.is_some(),
        reviewers: d.reviewers.into_iter().map(reviewer).collect(),
        web_url: Arc::from(web_url.as_str()),
    }
}

const fn pr_state(status: PrStatusDto) -> PrState {
    match status {
        PrStatusDto::Active => PrState::Active,
        PrStatusDto::Completed => PrState::Completed,
        PrStatusDto::Abandoned => PrState::Abandoned,
        PrStatusDto::NotSet | PrStatusDto::All | PrStatusDto::Unknown => PrState::Other,
    }
}

const fn merge_status(status: MergeStatusDto) -> MergeStatus {
    match status {
        MergeStatusDto::NotSet => MergeStatus::NotSet,
        MergeStatusDto::Queued => MergeStatus::Queued,
        MergeStatusDto::Conflicts => MergeStatus::Conflicts,
        MergeStatusDto::Succeeded => MergeStatus::Succeeded,
        MergeStatusDto::RejectedByPolicy => MergeStatus::RejectedByPolicy,
        MergeStatusDto::Failure => MergeStatus::Failure,
        MergeStatusDto::Unknown => MergeStatus::Unknown,
    }
}

fn reviewer(d: ReviewerDto) -> Reviewer {
    Reviewer {
        who: Person {
            id: Vsid(d.id),
            name: text(d.display_name),
            unique_name: opt_text(d.unique_name),
            group: d.is_container,
            descriptor: None,
        },
        vote: Vote(d.vote),
        required: d.is_required,
        declined: d.has_declined,
        flagged: d.is_flagged,
        voted_for: d.voted_for.into_iter().map(|v| Vsid(v.id)).collect(),
    }
}

pub(crate) fn thread(d: ThreadDto) -> ThreadDigest {
    let system_type = d
        .properties
        .and_then(|p| p.code_review_thread_type)
        .and_then(|v| v.value.as_str().map(Arc::from));
    ThreadDigest {
        id: ThreadId(d.id),
        status: d.status.map_or(ThreadStatus::Missing, thread_status),
        deleted: d.is_deleted,
        system_type,
        file_path: d.thread_context.and_then(|c| opt_text(c.file_path)),
        last_updated: d.last_updated_date,
        comments: d.comments.into_iter().map(comment).collect(),
    }
}

const fn thread_status(status: ThreadStatusDto) -> ThreadStatus {
    match status {
        ThreadStatusDto::Active => ThreadStatus::Active,
        ThreadStatusDto::Pending => ThreadStatus::Pending,
        ThreadStatusDto::Fixed => ThreadStatus::Fixed,
        ThreadStatusDto::WontFix => ThreadStatus::WontFix,
        ThreadStatusDto::Closed => ThreadStatus::Closed,
        ThreadStatusDto::ByDesign => ThreadStatus::ByDesign,
        ThreadStatusDto::Unknown | ThreadStatusDto::Other => ThreadStatus::Unknown,
    }
}

fn comment(d: CommentDto) -> CommentDigest {
    let kind = match d.comment_type {
        Some(CommentTypeDto::Text) => CommentKind::Text,
        Some(CommentTypeDto::CodeChange) => CommentKind::CodeChange,
        Some(CommentTypeDto::System) => CommentKind::System,
        Some(CommentTypeDto::Unknown | CommentTypeDto::Other) | None => CommentKind::Unknown,
    };
    let live_text = !d.is_deleted && kind != CommentKind::System;
    CommentDigest {
        id: CommentId(d.id),
        parent: CommentId(d.parent_comment_id),
        kind,
        deleted: d.is_deleted,
        author: d.author.map(person),
        published: d.published_date,
        snippet: live_text
            .then(|| d.content.as_deref().and_then(snippet))
            .flatten()
            .map(Arc::from),
    }
}

pub(crate) fn project(d: ProjectDto) -> Project {
    Project {
        id: ProjectId(d.id),
        name: text(d.name),
    }
}

pub(crate) fn repository(d: RepositoryDto) -> Repository {
    Repository {
        id: RepoId(d.id),
        name: text(d.name),
        project: ProjectId(d.project.id),
        project_name: text(d.project.name),
        disabled: d.is_disabled,
        in_maintenance: d.is_in_maintenance,
        is_fork: d.is_fork,
        web_url: opt_text(d.web_url),
    }
}

/// An IMS identity (connectionData, identities search). Name: custom over provider display
/// name; unique name: the `Account` property, else `Mail`.
pub(crate) fn ims_person(d: ImsIdentityDto) -> Person {
    let property = |key: &str| {
        d.properties
            .get(key)
            .and_then(|p| p.value.as_str())
            .map(str::to_owned)
    };
    let unique_name = property("Account").or_else(|| property("Mail"));
    let name = d
        .custom_display_name
        .filter(|n| non_blank(n))
        .or(d.provider_display_name)
        .unwrap_or_default();
    Person {
        id: Vsid(d.id),
        name: text(name),
        unique_name: opt_text(unique_name),
        group: d.is_container,
        descriptor: opt_text(d.subject_descriptor),
    }
}

/// A Graph subject: no VSID yet (resolved later via storage keys).
pub(crate) fn candidate(d: GraphSubjectDto) -> PersonCandidate {
    let group = d
        .subject_kind
        .as_deref()
        .is_some_and(|k| k.eq_ignore_ascii_case("group"));
    PersonCandidate {
        id: None,
        descriptor: Some(text(d.descriptor)),
        name: text(d.display_name),
        unique_name: opt_text(d.mail_address.or(d.principal_name)),
        group,
        source: CandidateSource::Directory,
    }
}

#[cfg(test)]
mod tests;
