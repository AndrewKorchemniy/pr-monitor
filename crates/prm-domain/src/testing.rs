//! Deterministic builders and fakes for tests in this and downstream crates
//! (enabled by `#[cfg(test)]` or the `test-support` feature).
#![allow(
    clippy::panic,
    reason = "test fixtures fail loudly on bad literals; the lint is silent under cfg(test), so expect would be unfulfilled there"
)]

use std::sync::{Arc, Mutex, PoisonError};

use url::Url;

use crate::{
    ids::{CommentId, OrgId, PrKey, PrNumber, ProjectId, RepoId, ThreadId, Vsid},
    model::{
        CommentDigest, CommentKind, MergeStatus, PrRecord, PrState, PullRequest, Reviewer,
        ThreadDigest, ThreadStatus, Vote,
    },
    person::Person,
    ports::{Clock, PortError, PortErrorKind, UrlOpener},
    time::Timestamp,
};

/// Base time for builders: 2023-11-14T22:13:20Z.
pub const BASE_TIME_SECS: i64 = 1_700_000_000;

/// `00000000-0000-0000-0000-{n:012}`.
pub const fn vsid(n: u128) -> Vsid {
    Vsid::from_u128(n)
}

pub const fn repo(n: u128) -> RepoId {
    RepoId::from_u128(n)
}

pub const fn project(n: u128) -> ProjectId {
    ProjectId::from_u128(n)
}

/// Panics on an invalid name; tests only use literals.
pub fn org(name: &str) -> OrgId {
    OrgId::parse(name).unwrap_or_else(|e| panic!("bad test org {name}: {e}"))
}

pub fn key(org_name: &str, repo_n: u128, pr: i32) -> PrKey {
    PrKey::new(org(org_name), repo(repo_n), PrNumber(pr))
}

pub fn person(n: u128, name: &str) -> Person {
    Person::new(vsid(n), name)
}

pub fn group(n: u128, name: &str) -> Person {
    Person {
        group: true,
        ..person(n, name)
    }
}

/// Seconds since the Unix epoch; panics on out-of-range input.
pub fn ts(secs: i64) -> Timestamp {
    Timestamp::from_unix(secs).unwrap_or_else(|| panic!("bad test timestamp {secs}"))
}

pub fn record(pr: PullRequest, threads: Vec<ThreadDigest>) -> PrRecord {
    PrRecord {
        pr,
        threads,
        fetched_at: ts(BASE_TIME_SECS),
    }
}

/// Builds a `PullRequest` in project 1 with sensible defaults.
#[derive(Clone, Debug)]
pub struct PrBuilder {
    pr: PullRequest,
}

impl PrBuilder {
    pub fn new(key: PrKey) -> Self {
        let web_url = format!(
            "https://dev.azure.com/{}/Payments/_git/api/pullrequest/{}",
            key.org, key.number
        );
        let title = format!("PR {}", key.number);
        Self {
            pr: PullRequest {
                key,
                project: project(1),
                project_name: "Payments".into(),
                repo_name: "api".into(),
                title: title.into(),
                author: person(1, "Author One"),
                created_at: ts(BASE_TIME_SECS),
                closed_at: None,
                state: PrState::Active,
                draft: false,
                source_branch: "feature/x".into(),
                target_branch: "main".into(),
                merge_status: MergeStatus::Succeeded,
                head_commit: None,
                auto_complete: false,
                reviewers: Vec::new(),
                web_url: web_url.into(),
            },
        }
    }

    pub fn author(mut self, author: Person) -> Self {
        self.pr.author = author;
        self
    }

    pub fn title(mut self, title: &str) -> Self {
        self.pr.title = title.into();
        self
    }

    pub fn project(mut self, n: u128) -> Self {
        self.pr.project = project(n);
        self
    }

    pub fn head(mut self, commit: &str) -> Self {
        self.pr.head_commit = Some(commit.into());
        self
    }

    pub fn created(mut self, at: Timestamp) -> Self {
        self.pr.created_at = at;
        self
    }

    pub fn state(mut self, state: PrState) -> Self {
        self.pr.state = state;
        self
    }

    pub fn draft(mut self) -> Self {
        self.pr.draft = true;
        self
    }

    pub fn conflicts(mut self) -> Self {
        self.pr.merge_status = MergeStatus::Conflicts;
        self
    }

    pub fn reviewer(self, who: Person, vote: i16) -> Self {
        self.push_reviewer(who, vote, false, &[])
    }

    /// A team reviewer entry (`isContainer`) with its rolled-up vote.
    pub fn group_reviewer(self, who: Person, vote: i16) -> Self {
        let who = Person { group: true, ..who };
        self.push_reviewer(who, vote, false, &[])
    }

    /// An individual whose vote rolls up into the given group ids.
    pub fn reviewer_for(self, who: Person, vote: i16, groups: &[u128]) -> Self {
        self.push_reviewer(who, vote, false, groups)
    }

    pub fn declined(self, who: Person, vote: i16) -> Self {
        self.push_reviewer(who, vote, true, &[])
    }

    fn push_reviewer(mut self, who: Person, vote: i16, declined: bool, groups: &[u128]) -> Self {
        self.pr.reviewers.push(Reviewer {
            who,
            vote: Vote(vote),
            required: false,
            declined,
            flagged: false,
            voted_for: groups.iter().map(|g| vsid(*g)).collect(),
        });
        self
    }

    pub fn build(self) -> PullRequest {
        self.pr
    }
}

/// Builds a `ThreadDigest`: active, human, no comments until added.
#[derive(Clone, Debug)]
pub struct ThreadBuilder {
    thread: ThreadDigest,
}

impl ThreadBuilder {
    pub fn human(id: i32) -> Self {
        Self {
            thread: ThreadDigest {
                id: ThreadId(id),
                status: ThreadStatus::Active,
                deleted: false,
                system_type: None,
                file_path: None,
                last_updated: None,
                comments: Vec::new(),
            },
        }
    }

    pub fn status(mut self, status: ThreadStatus) -> Self {
        self.thread.status = status;
        self
    }

    pub fn file(mut self, path: &str) -> Self {
        self.thread.file_path = Some(path.into());
        self
    }

    /// Marks this as a system thread (`properties.CodeReviewThreadType`).
    pub fn system(mut self, thread_type: &str) -> Self {
        self.thread.system_type = Some(thread_type.into());
        self
    }

    pub fn deleted(mut self) -> Self {
        self.thread.deleted = true;
        self
    }

    pub fn comment(self, author: Person) -> Self {
        self.push_comment(author, CommentKind::Text, false)
    }

    pub fn comment_kind(self, author: Person, kind: CommentKind) -> Self {
        self.push_comment(author, kind, false)
    }

    pub fn deleted_comment(self, author: Person) -> Self {
        self.push_comment(author, CommentKind::Text, true)
    }

    fn push_comment(mut self, author: Person, kind: CommentKind, deleted: bool) -> Self {
        let n = i32::try_from(self.thread.comments.len()).unwrap_or(i32::MAX) + 1;
        let live_text = !deleted && kind != CommentKind::System;
        self.thread.comments.push(CommentDigest {
            id: CommentId(n),
            parent: CommentId(i32::from(n != 1)),
            kind,
            deleted,
            author: Some(author),
            published: Some(ts(BASE_TIME_SECS + i64::from(n))),
            snippet: live_text.then(|| Arc::from(format!("comment {n}"))),
        });
        self
    }

    pub fn build(self) -> ThreadDigest {
        self.thread
    }
}

/// A clock tests move by hand.
#[derive(Debug)]
pub struct ManualClock {
    now: Mutex<Timestamp>,
}

impl ManualClock {
    pub fn new(now: Timestamp) -> Self {
        Self {
            now: Mutex::new(now),
        }
    }

    pub fn set(&self, now: Timestamp) {
        *self.now.lock().unwrap_or_else(PoisonError::into_inner) = now;
    }

    pub fn advance(&self, by: std::time::Duration) {
        let mut now = self.now.lock().unwrap_or_else(PoisonError::into_inner);
        *now = now.saturating_add(by);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Timestamp {
        *self.now.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Records opened URLs; can be told to fail the next call.
#[derive(Debug, Default)]
pub struct RecordingUrlOpener {
    opened: Mutex<Vec<Url>>,
    fail_next: Mutex<bool>,
}

impl RecordingUrlOpener {
    pub fn opened(&self) -> Vec<Url> {
        self.opened
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn fail_next(&self) {
        *self
            .fail_next
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = true;
    }
}

impl UrlOpener for RecordingUrlOpener {
    fn open(&self, url: &Url) -> Result<(), PortError> {
        let mut fail = self
            .fail_next
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if std::mem::take(&mut *fail) {
            return Err(PortError::new(PortErrorKind::Failed, "scripted failure"));
        }
        self.opened
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(url.clone());
        Ok(())
    }
}

/// Rows for one org ("contoso") built through the real pipeline, with `reviewers` listed and
/// nothing seen yet.
pub fn rows(records: &[PrRecord], reviewers: &[u128]) -> Vec<crate::view::PrRow> {
    use std::collections::{BTreeMap, HashSet};

    use crate::{
        classify::ClassifyConfig,
        snapshot::{SeenMap, classify_record},
        view::{RowBuildInput, build_rows},
    };

    let config = ClassifyConfig {
        reviewers: reviewers.iter().map(|n| vsid(*n)).collect(),
        ..ClassifyConfig::default()
    };
    let snapshot = records
        .iter()
        .map(|r| (r.pr.key.clone(), classify_record(r, &config, false)))
        .collect();
    let by_key: BTreeMap<PrKey, PrRecord> = records
        .iter()
        .map(|r| (r.pr.key.clone(), r.clone()))
        .collect();
    let org_id = org("contoso");
    let label: Arc<str> = Arc::from("Contoso");
    let ignored = HashSet::new();
    let input = RowBuildInput {
        org: &org_id,
        org_label: &label,
        snapshot: &snapshot,
        records: &by_key,
        reviewers: &config.reviewers,
        ignored: &ignored,
        org_stale: false,
    };
    build_rows(&[input], &SeenMap::new(), ts(BASE_TIME_SECS + 3_600))
}
