//! Which projects and repositories an org watches: all projects, or chosen projects with all or
//! chosen repositories. Searchable; built from the org's catalog.

use std::sync::Arc;

use eframe::egui::{self, RichText};
use prm_domain::{
    model::{Catalog, Project, Repository},
    scope::{ProjectScope, RepoSel, ScopeSpec},
};

use crate::{
    copy,
    theme::{UiTheme, adapter, icons::Icon},
    widgets::{self, controls},
};

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
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            reload = controls::icon_button(ui, theme, Icon::Refresh, copy::REFRESH_LIST)
                .on_hover_text(copy::REFRESH_LIST)
                .clicked();
            widgets::muted(ui, theme, &summary(catalog, scope));
        });
    });
    let Some(catalog) = catalog else {
        return reload;
    };
    if scope.all_projects {
        return reload;
    }
    ui.add_space(theme.tokens.metrics.space.xs);
    let hint = copy::search_scope(catalog.projects.len(), catalog.repos.len());
    let id = ui.make_persistent_id("scope_search");
    controls::search_field(
        ui,
        theme,
        &mut filter.text,
        (id, &hint),
        ui.available_width(),
    );
    ui.add_space(theme.tokens.metrics.space.xs);
    let needle = filter.text.trim().to_lowercase();
    adapter::frame_sunken(theme)
        .corner_radius(egui::CornerRadius::same(theme.tokens.metrics.radius.md))
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = theme.tokens.metrics.space.xs;
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .auto_shrink([false, true])
                .show(ui, |ui| project_list(ui, theme, catalog, &needle, scope));
        });
    reload
}

/// Projects (and their repositories) matching the search.
fn project_list(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    catalog: &Catalog,
    needle: &str,
    scope: &mut ScopeSpec,
) {
    for project in &catalog.projects {
        let repos = matching_repos(catalog, project, needle);
        if !repos.is_empty() || project.name.to_lowercase().contains(needle) {
            project_row(ui, theme, project, &repos, scope);
        }
    }
}

/// "2 of 10 repositories", or that projects are still loading.
pub(crate) fn summary(catalog: Option<&Catalog>, scope: &ScopeSpec) -> String {
    let Some(catalog) = catalog else {
        return copy::LOADING_PROJECTS.to_owned();
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
