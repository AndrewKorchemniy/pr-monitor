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
        columns::table_minimum,
        empty::{EmptyInputs, empty_kind},
    },
    widgets::{
        banner::banners,
        empty_state::empty_state,
        peek::peek,
        pr_cards::{card_pitch, pr_cards},
        pr_table::{TableInputs, header_pitch, pr_table, table_pitch, table_room},
        status_bar::status_bar,
        toolbar::{ToolbarInputs, toolbar},
    },
};

const PEEK_DEFAULT: f32 = 360.0;
const PEEK_RANGE: std::ops::RangeInclusive<f32> = 280.0..=640.0;
/// The docked peek's panel.
const PEEK_PANEL: &str = "peek";
/// A docked peek never leaves the list narrower than the smallest window (points).
const LIST_MIN: f32 = MIN_SIZE[0];
/// The banners take at most this share of the height under the toolbar; more of them scroll,
/// so the list always keeps room.
const BANNERS_SHARE: f32 = 0.4;

/// How the dashboard lists pull requests.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum ListMode {
    Table,
    /// The window is too narrow for a table worth showing (`table_breakpoint`).
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

/// The narrowest list that shows the table rather than cards (points): the room a table worth
/// showing needs (`columns::table_minimum`), the scroll bar's lane and the list's frame.
pub(crate) fn table_breakpoint(spacing: &egui::Spacing, theme: &UiTheme) -> f32 {
    let frame = adapter::frame_table(theme).total_margin().sum().x;
    // why: `table_room` takes the scroll bar's lane out of the width it is given, so the room
    // in a list 0 pt wide is minus the lane.
    let room = table_room(spacing, theme, 0.0);
    table_minimum(room) - room.width + frame
}

/// The table, or cards when a list `width` points wide has no room for a table worth showing.
fn list_mode(ui: &egui::Ui, theme: &UiTheme, width: f32) -> ListMode {
    if width >= table_breakpoint(ui.spacing(), theme) {
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
    let stopped = d.service_stopped.is_some();
    let inputs = ToolbarInputs {
        group: d.view.group,
        sort: (mode == ListMode::Cards).then_some(d.view.sort),
        show_org: cx.state.show_org_column,
        stopped,
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
        .show(ui, |ui| {
            let counts = (&d.view.query, d.view.bar_counts);
            status_bar(ui, cx, counts, window_focused, stopped);
        });
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

/// The banners nobody dismissed, the stopped service first, scrolling when they would take more
/// than their share of the height (`BANNERS_SHARE`).
fn banner_panel(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, d: &mut DashboardUi) {
    let published = cx.state.banners.iter();
    let visible: Vec<_> = d
        .service_stopped
        .iter()
        .chain(published.filter(|b| !d.dismissed.contains(&b.id)))
        .collect();
    if visible.is_empty() {
        return;
    }
    let most = ui.available_height() * BANNERS_SHARE;
    egui::Panel::top("banners")
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 6)))
        .show(ui, |ui| {
            scrolled_banners(ui, cx, (&visible, &mut d.banners_expanded), most);
        });
}

/// `visible` banners in a scroll area as tall as they are, up to `most` points, cut at the
/// end of the last whole banner that fits so no line of text is sliced.
fn scrolled_banners(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    (visible, expanded): (&[&Banner], &mut bool),
    most: f32,
) {
    let ends_id = egui::Id::unique("banner_ends");
    let ends: Vec<f32> = ui.data(|d| d.get_temp(ends_id)).unwrap_or_default();
    let height = ends
        .iter()
        .copied()
        .take_while(|end| *end <= most)
        .last()
        .filter(|_| ends.last().is_some_and(|last| *last > most))
        .unwrap_or(most);
    // why: a panel lays its content out in the height it took last frame, so a scroll area
    // sized from the panel would never grow; it gets room of its own.
    let room = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), height));
    ui.scope_builder(egui::UiBuilder::new().max_rect(room), |ui| {
        // why: the bar floats over the banners' own right inset, so both insets match.
        ui.spacing_mut().scroll.floating_allocated_width = 0.0;
        let shown = egui::ScrollArea::vertical()
            .id_salt("banners")
            .auto_shrink([false, true])
            .max_height(height)
            .show(ui, |ui| banners(ui, cx, visible, expanded))
            .inner;
        if shown != ends {
            ui.data_mut(|d| d.insert_temp(ends_id, shown));
            ui.ctx().request_repaint();
        }
    });
}

/// The peek at the right, `width` points wide (the saved width, at most `max`), resizable up to
/// `max`. Only a drag saves a new width, so a narrow window never overwrites the saved one.
fn docked_peek(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    d: &mut DashboardUi,
    (width, max): (f32, f32),
) {
    let theme = cx.theme.clone();
    // why: egui keeps the width the panel last had, cut to fit while the window was narrower,
    // and prefers it to `default_size`; forgetting it lets the peek grow back with the window.
    let id = ui.scope_id().with_salt(egui::IdSalt::new(PEEK_PANEL));
    ui.ctx()
        .data_mut(|data| data.remove::<egui::PanelState>(id));
    let panel = egui::Panel::right(PEEK_PANEL)
        .default_size(width)
        .size_range(*PEEK_RANGE.start()..=max)
        .resizable(true)
        .frame(adapter::frame_raised(&theme))
        .show(ui, |ui| peek(ui, cx, d.view.selected_row()));
    let shown = panel.response.rect.width();
    if (shown - width).abs() > 1.0 {
        d.peek.width = Some(shown.round());
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
    // why: egui_extras fills selected rows with the text-selection colour, which is too heavy
    // for the muted and tone text on a row; the list uses the row tokens the theme guards.
    let c = &cx.theme.colors;
    ui.visuals_mut().selection.bg_fill = if window_focused {
        c.row_selected
    } else {
        c.row_selected_inactive
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
/// on screen, though the rows change height, and a selection in view stays wholly in view.
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
    // why: the table's rows start under its header, the cards' at the top of the list.
    let header = header_pitch(ui.spacing(), theme);
    let (pitch, shift, room) = match mode {
        ListMode::Table => (
            table_pitch(ui.spacing(), theme),
            header,
            ui.available_height() - header,
        ),
        ListMode::Cards => (card_pitch(ui, theme), -header, ui.available_height()),
    };
    let AnchorResult::Offset(offset) = restore(&anchor, &view.display, pitch) else {
        return;
    };
    let mut offset = offset + shift;
    let (rows, spacing) = (&view.display.rows, ui.spacing().item_spacing.y);
    let was_shown = (0.0..room).contains(&(anchor.top - anchor.offset_y));
    if view.selected.as_ref() == Some(&anchor.key) && was_shown {
        // why: a card is taller than the row it replaces, so a selection near the bottom
        // would run out of the list; it moves up just enough to stay wholly in view.
        let top = pitch.top_of(rows, anchor.display_index);
        offset = offset.max(top + pitch.pr - spacing - room).min(top);
    }
    // why: an offset past the end is not cut short but springs back over several frames.
    let end = (pitch.top_of(rows, rows.len()) - spacing - room).max(0.0);
    d.view.scroll = ScrollIntent::Offset(offset.clamp(0.0, end));
}

/// "contoso" or "3 organizations", for empty-state copy.
fn scope_label(cx: &FrameCx<'_>) -> String {
    let enabled: Vec<_> = cx.state.orgs.iter().filter(|o| o.enabled).collect();
    match enabled.as_slice() {
        [one] => one.label.to_string(),
        many => copy::organizations(many.len()),
    }
}
