//! Which projects and repositories an org watches: all projects, or chosen projects with all or
//! chosen repositories. Searchable; built from the org's catalog.

use std::sync::Arc;

use eframe::egui::{self, RichText};
use prm_domain::{
    model::{Catalog, Project, Repository},
    scope::{ProjectScope, RepoSel, ScopeSpec},
};

use crate::{copy, theme::UiTheme, widgets};

#[derive(Clone, Debug, Default)]
pub(crate) struct ScopeFilter {
    pub text: String,
}

/// Returns true when the user asked to reload the catalog.
pub(crate) fn scope_picker(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    filter: &mut ScopeFilter,
    catalog: Option<&Catalog>,
    scope: &mut ScopeSpec,
) -> bool {
    let mut reload = false;
    ui.horizontal(|ui| {
        ui.checkbox(&mut scope.all_projects, copy::WATCH_ALL_PROJECTS);
        reload = ui.button(copy::REFRESH_LIST).clicked();
    });
    let Some(catalog) = catalog else {
        widgets::muted(ui, theme, copy::LOADING_PROJECTS);
        return reload;
    };
    let total = catalog.repos.len();
    if scope.all_projects {
        widgets::muted(ui, theme, &copy::repos_selected(total, total));
        return reload;
    }
    widgets::muted(
        ui,
        theme,
        &copy::repos_selected(chosen(catalog, scope), total),
    );
    ui.add(
        egui::TextEdit::singleline(&mut filter.text)
            .hint_text(copy::search_scope(catalog.projects.len(), total))
            .desired_width(320.0),
    );
    let needle = filter.text.trim().to_lowercase();
    egui::ScrollArea::vertical()
        .max_height(320.0)
        .show(ui, |ui| {
            for project in &catalog.projects {
                let repos = matching_repos(catalog, project, &needle);
                if !repos.is_empty() || project.name.to_lowercase().contains(&needle) {
                    project_row(ui, theme, project, &repos, scope);
                }
            }
        });
    reload
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
fn matching_repos<'a>(
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

fn project_row(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    project: &Project,
    repos: &[&Repository],
    scope: &mut ScopeSpec,
) {
    let position = scope.projects.iter().position(|p| p.name == project.name);
    let mut included = position.is_some();
    let header = RichText::new(project.name.as_ref()).font(theme.fonts.body_strong.clone());
    if ui.checkbox(&mut included, header).changed() {
        match position {
            Some(index) => {
                scope.projects.remove(index);
            }
            None => scope.projects.push(ProjectScope {
                id: Some(project.id),
                name: project.name.clone(),
                all_repos: true,
                repos: Vec::new(),
            }),
        }
    }
    let Some(entry) = scope.projects.iter_mut().find(|p| p.name == project.name) else {
        return;
    };
    ui.indent(("scope", project.name.as_ref()), |ui| {
        repo_rows(ui, project, repos, entry);
    });
}

fn repo_rows(
    ui: &mut egui::Ui,
    project: &Project,
    repos: &[&Repository],
    entry: &mut ProjectScope,
) {
    let count = u32::try_from(repos.len()).unwrap_or(u32::MAX);
    ui.checkbox(&mut entry.all_repos, copy::all_repos(count, &project.name));
    if entry.all_repos {
        return;
    }
    for repo in repos {
        let at = entry.repos.iter().position(|r| r.name == repo.name);
        let mut on = at.is_some();
        if !ui.checkbox(&mut on, repo.name.as_ref()).changed() {
            continue;
        }
        match at {
            Some(index) => {
                entry.repos.remove(index);
            }
            None => entry.repos.push(RepoSel {
                id: Some(repo.id),
                name: Arc::clone(&repo.name),
            }),
        }
    }
}
