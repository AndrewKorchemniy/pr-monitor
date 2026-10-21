//! The refresh planner: as few PR-list calls as possible for the resolved scope, which listed
//! PRs are tracked, and whose threads to read within a per-refresh budget.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::{
    ids::{PrKey, ProjectId, RepoId, Vsid},
    model::{PrRecord, PrState, PullRequest},
    scope::ResolvedScope,
};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrQuery {
    /// Undocumented `/{org}/_apis/git/pullrequests`; opt-in and probed first.
    OrgWide,
    Project {
        project: ProjectId,
    },
    Repo {
        project: ProjectId,
        repo: RepoId,
    },
}

impl PrQuery {
    pub fn covers(&self, project: ProjectId, repo: RepoId) -> bool {
        match self {
            Self::OrgWide => true,
            Self::Project { project: p } => *p == project,
            Self::Repo {
                project: p,
                repo: r,
            } => *p == project && *r == repo,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum QueryMode {
    PerProject,
    OrgWide,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DraftPolicy {
    Hide,
    /// Last because `#[serde(other)]` must be the final variant.
    #[default]
    #[serde(other)]
    Show,
}

/// Whose pull requests an org tracks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Authors {
    /// Every author in scope except `except`: the signed-in user, when their own pull requests
    /// are left out.
    Everyone { except: Option<Vsid> },
    /// Only these authors: the listed people, plus the signed-in user when included.
    Only(HashSet<Vsid>),
}

impl Authors {
    /// Whether pull requests by `author` are tracked: the one answer to that question.
    pub fn tracks(&self, author: Vsid) -> bool {
        match self {
            Self::Everyone { except } => *except != Some(author),
            Self::Only(authors) => authors.contains(&author),
        }
    }
}

/// Which active pull requests in scope an org tracks. Built from config by
/// `OrgProfile::track_spec`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackSpec {
    pub authors: Authors,
    pub drafts: DraftPolicy,
}

/// Empty scope: no queries. Org-wide: one. Otherwise per project: a project query when all
/// repos or at least two repos are selected, a repo query for exactly one.
pub fn plan_queries(scope: &ResolvedScope, mode: QueryMode) -> Vec<PrQuery> {
    if scope.is_empty() {
        return Vec::new();
    }
    if mode == QueryMode::OrgWide {
        return vec![PrQuery::OrgWide];
    }
    let mut out = Vec::new();
    for (pid, sel) in &scope.projects {
        match sel.repos.len() {
            0 => {}
            1 if !sel.all_repos => {
                if let Some(repo) = sel.repos.iter().next() {
                    out.push(PrQuery::Repo {
                        project: *pid,
                        repo: *repo,
                    });
                }
            }
            _ => out.push(PrQuery::Project { project: *pid }),
        }
    }
    out
}

/// Keeps active PRs by tracked authors in scope (drafts dropped when hidden).
pub fn select_tracked(
    prs: Vec<PullRequest>,
    scope: &ResolvedScope,
    track: &TrackSpec,
) -> Vec<PullRequest> {
    prs.into_iter()
        .filter(|pr| {
            pr.state == PrState::Active
                && track.authors.tracks(pr.author.id)
                && scope.contains_repo(pr.project, pr.key.repo)
                && !(track.drafts == DraftPolicy::Hide && pr.draft)
        })
        .collect()
}

/// Which tracked PRs have their threads read this refresh.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ThreadReads {
    /// Read now, at most the budget.
    pub read: Vec<PullRequest>,
    /// Known PRs over the budget: they keep the threads read before.
    pub reuse: Vec<PullRequest>,
    /// New PRs over the budget, left out until a later refresh reads them (a PR never shows
    /// without its threads, or its first comments would read as new later).
    pub deferred: usize,
}

/// Spreads thread reads over refreshes when more PRs are tracked than `budget`, so tracking
/// everyone in a busy project costs a bounded number of requests per refresh. New PRs go
/// first, so every refresh shows more of them however many known PRs there are; known PRs
/// follow, least recently read first. In both groups the signed-in user's PRs (as author or
/// reviewer) lead.
pub fn plan_thread_reads(
    mut prs: Vec<PullRequest>,
    prev: &BTreeMap<PrKey, PrRecord>,
    me: Vsid,
    budget: usize,
) -> ThreadReads {
    if prs.len() <= budget {
        return ThreadReads {
            read: prs,
            ..ThreadReads::default()
        };
    }
    prs.sort_by_cached_key(|pr| {
        let last_read = prev.get(&pr.key).map(|record| record.fetched_at);
        (
            last_read.is_some(),
            !involves(pr, me),
            last_read,
            pr.key.clone(),
        )
    });
    let over = prs.split_off(budget);
    let (reuse, deferred): (Vec<_>, Vec<_>) =
        over.into_iter().partition(|pr| prev.contains_key(&pr.key));
    ThreadReads {
        read: prs,
        reuse,
        deferred: deferred.len(),
    }
}

/// `me` wrote the PR or is asked to review it.
fn involves(pr: &PullRequest, me: Vsid) -> bool {
    pr.author.id == me || pr.reviewers.iter().any(|r| r.who.id == me)
}

#[cfg(test)]
mod tests;
