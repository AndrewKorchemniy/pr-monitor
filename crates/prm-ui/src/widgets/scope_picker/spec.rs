//! What an org's scope covers and how the picker edits it: the counts and descriptions for the
//! summary lines, and choosing whole projects or single repositories.

use std::sync::Arc;

use prm_domain::{
    model::{Catalog, Project, Repository},
    scope::{ProjectScope, RepoSel, ScopeSpec},
};

use super::CatalogState;
use crate::copy;

/// "2 of 10 repositories", or that projects are still loading or could not be loaded.
pub(crate) fn summary(catalog: CatalogState<'_>, scope: &ScopeSpec) -> String {
    let catalog = match catalog {
        CatalogState::Ready(catalog) => catalog,
        CatalogState::Loading => return copy::LOADING_PROJECTS.to_owned(),
        CatalogState::Failed(_) => return copy::PROJECTS_FAILED.to_owned(),
    };
    let total = catalog.repos.len();
    let chosen = if scope.all_projects {
        total
    } else {
        chosen(catalog, scope)
    };
    copy::repos_selected(chosen, total)
}

/// "Platform: pr-monitor, build-tools · Identity (all repositories)".
pub(crate) fn detail(scope: &ScopeSpec) -> String {
    if scope.all_projects {
        return copy::WATCH_ALL_PROJECTS.to_owned();
    }
    scope
        .projects
        .iter()
        .map(|p| {
            if p.all_repos {
                copy::whole_project(&p.name)
            } else {
                let names: Vec<&str> = p.repos.iter().map(|r| r.name.as_ref()).collect();
                format!("{}: {}", p.name, names.join(", "))
            }
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Repositories the scope covers.
fn chosen(catalog: &Catalog, scope: &ScopeSpec) -> usize {
    let count = |project: &str| {
        catalog
            .repos
            .iter()
            .filter(|r| r.project_name.as_ref() == project)
            .count()
    };
    scope
        .projects
        .iter()
        .map(|p| {
            if p.all_repos {
                count(&p.name)
            } else {
                p.repos.len()
            }
        })
        .sum()
}

/// A project's enabled repositories matching the search (all of them when the project does).
pub(super) fn matching_repos<'a>(
    catalog: &'a Catalog,
    project: &Project,
    needle: &str,
) -> Vec<&'a Repository> {
    let project_matches = project.name.to_lowercase().contains(needle);
    catalog
        .repos
        .iter()
        .filter(|r| r.project == project.id && !r.disabled)
        .filter(|r| project_matches || r.name.to_lowercase().contains(needle))
        .collect()
}

/// Chooses all of a project's repositories, or none.
pub(super) fn set_project(scope: &mut ScopeSpec, project: &Project, on: bool) {
    scope.projects.retain(|p| p.name != project.name);
    if on {
        scope.projects.push(ProjectScope {
            id: Some(project.id),
            name: project.name.clone(),
            all_repos: true,
            repos: Vec::new(),
        });
    }
}

/// Adds or removes one repository, collapsing to "all" or to nothing at the ends.
pub(super) fn set_repo(
    scope: &mut ScopeSpec,
    (project, all): (&Project, &[&Repository]),
    repo: &Repository,
    on: bool,
) {
    let sel = |r: &Repository| RepoSel {
        id: Some(r.id),
        name: Arc::clone(&r.name),
    };
    let index = scope
        .projects
        .iter()
        .position(|p| p.name == project.name)
        .unwrap_or_else(|| {
            scope.projects.push(ProjectScope {
                id: Some(project.id),
                name: project.name.clone(),
                all_repos: false,
                repos: Vec::new(),
            });
            scope.projects.len() - 1
        });
    let entry = &mut scope.projects[index];
    if entry.all_repos {
        entry.all_repos = false;
        entry.repos = all.iter().map(|r| sel(r)).collect();
    }
    entry.repos.retain(|r| r.name != repo.name);
    if on {
        entry.repos.push(sel(repo));
    }
    if entry.repos.len() == all.len() {
        entry.all_repos = true;
        entry.repos.clear();
    } else if entry.repos.is_empty() {
        scope.projects.remove(index);
    }
}
