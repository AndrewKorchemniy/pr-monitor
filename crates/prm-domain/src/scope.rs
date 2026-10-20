//! Which projects and repositories an org profile watches, resolved against the catalog.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use serde::{Deserialize, Serialize};

use crate::{
    ids::{ProjectId, RepoId},
    model::{Catalog, Project, Repository},
};

/// Config shape (used verbatim in `config.toml`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScopeSpec {
    pub all_projects: bool,
    pub projects: Vec<ProjectScope>,
}

impl Default for ScopeSpec {
    fn default() -> Self {
        Self {
            all_projects: true,
            projects: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectScope {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<ProjectId>,
    pub name: Arc<str>,
    pub all_repos: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub repos: Vec<RepoSel>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RepoSel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RepoId>,
    pub name: Arc<str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectSel {
    pub name: Arc<str>,
    pub all_repos: bool,
    /// Concrete enabled repo set (all usable repos when `all_repos`).
    pub repos: BTreeSet<RepoId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScopeIssue {
    ProjectNotFound(Arc<str>),
    RepoNotFound { project: Arc<str>, repo: Arc<str> },
    RepoDisabled(Arc<str>),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct ScopeStats {
    pub projects_in_scope: u32,
    pub projects_total: u32,
    pub repos_in_scope: u32,
    pub repos_total: u32,
    /// Repos whose queries failed in the last refresh; set by the refresh job, not here.
    pub repos_failed: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ResolvedScope {
    pub projects: BTreeMap<ProjectId, ProjectSel>,
    pub issues: Vec<ScopeIssue>,
    pub stats: ScopeStats,
}

impl ResolvedScope {
    pub fn contains_repo(&self, project: ProjectId, repo: RepoId) -> bool {
        self.projects
            .get(&project)
            .is_some_and(|p| p.repos.contains(&repo))
    }

    pub fn is_empty(&self) -> bool {
        self.projects.values().all(|p| p.repos.is_empty())
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Ids win; otherwise a case-insensitive name match. Disabled and in-maintenance repos are
/// excluded (and reported when explicitly selected). `all_projects` takes every catalog project.
/// Entries naming the same project merge.
pub fn resolve_scope(spec: &ScopeSpec, catalog: &Catalog) -> ResolvedScope {
    let mut out = ResolvedScope::default();
    if spec.all_projects {
        for project in &catalog.projects {
            add_all_repos(&mut out, project, catalog);
        }
    } else {
        for wanted in &spec.projects {
            resolve_project(&mut out, wanted, catalog);
        }
    }
    out.stats = stats(&out, catalog);
    out
}

fn resolve_project(out: &mut ResolvedScope, wanted: &ProjectScope, catalog: &Catalog) {
    let Some(project) = find_project(catalog, wanted) else {
        out.issues
            .push(ScopeIssue::ProjectNotFound(wanted.name.clone()));
        return;
    };
    if wanted.all_repos {
        add_all_repos(out, project, catalog);
        return;
    }
    let mut repos = BTreeSet::new();
    for sel in &wanted.repos {
        match find_repo(catalog, project, sel) {
            None => out.issues.push(ScopeIssue::RepoNotFound {
                project: project.name.clone(),
                repo: sel.name.clone(),
            }),
            Some(repo) if !repo.is_usable() => {
                out.issues.push(ScopeIssue::RepoDisabled(repo.name.clone()));
            }
            Some(repo) => {
                repos.insert(repo.id);
            }
        }
    }
    selection_for(out, project).repos.extend(repos);
}

fn find_project<'a>(catalog: &'a Catalog, wanted: &ProjectScope) -> Option<&'a Project> {
    let by_name = || {
        catalog
            .projects
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(wanted.name.trim()))
    };
    wanted
        .id
        .and_then(|id| catalog.project(id))
        .or_else(by_name)
}

fn find_repo<'a>(catalog: &'a Catalog, project: &Project, sel: &RepoSel) -> Option<&'a Repository> {
    let by_id = sel
        .id
        .and_then(|id| catalog.repo(id))
        .filter(|repo| repo.project == project.id);
    by_id.or_else(|| {
        catalog
            .repos
            .iter()
            .find(|r| r.project == project.id && r.name.eq_ignore_ascii_case(sel.name.trim()))
    })
}

fn usable_repos(catalog: &Catalog, project: ProjectId) -> BTreeSet<RepoId> {
    catalog
        .repos
        .iter()
        .filter(|r| r.project == project && r.is_usable())
        .map(|r| r.id)
        .collect()
}

/// Selects every usable repo of the project.
fn add_all_repos(out: &mut ResolvedScope, project: &Project, catalog: &Catalog) {
    let selection = selection_for(out, project);
    selection.all_repos = true;
    selection.repos.extend(usable_repos(catalog, project.id));
}

/// The project's selection, created empty on first use so repeated entries merge.
fn selection_for<'a>(out: &'a mut ResolvedScope, project: &Project) -> &'a mut ProjectSel {
    out.projects
        .entry(project.id)
        .or_insert_with(|| ProjectSel {
            name: project.name.clone(),
            all_repos: false,
            repos: BTreeSet::new(),
        })
}

fn stats(scope: &ResolvedScope, catalog: &Catalog) -> ScopeStats {
    let selected = scope.projects.values();
    ScopeStats {
        projects_in_scope: count(selected.clone().filter(|p| !p.repos.is_empty()).count()),
        projects_total: count(catalog.projects.len()),
        repos_in_scope: count(selected.map(|p| p.repos.len()).sum()),
        repos_total: count(catalog.repos.iter().filter(|r| r.is_usable()).count()),
        repos_failed: 0,
    }
}

#[cfg(test)]
mod tests;
