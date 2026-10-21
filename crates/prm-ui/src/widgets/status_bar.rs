//! The status bar: counts that double as filters, per-org auth dots, when data was updated,
//! the next refresh, and the repository count.

use eframe::egui::{self, RichText};
use prm_domain::{
    PrStatus, Timestamp,
    view::{StatusFilter, ViewQuery, age::age_label},
};
use prm_service::{AuthStateView, OrgHealth, OrgStatus, RefreshPhase};

use crate::{
    actions::UiAction,
    commands::CommandId,
    copy,
    format::{Countdown, countdown},
    frame::FrameCx,
    theme::{
        Tone,
        icons::{self, Icon},
    },
    widgets::{
        chip::Chip,
        status_pill::{status_icon, status_tone},
    },
};

const STATUSES: [(PrStatus, StatusFilter); 3] = [
    (PrStatus::NeedsReview, StatusFilter::NeedsReview),
    (PrStatus::ActiveComments, StatusFilter::ActiveComments),
    (PrStatus::Reviewed, StatusFilter::Reviewed),
];

/// The tone of an org's dot: ok, working, or needs attention.
pub(crate) fn org_tone(org: &OrgStatus) -> Tone {
    match (&org.auth.state, &org.health) {
        (
            AuthStateView::NeedsSignIn { .. }
            | AuthStateView::Blocked { .. }
            | AuthStateView::SignedOut,
            _,
        )
        | (_, OrgHealth::Failed { .. }) => Tone::Danger,
        (_, OrgHealth::Degraded { .. } | OrgHealth::Paused { .. }) => Tone::Warning,
        (AuthStateView::Checking | AuthStateView::SigningIn { .. } | AuthStateView::Unknown, _)
        | (_, OrgHealth::Refreshing { .. }) => Tone::Info,
        (
            AuthStateView::SignedIn | AuthStateView::Disabled,
            OrgHealth::Idle | OrgHealth::Ok | OrgHealth::Disabled,
        ) => Tone::Success,
    }
}

pub(crate) fn status_bar(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    query: &ViewQuery,
    focused: bool,
) {
    ui.horizontal_centered(|ui| {
        orgs(ui, cx);
        ui.add_space(8.0);
        counts(ui, cx, query);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            timing(ui, cx, focused);
        });
    });
}

fn orgs(ui: &mut egui::Ui, cx: &mut FrameCx<'_>) {
    let theme = cx.theme.clone();
    for org in cx.state.orgs.iter().filter(|o| o.enabled) {
        let tone = theme.colors.tone(org_tone(org));
        let response = ui
            .horizontal(|ui| {
                icons::show(ui, Icon::Dot, 8.0, tone.solid);
                ui.label(RichText::new(org.label.as_ref()).font(theme.fonts.label.clone()));
            })
            .response;
        let identity = copy::identity(&org.label, org.auth.method);
        let response = response
            .interact(egui::Sense::click())
            .on_hover_text(identity);
        if response.clicked() {
            cx.push(UiAction::OpenSettings(
                prm_domain::auth::SettingsSection::Organizations,
            ));
        }
    }
}

fn counts(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, query: &ViewQuery) {
    let theme = cx.theme.clone();
    let summary = cx.state.summary;
    let total = summary.needs_review + summary.active_comments + summary.reviewed;
    // why: with nothing tracked, zero-count filters only compete with the empty state.
    if total == 0 && query.status == StatusFilter::default() && !query.unread_only {
        return;
    }
    for (status, filter) in STATUSES {
        let count = match status {
            PrStatus::NeedsReview => summary.needs_review,
            PrStatus::ActiveComments => summary.active_comments,
            PrStatus::Reviewed => summary.reviewed,
        };
        let text = copy::status_chip(count, status);
        let chip = Chip {
            icon: Some(status_icon(status)),
            text: &text,
            tone: status_tone(status),
            selected: query.status == filter,
        };
        if chip.show(ui, &theme).clicked() {
            cx.push(UiAction::SetStatus(filter));
        }
    }
    if summary.unread > 0 || query.unread_only {
        let text = copy::changed_chip(summary.unread);
        let chip = Chip {
            icon: Some(Icon::Bar),
            text: &text,
            tone: Tone::Info,
            selected: query.unread_only,
        };
        if chip
            .show(ui, &theme)
            .on_hover_text(copy::CHANGED_TOOLTIP)
            .clicked()
        {
            cx.run(CommandId::ChangedOnly);
        }
    }
}

fn timing(ui: &mut egui::Ui, cx: &FrameCx<'_>, focused: bool) {
    let theme = &cx.theme;
    let small = |text: String| {
        RichText::new(text)
            .font(theme.fonts.small.clone())
            .color(theme.colors.text_secondary)
    };
    let refresh = &cx.state.refresh;
    if let Some(text) = phase_text(cx) {
        ui.label(small(text));
    } else if let Some(next) = refresh.next_due.filter(|n| *n > cx.now) {
        let text = match countdown(next.since(cx.now), focused) {
            Countdown::Minutes(m) => copy::next_in_minutes(m),
            Countdown::Seconds(s) => copy::next_in_seconds(s),
        };
        ui.label(small(text));
    }
    ui.label(small(updated_text(refresh.last_completed, cx.now)));
    if refresh.slowed {
        ui.label(small(copy::SLOWED.to_owned()))
            .on_hover_text(copy::THROTTLED);
    }
    if let Some(until) = cx.state.notifications.paused_until {
        ui.label(small(copy::paused_until(&crate::format::clock(
            until, cx.offset,
        ))));
    }
    let (ok, total, failed) =
        cx.state
            .orgs
            .iter()
            .filter(|o| o.enabled)
            .fold((0, 0, 0), |acc, o| {
                let s = &o.scope;
                (
                    acc.0 + s.repos_in_scope - s.repos_failed.min(s.repos_in_scope),
                    acc.1 + s.repos_in_scope,
                    acc.2 + s.repos_failed,
                )
            });
    if total > 0 {
        let text = if failed > 0 {
            copy::repos_failed(ok, total, failed)
        } else {
            copy::repos(total)
        };
        ui.label(small(text));
    }
}

/// What a running refresh is doing ("Reading comments 12/40"), when one is.
fn phase_text(cx: &FrameCx<'_>) -> Option<String> {
    if !cx.state.refresh.in_progress {
        return None;
    }
    let progress = cx.state.refresh.progress.first()?;
    Some(match progress.phase {
        RefreshPhase::Authenticating => copy::SIGNING_IN.to_owned(),
        RefreshPhase::Catalog => copy::LOADING_PROJECTS.to_owned(),
        RefreshPhase::PullRequests | RefreshPhase::Resolving => {
            copy::listing_progress(progress.done, progress.total)
        }
        RefreshPhase::Threads => copy::reading_progress(progress.done, progress.total),
    })
}

/// "Updated 2m ago", "Updated just now" or "Not updated yet".
pub(crate) fn updated_text(last: Option<Timestamp>, now: Timestamp) -> String {
    match last {
        None => copy::NOT_UPDATED.to_owned(),
        Some(at) if now.since(at).as_secs() < 60 => copy::UPDATED_NOW.to_owned(),
        Some(at) => copy::updated_ago(&age_label(now, at).text),
    }
}
