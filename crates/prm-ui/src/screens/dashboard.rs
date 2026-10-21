//! The dashboard: toolbar, banners, the table (or an empty state), the peek drawer and the
//! status bar.

use std::{collections::BTreeSet, sync::Arc};

use eframe::egui;
use prm_config::Severity;
use prm_domain::view::age::AgeCache;
use prm_service::state::Banner;

use crate::{
    copy,
    frame::FrameCx,
    render::hover_bands::TableGeometry,
    theme::adapter,
    ui_state::PeekLayout,
    view::{
        DashboardView,
        empty::{EmptyInputs, empty_kind},
    },
    widgets::{
        banner::banners,
        empty_state::empty_state,
        peek::peek,
        pr_table::{TableInputs, pr_table},
        status_bar::status_bar,
        toolbar::toolbar,
    },
};

const PEEK_DEFAULT: f32 = 360.0;
const PEEK_RANGE: std::ops::RangeInclusive<f32> = 280.0..=640.0;

pub(crate) struct DashboardUi {
    pub view: DashboardView,
    /// The filter field's text (the view's query follows it).
    pub filter: String,
    pub peek: PeekLayout,
    pub banners_expanded: bool,
    pub dismissed: BTreeSet<Arc<str>>,
    /// Shown first once the background service has stopped (it cannot recover).
    pub service_stopped: Option<Banner>,
    pub ages: AgeCache,
    pub filter_focused: bool,
    pub geometry: Option<TableGeometry>,
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

pub(crate) fn dashboard(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    d: &mut DashboardUi,
    window_focused: bool,
) {
    let theme = cx.theme.clone();
    let row = &theme.tokens.metrics.row;
    egui::Panel::top("toolbar")
        .exact_size(theme.toolbar_height())
        .frame(adapter::frame_chrome(&theme))
        .show(ui, |ui| {
            d.filter_focused = toolbar(ui, cx, &mut d.filter, d.view.group);
        });
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
    egui::Panel::bottom("status_bar")
        .exact_size(row.statusbar_height)
        .frame(adapter::frame_chrome(&theme))
        .show(ui, |ui| status_bar(ui, cx, &d.view.query, window_focused));
    if d.peek.open {
        let panel = egui::Panel::right("peek")
            .default_size(d.peek.width.unwrap_or(PEEK_DEFAULT))
            .size_range(PEEK_RANGE)
            .resizable(true)
            .frame(adapter::frame_raised(&theme))
            .show(ui, |ui| peek(ui, cx, d.view.selected_row()));
        let width = panel.response.rect.width().round();
        if d.peek.width != Some(width) {
            d.peek.width = Some(width);
        }
    }
    egui::CentralPanel::no_frame()
        .frame(adapter::frame_table(&theme))
        .show(ui, |ui| body(ui, cx, d, window_focused));
}

fn body(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, d: &mut DashboardUi, window_focused: bool) {
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
    let input = TableInputs {
        ages: &d.ages,
        show_org: cx.state.show_org_column,
        focused: window_focused,
    };
    d.geometry = pr_table(ui, cx, &mut d.view, &input).geometry;
}

/// "contoso" or "3 organizations", for empty-state copy.
fn scope_label(cx: &FrameCx<'_>) -> String {
    let enabled: Vec<_> = cx.state.orgs.iter().filter(|o| o.enabled).collect();
    match enabled.as_slice() {
        [one] => one.label.to_string(),
        many => copy::organizations(many.len()),
    }
}
