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
    theme::{
        UiTheme,
        icons::{self, Icon},
    },
    widgets::{
        self,
        controls::{self, Check},
    },
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
        // why: line the box up with the project boxes, which sit after a chevron.
        ui.add_space(CHEVRON);
        controls::checkbox(ui, theme, &mut scope.all_projects, copy::WATCH_ALL_PROJECTS);
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
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = theme.tokens.metrics.space.xxs;
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
        let shown = matching_repos(catalog, project, needle);
        if shown.is_empty() && !project.name.to_lowercase().contains(needle) {
            continue;
        }
        let all = matching_repos(catalog, project, "");
        let searching = !needle.is_empty();
        project_row(ui, theme, (project, &all), scope, searching);
        let open_id = ui.make_persistent_id(("scope_open", project.name.as_ref()));
        if searching || ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false) {
            ui.indent(("scope", project.name.as_ref()), |ui| {
                repo_rows(ui, theme, (project, &all), &shown, scope);
            });
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

/// A project with a chevron, a tri-state checkbox (all, some or none of its repositories)
/// and how many of them are chosen.
fn project_row(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    (project, all): (&Project, &[&Repository]),
    scope: &mut ScopeSpec,
    searching: bool,
) {
    let position = scope.projects.iter().position(|p| p.name == project.name);
    let chosen = position.map_or(0, |i| {
        let entry = &scope.projects[i];
        if entry.all_repos {
            all.len()
        } else {
            entry.repos.len()
        }
    });
    let open_id = ui.make_persistent_id(("scope_open", project.name.as_ref()));
    let mut open = searching || ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
    ui.horizontal(|ui| {
        if chevron(ui, theme, open).clicked() {
            open = !open;
            ui.data_mut(|d| d.insert_temp(open_id, open));
        }
        let state = match chosen {
            0 => Check::Off,
            n if n == all.len() => Check::On,
            _ => Check::Mixed,
        };
        let header = RichText::new(project.name.as_ref()).font(theme.fonts.body_strong.clone());
        if controls::check(ui, theme, state, header.into()).clicked() {
            set_project(scope, project, state != Check::On);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            widgets::muted(ui, theme, &format!("{chosen}/{}", all.len()));
        });
    });
}

/// The expand chevron's width; other rows indent by it (plus spacing) to line up.
const CHEVRON: f32 = 14.0;

fn chevron(ui: &mut egui::Ui, theme: &UiTheme, open: bool) -> egui::Response {
    let size = egui::vec2(CHEVRON, CHEVRON);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let icon = if open {
        Icon::ChevronDown
    } else {
        Icon::ChevronRight
    };
    let color = if response.hovered() {
        theme.colors.text_primary
    } else {
        theme.colors.text_muted
    };
    icons::paint(ui.painter(), rect.shrink(3.0), icon, color);
    let name = if open { copy::COLLAPSE } else { copy::EXPAND };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    response
}

/// Chooses all of a project's repositories, or none.
fn set_project(scope: &mut ScopeSpec, project: &Project, on: bool) {
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

fn repo_rows(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    (project, all): (&Project, &[&Repository]),
    shown: &[&Repository],
    scope: &mut ScopeSpec,
) {
    for repo in shown {
        let entry = scope.projects.iter().find(|p| p.name == project.name);
        let mut on =
            entry.is_some_and(|p| p.all_repos || p.repos.iter().any(|r| r.name == repo.name));
        if controls::checkbox(ui, theme, &mut on, repo.name.as_ref()).changed() {
            set_repo(scope, (project, all), repo, on);
        }
    }
}

/// Adds or removes one repository, collapsing to "all" or to nothing at the ends.
fn set_repo(
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
