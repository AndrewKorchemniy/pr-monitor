//! Vote marks for the reviewers whose votes count, in list order, with a tooltip of names.

use eframe::egui::{self, Color32, Sense, vec2};
use prm_domain::{model::VoteKind, view::ReviewerChip};

use crate::{
    avatars::AvatarStore,
    copy,
    theme::{UiTheme, icons::Icon},
    widgets::avatar::{AvatarFace, paint_avatar},
};

/// A reviewer's avatar (points).
const AVATAR: f32 = 16.0;
/// From one avatar's left edge to the next.
const STEP: f32 = 20.0;
/// The vote glyph on an avatar's corner, and the disc behind it.
const BADGE: f32 = 7.0;
const BADGE_DISC: f32 = 9.0;
/// At most this many marks; the rest become "+N".
const MAX_MARKS: usize = 4;

pub(crate) const fn vote_icon(vote: VoteKind) -> Option<Icon> {
    match vote {
        VoteKind::Approved => Some(Icon::Check),
        VoteKind::ApprovedWithSuggestions => Some(Icon::CheckDot),
        VoteKind::WaitingForAuthor => Some(Icon::Pause),
        VoteKind::Rejected => Some(Icon::Cross),
        VoteKind::NoVote | VoteKind::NotApplicable | VoteKind::Other => None,
    }
}

pub(crate) const fn vote_color(theme: &UiTheme, vote: VoteKind) -> Color32 {
    match vote {
        VoteKind::Approved => theme.colors.vote_approved,
        VoteKind::ApprovedWithSuggestions => theme.colors.vote_suggestions,
        VoteKind::WaitingForAuthor => theme.colors.vote_waiting,
        VoteKind::Rejected => theme.colors.vote_rejected,
        VoteKind::NoVote | VoteKind::NotApplicable | VoteKind::Other => theme.colors.vote_none,
    }
}

/// Listed reviewers who voted.
fn voters(reviewers: &[ReviewerChip]) -> impl Iterator<Item = &ReviewerChip> {
    reviewers
        .iter()
        .filter(|r| r.listed && vote_icon(r.vote).is_some())
}

/// The avatar of each reviewer who voted, with their vote on its corner; the rest as "+N".
pub(crate) fn vote_tally(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    avatars: &AvatarStore,
    reviewers: &[ReviewerChip],
) -> egui::Response {
    let mut voted = voters(reviewers);
    let response = marks(ui, theme, voters(reviewers).count(), |ui, face_rect| {
        if let Some(reviewer) = voted.next() {
            paint_avatar(ui, theme, avatars, &AvatarFace::of(reviewer), face_rect);
            paint_vote_badge(ui.painter(), theme, face_rect, reviewer.vote);
        }
    });
    describe(response, &tooltip(reviewers))
}

/// `total` avatar-sized marks in a row, as many as fit and at most `MAX_MARKS`, the rest as
/// "+N". `paint` draws the next mark into the rect it is given, in order.
pub(crate) fn marks(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    total: usize,
    mut paint: impl FnMut(&egui::Ui, egui::Rect),
) -> egui::Response {
    let extra_galley = |extra: usize| {
        ui.painter().layout_no_wrap(
            format!("+{extra}"),
            theme.fonts.mono.clone(),
            theme.colors.text_muted,
        )
    };
    let room = ui.available_width();
    let mut shown = total.min(MAX_MARKS).min(marks_in(room).max(1));
    let mut galley = None;
    if shown < total {
        let g = extra_galley(total - shown);
        shown = shown.min(marks_in(room - g.size().x).max(1));
        galley = Some(extra_galley(total - shown));
    }
    let width = shown as f32 * STEP + galley.as_ref().map_or(0.0, |g| g.size().x);
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, ui.available_height()), Sense::hover());
    if ui.is_rect_visible(rect) {
        let mut x = rect.left();
        for _ in 0..shown {
            let at = egui::pos2(x, rect.center().y - AVATAR / 2.0);
            paint(ui, egui::Rect::from_min_size(at, vec2(AVATAR, AVATAR)));
            x += STEP;
        }
        if let Some(galley) = galley {
            let pos = egui::pos2(x, rect.center().y - galley.size().y / 2.0);
            ui.painter().galley(pos, galley, theme.colors.text_muted);
        }
    }
    response
}

/// Explains `response` on hover and to screen readers; nothing when `summary` is empty.
pub(crate) fn describe(response: egui::Response, summary: &str) -> egui::Response {
    let response = if summary.is_empty() {
        response
    } else {
        response.on_hover_text(summary)
    };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, summary));
    response
}

/// How many marks fit in `room` points.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a small, non-negative count of marks"
)]
fn marks_in(room: f32) -> usize {
    (room.max(0.0) / STEP) as usize
}

/// The vote's glyph on the bottom-right corner of an avatar's `rect`, on a disc in the canvas
/// colour so it stays readable over any picture. A vote with no glyph (no review yet) is an
/// empty ring in its place.
pub(crate) fn paint_vote_badge(
    painter: &egui::Painter,
    theme: &UiTheme,
    rect: egui::Rect,
    vote: VoteKind,
) {
    let center = badge_center(rect);
    painter.circle_filled(center, BADGE_DISC / 2.0, theme.colors.surface_canvas);
    if let Some(icon) = vote_icon(vote) {
        let glyph = egui::Rect::from_center_size(center, vec2(BADGE, BADGE));
        crate::theme::icons::paint(painter, glyph, icon, vote_color(theme, vote));
    } else {
        let ring = egui::Stroke::new(1.0, theme.colors.vote_none);
        painter.circle_stroke(center, BADGE_DISC / 2.0 - ring.width / 2.0, ring);
    }
}

/// Where the badge sits: the bottom-right corner of an avatar's `rect`.
fn badge_center(rect: egui::Rect) -> egui::Pos2 {
    rect.right_bottom() - vec2(BADGE_DISC, BADGE_DISC) / 2.0 + vec2(2.0, 2.0)
}

/// "Ana: Approved · Ben: Waiting for author".
fn tooltip(reviewers: &[ReviewerChip]) -> String {
    voters(reviewers)
        .map(|r| format!("{}: {}", r.name, copy::vote_word(r.vote)))
        .collect::<Vec<_>>()
        .join(" · ")
}
