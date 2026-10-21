//! The status bar: one quiet line of text segments split by hairlines. Org dots and names on the
//! left, then counts that double as filter toggles; refresh timing and the repository count on
//! the right. In a narrow window segments give way by rank, and the counts never do.

mod fit;
mod segment;

use std::{borrow::Cow, time::Duration};

use eframe::egui::{self, Sense, Stroke, vec2};
use prm_domain::{
    PrStatus, Timestamp,
    auth::SettingsSection,
    view::{StatusFilter, ViewQuery, age::age_label},
};
use prm_service::{AppState, AuthStateView, OrgHealth, OrgStatus, RefreshPhase};

use crate::{
    actions::UiAction,
    commands::CommandId,
    copy,
    format::{Countdown, countdown},
    frame::FrameCx,
    theme::{Tone, UiTheme, icons::Icon},
    widgets::{
        status_bar::{
            fit::{Rank, Slot, fit},
            segment::{Glyph, Kind, Segment},
        },
        status_pill::{status_icon, status_tone},
    },
};

const STATUSES: [(PrStatus, StatusFilter); 3] = [
    (PrStatus::NeedsReview, StatusFilter::NeedsReview),
    (PrStatus::ActiveComments, StatusFilter::ActiveComments),
    (PrStatus::Reviewed, StatusFilter::Reviewed),
];

/// An org's auth dot, in points.
const DOT: f32 = 8.0;
/// A count's status glyph, in points.
const GLYPH: f32 = 10.0;
/// The hairline between segments, in points.
const SEPARATOR_HEIGHT: f32 = 14.0;
/// Progress counts the refresh text is sized for: three digits.
const SIZING_COUNT: u32 = 100;
/// Minutes the countdown is sized for: two digits.
const SIZING_MINUTES: u64 = 10;
/// The age the updated text is sized for: two-digit minutes.
const SIZING_AGE: Duration = Duration::from_mins(10);

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
    let theme = cx.theme.clone();
    let m = &theme.tokens.metrics;
    // why: segments carry their own padding, so text sits the same distance from both ends
    // and hover fills reach the frame's inset.
    ui.spacing_mut().item_spacing.x = 0.0;
    let height = ui.available_height() - 2.0 * m.space.xs;
    let state = cx.state;
    ui.horizontal_centered(|ui| {
        let mut left = org_slots(ui, &theme, state);
        count_slots(ui, &theme, state, query, &mut left);
        let mut right = timing_slots(ui, cx, focused);
        // why: the gap keeps the two sides apart however narrow the window gets.
        let room = ui.available_width() - m.space.xl;
        fit(room, &theme, &mut left, &mut right);
        show_line(ui, cx, &theme, height, left);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            show_line(ui, cx, &theme, height, right);
        });
    });
}

/// What a shown segment does with its response.
#[derive(Copy, Clone)]
pub(super) enum Hook<'a> {
    /// Nothing beyond showing its text.
    None,
    /// Names the org and how it signs in; a click opens the org settings.
    Org(&'a OrgStatus),
    /// A click toggles that status filter.
    Status(StatusFilter),
    /// A click toggles changed-only.
    Changed,
    /// Explains the throttling.
    Slowed,
}

fn org_slots<'a>(ui: &egui::Ui, theme: &UiTheme, state: &'a AppState) -> Vec<Slot<'a>> {
    let mut slots = Vec::new();
    for org in state.orgs.iter().filter(|o| o.enabled) {
        let glyph = Glyph {
            icon: Icon::Dot,
            size: DOT,
            color: theme.colors.tone(org_tone(org)).solid,
        };
        let text = Cow::Borrowed(&*org.label);
        // why: until every name fits, the dot stands in; the name stays in its tooltip and
        // accessible name.
        let segment = Segment::new(ui, theme, Some(glyph), text, Kind::Link).with_collapsed();
        slots.push(Slot::new(theme, segment, Rank::OrgDots, Hook::Org(org)));
    }
    slots
}

fn count_slots(
    ui: &egui::Ui,
    theme: &UiTheme,
    state: &AppState,
    query: &ViewQuery,
    slots: &mut Vec<Slot<'_>>,
) {
    let summary = state.summary;
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
        let look = (status_icon(status), status_tone(status));
        let text = copy::status_count(count, status);
        let segment = toggle(ui, theme, look, (text, count, query.status == filter));
        let hook = Hook::Status(filter);
        slots.push(Slot::new(theme, segment, Rank::Counts, hook));
    }
    if summary.unread > 0 || query.unread_only {
        let text = copy::changed_count(summary.unread);
        let look = (Icon::Bar, Tone::Info);
        let segment = toggle(ui, theme, look, (text, summary.unread, query.unread_only));
        slots.push(Slot::new(theme, segment, Rank::Counts, Hook::Changed));
    }
}

/// A count that toggles its filter: the glyph in the tone's solid colour, then the label, or
/// only the number when the bar is short of room.
fn toggle<'a>(
    ui: &egui::Ui,
    theme: &UiTheme,
    (icon, tone): (Icon, Tone),
    (text, count, on): (String, u32, bool),
) -> Segment<'a> {
    let glyph = Glyph {
        icon,
        size: GLYPH,
        color: theme.colors.tone(tone).solid,
    };
    let kind = Kind::Toggle { tone, on };
    Segment::new(ui, theme, Some(glyph), text.into(), kind).with_short(ui, theme, count.to_string())
}

/// The right side in display order, right to left: refresh, updated, slowed, paused, repos.
fn timing_slots<'a>(ui: &egui::Ui, cx: &FrameCx<'_>, focused: bool) -> Vec<Slot<'a>> {
    let theme = &cx.theme;
    let refresh = &cx.state.refresh;
    let mut slots = Vec::new();
    slots.extend(refresh_slot(ui, cx, focused));
    let updated = updated_text(refresh.last_completed, cx.now).into();
    let updated = Slot::info(ui, theme, updated, Rank::Updated, Hook::None);
    let sizing = updated_text(Some(cx.now.saturating_sub(SIZING_AGE)), cx.now);
    slots.push(updated.with_room_for(ui, theme, &[copy::UPDATED_NOW, &sizing]));
    if refresh.slowed {
        let slowed = copy::SLOWED.into();
        slots.push(Slot::info(ui, theme, slowed, Rank::Slowed, Hook::Slowed));
    }
    if let Some(until) = cx.state.notifications.paused_until {
        let clock = crate::format::clock(until, cx.offset);
        let paused = copy::paused_until(&clock).into();
        slots.push(Slot::info(ui, theme, paused, Rank::Paused, Hook::None));
    }
    if let Some(repos) = repos_text(cx.state) {
        slots.push(Slot::info(ui, theme, repos.into(), Rank::Repos, Hook::None));
    }
    slots
}

/// What a running refresh is doing, or when the next one runs. It fits as its widest text in
/// that state, so its neighbours do not come and go as it ticks; so does its short form.
fn refresh_slot<'a>(ui: &egui::Ui, cx: &FrameCx<'_>, focused: bool) -> Option<Slot<'a>> {
    let theme = &cx.theme;
    if let Some((text, short)) = phase_text(cx.state) {
        let total = cx.state.refresh.progress.first().map_or(0, |p| p.total);
        let count = total.max(SIZING_COUNT);
        let listing = copy::listing_progress(count, count);
        let reading = copy::reading_progress(count, count);
        let widest = [copy::SIGNING_IN, copy::LOADING_PROJECTS, &listing, &reading];
        let (listing, reading) = (
            copy::listing_short(count, count),
            copy::reading_short(count, count),
        );
        let shortest = [copy::SIGNING_IN, copy::LOADING_PROJECTS, &listing, &reading];
        let segment = Segment::new(ui, theme, None, text.into(), Kind::Info);
        let slot = Slot::new(
            theme,
            segment.with_short(ui, theme, short),
            Rank::Refresh,
            Hook::None,
        );
        let slot = slot.with_room_for(ui, theme, &widest);
        return Some(slot.with_short_room_for(ui, theme, &shortest));
    }
    let next = cx.state.refresh.next_due.filter(|n| *n > cx.now)?;
    let text = match countdown(next.since(cx.now), focused) {
        Countdown::Minutes(m) => copy::next_in_minutes(m),
        Countdown::Seconds(s) => copy::next_in_seconds(s),
    };
    let minutes = copy::next_in_minutes(SIZING_MINUTES);
    let seconds = copy::next_in_seconds(0);
    let slot = Slot::info(ui, theme, text.into(), Rank::Refresh, Hook::None);
    Some(slot.with_room_for(ui, theme, &[&minutes, &seconds]))
}

/// Paints one side's shown slots in order, with a hairline between neighbours; collapsed org
/// dots, and compact counts, sit together as clusters.
fn show_line(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    theme: &UiTheme,
    height: f32,
    slots: Vec<Slot<'_>>,
) {
    // why: `None` before the first slot; then the cluster the slot before belongs to, if any.
    let mut previous: Option<Option<Rank>> = None;
    for slot in slots.into_iter().filter(|s| s.shown) {
        let cluster = slot.cluster();
        let joins = cluster.is_some() && previous == Some(cluster);
        if previous.is_some() && !joins {
            separator(ui, theme);
        }
        previous = Some(cluster);
        let response = slot.segment.show(ui, theme, height);
        respond(cx, slot.hook, response);
    }
}

fn respond(cx: &mut FrameCx<'_>, hook: Hook<'_>, response: egui::Response) {
    match hook {
        Hook::None => {}
        Hook::Org(org) => {
            let identity = copy::identity(&org.label, org.auth.method);
            if response.on_hover_text(identity).clicked() {
                cx.push(UiAction::OpenSettings(SettingsSection::Organizations));
            }
        }
        Hook::Status(filter) => {
            if response.clicked() {
                cx.push(UiAction::SetStatus(filter));
            }
        }
        Hook::Changed => {
            if response.on_hover_text(copy::CHANGED_TOOLTIP).clicked() {
                cx.run(CommandId::ChangedOnly);
            }
        }
        Hook::Slowed => {
            response.on_hover_text(copy::THROTTLED);
        }
    }
}

pub(super) fn separator_width(theme: &UiTheme) -> f32 {
    theme.tokens.metrics.space.sm
}

fn separator(ui: &mut egui::Ui, theme: &UiTheme) {
    let size = vec2(separator_width(theme), SEPARATOR_HEIGHT);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let stroke = Stroke::new(
        theme.tokens.metrics.stroke.hairline,
        theme.colors.border_default,
    );
    let x = ui.painter().round_to_pixel_center(rect.center().x);
    ui.painter().vline(x, rect.y_range(), stroke);
}

/// "12 repos", or how many failed; `None` before any repository is in scope.
fn repos_text(state: &AppState) -> Option<String> {
    let (ok, total, failed) = state
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
    (total > 0).then(|| {
        if failed > 0 {
            copy::repos_failed(ok, total, failed)
        } else {
            copy::repos(total)
        }
    })
}

/// What a running refresh is doing ("Reading comments 12/40"), when one is, and its short form
/// for a narrow window ("Reading 12/40").
fn phase_text(state: &AppState) -> Option<(String, String)> {
    if !state.refresh.in_progress {
        return None;
    }
    let progress = state.refresh.progress.first()?;
    let (done, total) = (progress.done, progress.total);
    Some(match progress.phase {
        RefreshPhase::Authenticating => (copy::SIGNING_IN.to_owned(), copy::SIGNING_IN.to_owned()),
        RefreshPhase::Catalog => (
            copy::LOADING_PROJECTS.to_owned(),
            copy::LOADING_PROJECTS.to_owned(),
        ),
        RefreshPhase::PullRequests | RefreshPhase::Resolving => (
            copy::listing_progress(done, total),
            copy::listing_short(done, total),
        ),
        RefreshPhase::Threads => (
            copy::reading_progress(done, total),
            copy::reading_short(done, total),
        ),
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
