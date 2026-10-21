//! The status bar: one quiet line of text segments split by hairlines. Org dots and names on the
//! left, then counts that double as filter toggles; refresh timing and the repository count on
//! the right.

use std::sync::Arc;

use eframe::egui::{self, Color32, Rect, Sense, Stroke, WidgetInfo, pos2, vec2};
use prm_domain::{
    PrStatus, Timestamp,
    auth::SettingsSection,
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
        Tone, UiTheme,
        icons::{self, Icon},
    },
    widgets::{
        controls,
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
/// The active filter's underline, in points.
const UNDERLINE: f32 = 2.0;

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
    ui.horizontal_centered(|ui| {
        let mut left = Line::new(&theme, height, 0.0);
        orgs(ui, cx, &mut left);
        counts(ui, cx, query, &mut left);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // why: in a narrow window the timing gives way to the filters, keeping a gap.
            timing(ui, cx, focused, &mut Line::new(&theme, height, m.space.xl));
        });
    });
}

fn orgs(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, line: &mut Line<'_>) {
    for org in cx.state.orgs.iter().filter(|o| o.enabled) {
        let segment = Segment {
            glyph: Some(Glyph {
                icon: Icon::Dot,
                size: DOT,
                color: line.theme.colors.tone(org_tone(org)).solid,
            }),
            text: &org.label,
            kind: Kind::Link,
        };
        let Some(response) = line.add(ui, segment) else {
            return;
        };
        let identity = copy::identity(&org.label, org.auth.method);
        if response.on_hover_text(identity).clicked() {
            cx.push(UiAction::OpenSettings(SettingsSection::Organizations));
        }
    }
}

fn counts(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, query: &ViewQuery, line: &mut Line<'_>) {
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
        let text = copy::status_count(count, status);
        let look = (status_icon(status), status_tone(status));
        let segment = Segment::toggle(line.theme, look, &text, query.status == filter);
        if line.add(ui, segment).is_some_and(|r| r.clicked()) {
            cx.push(UiAction::SetStatus(filter));
        }
    }
    if summary.unread > 0 || query.unread_only {
        let text = copy::changed_count(summary.unread);
        let look = (Icon::Bar, Tone::Info);
        let segment = Segment::toggle(line.theme, look, &text, query.unread_only);
        let response = line.add(ui, segment);
        if response.is_some_and(|r| r.on_hover_text(copy::CHANGED_TOOLTIP).clicked()) {
            cx.run(CommandId::ChangedOnly);
        }
    }
}

/// Laid out right to left: the first segment added sits at the bar's right end.
fn timing(ui: &mut egui::Ui, cx: &FrameCx<'_>, focused: bool, line: &mut Line<'_>) {
    let refresh = &cx.state.refresh;
    if let Some(text) = phase_text(cx) {
        line.text(ui, &text);
    } else if let Some(next) = refresh.next_due.filter(|n| *n > cx.now) {
        let text = match countdown(next.since(cx.now), focused) {
            Countdown::Minutes(m) => copy::next_in_minutes(m),
            Countdown::Seconds(s) => copy::next_in_seconds(s),
        };
        line.text(ui, &text);
    }
    line.text(ui, &updated_text(refresh.last_completed, cx.now));
    if refresh.slowed
        && let Some(response) = line.text(ui, copy::SLOWED)
    {
        response.on_hover_text(copy::THROTTLED);
    }
    if let Some(until) = cx.state.notifications.paused_until {
        let clock = crate::format::clock(until, cx.offset);
        line.text(ui, &copy::paused_until(&clock));
    }
    if let Some(text) = repos_text(cx) {
        line.text(ui, &text);
    }
}

/// "12 repos", or how many failed; `None` before any repository is in scope.
fn repos_text(cx: &FrameCx<'_>) -> Option<String> {
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
    (total > 0).then(|| {
        if failed > 0 {
            copy::repos_failed(ok, total, failed)
        } else {
            copy::repos(total)
        }
    })
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

/// One side of the bar: segments of equal height with a hairline between neighbours. Segments
/// are added in priority order; the first that does not fit is dropped with all after it, so
/// a narrow window never shows half a segment.
struct Line<'t> {
    theme: &'t UiTheme,
    /// The height of each segment's hover fill.
    height: f32,
    /// Room to leave free beyond the last segment, in points.
    reserve: f32,
    started: bool,
    full: bool,
}

impl<'t> Line<'t> {
    const fn new(theme: &'t UiTheme, height: f32, reserve: f32) -> Self {
        Self {
            theme,
            height,
            reserve,
            started: false,
            full: false,
        }
    }

    fn add(&mut self, ui: &mut egui::Ui, segment: Segment<'_>) -> Option<egui::Response> {
        let galley = segment.galley(ui, self.theme);
        let width = segment.width(self.theme, &galley);
        let separator = if self.started {
            self.separator_width()
        } else {
            0.0
        };
        self.full |= separator + width + self.reserve > ui.available_width();
        if self.full {
            return None;
        }
        if self.started {
            self.separator(ui);
        }
        self.started = true;
        Some(segment.show(ui, self.theme, galley, vec2(width, self.height)))
    }

    /// Plain information.
    fn text(&mut self, ui: &mut egui::Ui, text: &str) -> Option<egui::Response> {
        let segment = Segment {
            glyph: None,
            text,
            kind: Kind::Info,
        };
        self.add(ui, segment)
    }

    fn separator_width(&self) -> f32 {
        self.theme.tokens.metrics.space.sm
    }

    fn separator(&self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let size = vec2(self.separator_width(), SEPARATOR_HEIGHT);
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let stroke = Stroke::new(
            theme.tokens.metrics.stroke.hairline,
            theme.colors.border_default,
        );
        let x = ui.painter().round_to_pixel_center(rect.center().x);
        ui.painter().vline(x, rect.y_range(), stroke);
    }
}

/// A painted glyph before a segment's text.
#[derive(Copy, Clone)]
struct Glyph {
    icon: Icon,
    size: f32,
    color: Color32,
}

/// What a segment is to a screen reader and how it answers the pointer.
#[derive(Copy, Clone)]
enum Kind {
    /// Plain information.
    Info,
    /// A button that opens something.
    Link,
    /// A filter toggle; while `on`, its label and underline take the tone's foreground.
    Toggle { tone: Tone, on: bool },
}

/// One run of text in the bar, padded on both sides so hover fills have room.
struct Segment<'a> {
    glyph: Option<Glyph>,
    text: &'a str,
    kind: Kind,
}

impl<'a> Segment<'a> {
    fn toggle(theme: &UiTheme, (icon, tone): (Icon, Tone), text: &'a str, on: bool) -> Self {
        let glyph = Glyph {
            icon,
            size: GLYPH,
            color: theme.colors.tone(tone).solid,
        };
        Self {
            glyph: Some(glyph),
            text,
            kind: Kind::Toggle { tone, on },
        }
    }

    /// The text in the bar's font; its colour is picked when painted, after hit testing.
    fn galley(&self, ui: &egui::Ui, theme: &UiTheme) -> Arc<egui::Galley> {
        ui.painter().layout_no_wrap(
            self.text.to_owned(),
            theme.fonts.small.clone(),
            Color32::PLACEHOLDER,
        )
    }

    /// The glyph and text with padding on both sides, in points.
    fn width(&self, theme: &UiTheme, galley: &egui::Galley) -> f32 {
        let pad = theme.tokens.metrics.space.sm;
        let lead = self.glyph.map_or(0.0, |g| g.size + pad);
        pad + lead + galley.size().x + pad
    }

    fn show(
        self,
        ui: &mut egui::Ui,
        theme: &UiTheme,
        galley: Arc<egui::Galley>,
        size: egui::Vec2,
    ) -> egui::Response {
        let sense = match self.kind {
            Kind::Info => Sense::hover(),
            Kind::Link | Kind::Toggle { .. } => Sense::click(),
        };
        let (rect, response) = ui.allocate_exact_size(size, sense);
        if ui.is_rect_visible(rect) {
            self.paint(ui, theme, (rect, &response), galley);
        }
        let (kind, enabled, text) = (self.kind, ui.is_enabled(), self.text);
        response.widget_info(|| match kind {
            Kind::Info => WidgetInfo::labeled(egui::Role::Label, enabled, text),
            Kind::Link => WidgetInfo::labeled(egui::Role::Button, enabled, text),
            Kind::Toggle { on, .. } => WidgetInfo::selected(egui::Role::Button, enabled, on, text),
        });
        response
    }

    fn paint(
        &self,
        ui: &egui::Ui,
        theme: &UiTheme,
        (rect, response): (Rect, &egui::Response),
        galley: Arc<egui::Galley>,
    ) {
        let pad = theme.tokens.metrics.space.sm;
        if !matches!(self.kind, Kind::Info) {
            controls::ghost_fill(ui, theme, rect, response);
        }
        let painter = ui.painter();
        let mut x = rect.left() + pad;
        if let Some(glyph) = self.glyph {
            let center = pos2(x + glyph.size / 2.0, rect.center().y);
            let square = Rect::from_center_size(center, vec2(glyph.size, glyph.size));
            icons::paint(painter, square, glyph.icon, glyph.color);
            x += glyph.size + pad;
        }
        let top = rect.center().y - galley.size().y / 2.0;
        let text = Rect::from_min_size(pos2(x, top), galley.size());
        painter.galley(text.min, galley, self.text_color(theme, response));
        if let Kind::Toggle { tone, on: true } = self.kind {
            let underline = Rect::from_min_max(
                pos2(rect.left() + pad, text.bottom()),
                pos2(text.right(), text.bottom() + UNDERLINE),
            );
            painter.rect_filled(underline, 0.0, theme.colors.tone(tone).fg);
        }
    }

    fn text_color(&self, theme: &UiTheme, response: &egui::Response) -> Color32 {
        let colors = &theme.colors;
        match self.kind {
            Kind::Toggle { tone, on: true } => colors.tone(tone).fg,
            Kind::Link | Kind::Toggle { on: false, .. } if response.hovered() => {
                colors.text_primary
            }
            Kind::Info | Kind::Link | Kind::Toggle { .. } => colors.text_secondary,
        }
    }
}
