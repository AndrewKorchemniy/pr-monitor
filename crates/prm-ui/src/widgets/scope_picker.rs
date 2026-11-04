//! Which projects and repositories an org watches: all projects, or chosen projects with all or
//! chosen repositories. Searchable; built from the org's catalog. In a narrow dialog the summary
//! moves under the "Watch all projects" line; in a short window the list is shorter, and in a
//! very short one it scrolls with the dialog instead of in a box of its own.

mod spec;

pub(crate) use spec::{detail, summary};
use spec::{matching_repos, set_project, set_repo};

use eframe::egui::{self, RichText};
use prm_domain::{
    model::{Catalog, Project, Repository},
    scope::ScopeSpec,
};
use prm_service::{OrgStatus, UiError};

use crate::{
    copy,
    theme::{
        UiTheme,
        icons::{self, Icon},
    },
    widgets::{
        self,
        controls::{self, Check},
        fields::{self, Field},
    },
};

/// The project list's height in a tall window, in points.
const LIST_HEIGHT: f32 = 300.0;
/// The window height the list leaves to everything around it (the dialog's header, the search
/// field, the action bar and the margins), in points.
const LIST_SURROUND: f32 = 400.0;
/// From this window height (points) the list scrolls in a box of its own. In a shorter window
/// that box would be a few rows scrolling inside a scrolling dialog, so the list is laid out in
/// full and the dialog's body (or the first-run page) is the one thing that scrolls.
const LIST_BOX_FROM: f32 = 560.0;

/// How the project list is boxed.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum ListBox {
    /// As tall as its rows up to `LIST_HEIGHT` in a tall window, laid out in full in a short one.
    Auto,
    /// Exactly the height left in the dialog (below the search field), so the list can change
    /// without the dialog changing; laid out in full when less than `MIN_FILL` is left.
    Fill,
}

/// The least height (points) a `ListBox::Fill` list gets its own scrolling box at.
const MIN_FILL: f32 = 120.0;

#[derive(Clone, Debug, Default)]
pub(crate) struct ScopeFilter {
    pub text: String,
}

/// Where an org's project list stands.
#[derive(Copy, Clone, Debug)]
pub(crate) enum CatalogState<'a> {
    Loading,
    /// The last load failed and nothing is cached; the refresh button tries again.
    Failed(&'a UiError),
    Ready(&'a Catalog),
}

impl<'a> CatalogState<'a> {
    /// The org's catalog, else its load failure, else still loading (also for an unknown org).
    pub(crate) fn of(org: Option<&'a OrgStatus>) -> Self {
        let Some(org) = org else {
            return Self::Loading;
        };
        match (&org.catalog, &org.catalog_error) {
            (Some(catalog), _) => Self::Ready(catalog),
            (None, Some(error)) => Self::Failed(error),
            (None, None) => Self::Loading,
        }
    }
}

/// Returns true when the user asked to reload the catalog.
pub(crate) fn scope_picker(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    (filter, catalog): (&mut ScopeFilter, CatalogState<'_>),
    scope: &mut ScopeSpec,
    list: ListBox,
) -> bool {
    let reload = header(ui, theme, catalog, scope);
    let CatalogState::Ready(catalog) = catalog else {
        return reload;
    };
    if scope.all_projects {
        return reload;
    }
    ui.add_space(theme.tokens.metrics.space.sm);
    let hint = copy::search_scope(catalog.projects.len(), catalog.repos.len());
    let id = egui::Id::unique("scope_search");
    fields::text_field(ui, theme, &mut filter.text, &Field::new(id, &hint));
    ui.add_space(theme.tokens.metrics.space.sm);
    let needle = filter.text.trim().to_lowercase();
    let list_box = list_height(ui, list);
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = theme.tokens.metrics.space.xxs;
        match list_box {
            Some(height) => {
                egui::ScrollArea::vertical()
                    .max_height(height)
                    .auto_shrink([false, list == ListBox::Auto])
                    .show(ui, |ui| project_list(ui, theme, catalog, &needle, scope));
            }
            None => project_list(ui, theme, catalog, &needle, scope),
        }
    });
    reload
}

/// The height of the list's own scrolling box, or `None` to lay the list out in full.
fn list_height(ui: &egui::Ui, list: ListBox) -> Option<f32> {
    match list {
        ListBox::Auto => {
            let window = ui.ctx().content_rect().height();
            (window >= LIST_BOX_FROM).then(|| (window - LIST_SURROUND).min(LIST_HEIGHT))
        }
        ListBox::Fill => {
            let left = ui.available_height().floor();
            (left >= MIN_FILL).then_some(left)
        }
    }
}

/// "Watch all projects" with the refresh button at the right, and how many repositories are
/// chosen (or that the list failed) between them, or under them when the line is too narrow.
/// Returns true when the refresh button was clicked.
fn header(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    catalog: CatalogState<'_>,
    scope: &mut ScopeSpec,
) -> bool {
    let line = summary(catalog, scope);
    let spacing = ui.spacing().item_spacing.x;
    let measure = |text: &str| {
        let font = theme.fonts.small.clone();
        let galley = ui
            .painter()
            .layout_no_wrap(text.to_owned(), font, theme.colors.text_primary);
        galley.size().x
    };
    let needed = CHEVRON
        + controls::check_width(ui, theme, copy::WATCH_ALL_PROJECTS)
        + measure(&line)
        + theme.control_height()
        + 4.0 * spacing;
    let below = needed > ui.available_width();
    let mut reload = false;
    ui.horizontal(|ui| {
        // why: line the box up with the project boxes, which sit after a chevron and a gap.
        ui.add_space(CHEVRON + spacing);
        controls::checkbox(ui, theme, &mut scope.all_projects, copy::WATCH_ALL_PROJECTS);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            reload = controls::icon_button(ui, theme, Icon::Refresh, copy::REFRESH_LIST)
                .on_hover_text(copy::REFRESH_LIST)
                .clicked();
            if !below {
                summary_line(ui, theme, catalog, &line);
            }
        });
    });
    if below {
        ui.horizontal(|ui| {
            ui.add_space(CHEVRON + spacing);
            summary_line(ui, theme, catalog, &line);
        });
    }
    reload
}

/// The muted count of chosen repositories, or the failure (its reason on hover).
fn summary_line(ui: &mut egui::Ui, theme: &UiTheme, catalog: CatalogState<'_>, line: &str) {
    match catalog {
        CatalogState::Failed(error) => {
            widgets::error_text(ui, theme, line).on_hover_text(error.title.as_ref());
        }
        CatalogState::Loading | CatalogState::Ready(_) => {
            widgets::muted(ui, theme, line);
        }
    }
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
        if project_row(ui, theme, (project, &all), scope, searching) {
            ui.indent(("scope", project.name.as_ref()), |ui| {
                repo_rows(ui, theme, (project, &all), &shown, scope);
            });
        }
    }
}

/// A project with a chevron, a tri-state checkbox (all, some or none of its repositories)
/// and how many of them are chosen. Returns whether its repositories are shown (always while
/// searching).
fn project_row(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    (project, all): (&Project, &[&Repository]),
    scope: &mut ScopeSpec,
    searching: bool,
) -> bool {
    let position = scope.projects.iter().position(|p| p.name == project.name);
    let chosen = position.map_or(0, |i| {
        let entry = &scope.projects[i];
        if entry.all_repos {
            all.len()
        } else {
            entry.repos.len()
        }
    });
    let open_id = egui::Id::unique(("scope_open", project.name.as_ref()));
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
    searching || open
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
