//! The dashboard: toolbar, banners, the list (a table, or cards in a narrow window) or an empty
//! state, the peek drawer and the status bar. It works from the smallest window up.

use std::{collections::BTreeSet, sync::Arc};

use eframe::egui;
use prm_config::Severity;
use prm_domain::view::age::AgeCache;
use prm_service::state::Banner;

use crate::{
    copy,
    frame::FrameCx,
    render::{MIN_SIZE, hover_bands::TableGeometry},
    theme::{UiTheme, adapter},
    ui_state::PeekLayout,
    view::{
        DashboardView, ScrollIntent,
        anchor::{AnchorResult, capture, restore},
        columns::fits_table,
        empty::{EmptyInputs, empty_kind},
    },
    widgets::{
        banner::banners,
        empty_state::empty_state,
        peek::peek,
        pr_cards::{card_pitch, pr_cards},
        pr_table::{TableInputs, pr_table, table_pitch, table_room},
        status_bar::status_bar,
        toolbar::{ToolbarInputs, toolbar},
    },
};

const PEEK_DEFAULT: f32 = 360.0;
const PEEK_RANGE: std::ops::RangeInclusive<f32> = 280.0..=640.0;
/// A docked peek never leaves the list narrower than the smallest window (points).
const LIST_MIN: f32 = MIN_SIZE[0];

/// How the dashboard lists pull requests.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum ListMode {
    Table,
    /// The window is too narrow for a table worth showing (`columns::fits_table`).
    Cards,
}

/// Where the peek drawer goes this frame.
#[derive(Copy, Clone, Debug, PartialEq)]
enum PeekPlace {
    Closed,
    /// At the right, `width` points wide and at most `max`.
    Docked {
        width: f32,
        max: f32,
    },
    /// Over the list at full width, since docking would squeeze the list.
    Over,
}

impl PeekPlace {
    /// The drawer in a dashboard `width` points wide. With nothing selected it has nothing to
    /// show, so then it never covers the list.
    fn of(width: f32, peek: PeekLayout, view: &DashboardView) -> Self {
        let (least, most) = (*PEEK_RANGE.start(), *PEEK_RANGE.end());
        let max = (width - LIST_MIN).min(most);
        if !peek.open {
            Self::Closed
        } else if max >= least {
            Self::Docked {
                width: peek.width.unwrap_or(PEEK_DEFAULT).clamp(least, max),
                max,
            }
        } else if view.selected_row().is_some() {
            Self::Over
        } else {
            Self::Closed
        }
    }
}

pub(crate) struct DashboardUi {
    pub view: DashboardView,
    /// The filter field's text (the view's query follows it).
    pub filter: String,
    pub peek: PeekLayout,
    pub banners_expanded: bool,
    pub dismissed: BTreeSet<Arc<str>>,
    /// Shown first once the background service has stopped (it cannot recover); first run
    /// shows it too.
    pub service_stopped: Option<Banner>,
    pub ages: AgeCache,
    pub filter_focused: bool,
    pub geometry: Option<TableGeometry>,
    /// How the list was drawn last, so a switch keeps the user's place.
    pub mode: Option<ListMode>,
}

impl DashboardUi {
    pub(crate) fn new(view: DashboardView, peek: PeekLayout) -> Self {
        Self {
            filter: view.query.text.clone(),
            view,
            peek,
            banners_expanded: false,
            dismissed: BTreeSet::new(),
            service_stopped: None,
            ages: AgeCache::new(),
            filter_focused: false,
            geometry: None,
            mode: None,
        }
    }

    /// The background service stopped and cannot recover: a banner says so until a restart.
    pub(crate) fn show_service_stopped(&mut self) {
        self.service_stopped.get_or_insert_with(|| Banner {
            id: Arc::from("service-stopped"),
            severity: Severity::Error,
            org: None,
            title: Arc::from(copy::SERVICE_STOPPED),
            detail: None,
            action: None,
            secondary: None,
            dismissible: false,
        });
    }
}

/// The table, or cards when a list `width` points wide has no room for a table worth showing.
fn list_mode(ui: &egui::Ui, theme: &UiTheme, width: f32) -> ListMode {
    let frame = adapter::frame_table(theme).total_margin().sum().x;
    if fits_table(table_room(ui.spacing(), theme, width - frame)) {
        ListMode::Table
    } else {
        ListMode::Cards
    }
}

pub(crate) fn dashboard(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    d: &mut DashboardUi,
    window_focused: bool,
) {
    let theme = cx.theme.clone();
    let width = ui.available_rect_before_wrap().width();
    let place = PeekPlace::of(width, d.peek, &d.view);
    let list_width = match place {
        PeekPlace::Docked { width: peek, .. } => width - peek,
        PeekPlace::Closed | PeekPlace::Over => width,
    };
    let mode = list_mode(ui, &theme, list_width);
    let inputs = ToolbarInputs {
        group: d.view.group,
        sort: (mode == ListMode::Cards).then_some(d.view.sort),
        show_org: cx.state.show_org_column,
    };
    egui::Panel::top("toolbar")
        .exact_size(theme.toolbar_height())
        .frame(adapter::frame_chrome(&theme))
        .show(ui, |ui| {
            d.filter_focused = toolbar(ui, cx, &mut d.filter, inputs);
        });
    banner_panel(ui, cx, d);
    egui::Panel::bottom("status_bar")
        .exact_size(theme.tokens.metrics.row.statusbar_height)
        .frame(adapter::frame_chrome(&theme))
        .show(ui, |ui| status_bar(ui, cx, &d.view.query, window_focused));
    if let PeekPlace::Docked { width, max } = place {
        docked_peek(ui, cx, d, (width, max));
    }
    if place == PeekPlace::Over {
        d.geometry = None;
        egui::CentralPanel::no_frame()
            .frame(adapter::frame_raised(&theme))
            .show(ui, |ui| peek(ui, cx, d.view.selected_row()));
        return;
    }
    egui::CentralPanel::no_frame()
        .frame(adapter::frame_table(&theme))
        .show(ui, |ui| body(ui, cx, d, (mode, window_focused)));
}

/// The banners nobody dismissed, the stopped service first.
fn banner_panel(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, d: &mut DashboardUi) {
    let published = cx.state.banners.iter();
    let visible: Vec<_> = d
        .service_stopped
        .iter()
        .chain(published.filter(|b| !d.dismissed.contains(&b.id)))
        .collect();
    if !visible.is_empty() {
        egui::Panel::top("banners")
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 6)))
            .show(ui, |ui| banners(ui, cx, &visible, &mut d.banners_expanded));
    }
}

/// The peek at the right, resizable up to `max` points, remembering the width it was left at.
fn docked_peek(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    d: &mut DashboardUi,
    (width, max): (f32, f32),
) {
    let theme = cx.theme.clone();
    let panel = egui::Panel::right("peek")
        .default_size(width)
        .size_range(*PEEK_RANGE.start()..=max)
        .resizable(true)
        .frame(adapter::frame_raised(&theme))
        .show(ui, |ui| peek(ui, cx, d.view.selected_row()));
    let shown = panel.response.rect.width().round();
    if d.peek.width != Some(shown) {
        d.peek.width = Some(shown);
    }
}

fn body(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    d: &mut DashboardUi,
    (mode, window_focused): (ListMode, bool),
) {
    d.ages
        .update(d.view.rows(), d.view.rows_generation(), cx.now);
    let empty = empty_kind(&EmptyInputs {
        state: cx.state,
        visible: d.view.model.indices.len(),
        text: &d.view.query.text,
        status: d.view.query.status,
        unread_only: d.view.query.unread_only,
    });
    if let Some(kind) = empty {
        d.geometry = None;
        let scope = scope_label(cx);
        empty_state(ui, cx, &kind, &scope);
        return;
    }
    let first = d.mode.is_none();
    keep_place(ui, &cx.theme, d, mode);
    let request = d.view.scroll;
    let input = TableInputs {
        ages: &d.ages,
        show_org: cx.state.show_org_column,
        focused: window_focused,
    };
    let out = match mode {
        ListMode::Table => pr_table(ui, cx, &mut d.view, &input),
        ListMode::Cards => pr_cards(ui, cx, &mut d.view, &input),
    };
    d.geometry = out.geometry;
    if first && request != ScrollIntent::None && d.view.scroll == ScrollIntent::None {
        // why: on the first frame the panels around the list are not measured yet, so a
        // scroll to the saved selection stops short; it runs again once they are.
        d.view.scroll = request;
        ui.ctx().request_repaint();
    }
}

/// Switching between the table and the cards keeps the selection (or the top row) where it was
/// on screen, though the rows change height.
fn keep_place(ui: &egui::Ui, theme: &UiTheme, d: &mut DashboardUi, mode: ListMode) {
    let switched = d.mode.replace(mode).is_some_and(|last| last != mode);
    if !switched || d.view.scroll != ScrollIntent::None {
        return;
    }
    let view = &d.view;
    let place = (view.top_visible, view.offset_y);
    let Some(anchor) = capture(&view.display, view.selected.as_ref(), place, view.pitch) else {
        return;
    };
    let pitch = match mode {
        ListMode::Table => table_pitch(ui.spacing(), theme),
        ListMode::Cards => card_pitch(ui, theme),
    };
    if let AnchorResult::Offset(offset) = restore(&anchor, &view.display, pitch) {
        d.view.scroll = ScrollIntent::Offset(offset);
    }
}

/// "contoso" or "3 organizations", for empty-state copy.
fn scope_label(cx: &FrameCx<'_>) -> String {
    let enabled: Vec<_> = cx.state.orgs.iter().filter(|o| o.enabled).collect();
    match enabled.as_slice() {
        [one] => one.label.to_string(),
        many => copy::organizations(many.len()),
    }
}
