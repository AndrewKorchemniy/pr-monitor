//! The query planner: as few PR-list calls as possible for the resolved scope.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    ids::{ProjectId, RepoId, Vsid},
    model::{PrState, PullRequest},
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

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TrackSpec {
    /// Tracked authors, including the signed-in user when `include_me`.
    pub authors: HashSet<Vsid>,
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
                && track.authors.contains(&pr.author.id)
                && scope.contains_repo(pr.project, pr.key.repo)
                && !(track.drafts == DraftPolicy::Hide && pr.draft)
        })
        .collect()
}

#[cfg(test)]
mod tests;
