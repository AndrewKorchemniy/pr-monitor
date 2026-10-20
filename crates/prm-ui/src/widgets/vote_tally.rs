//! Vote marks for the reviewers whose votes count, in list order, with a tooltip of names.

use eframe::egui::{self, Color32, Sense, vec2};
use prm_domain::{model::VoteKind, view::ReviewerChip};

use crate::{
    copy,
    theme::{UiTheme, icons::Icon},
};

const MARK: f32 = 10.0;
const GAP: f32 = 4.0;
/// At most this many marks; the rest become "+N".
const MAX_MARKS: usize = 5;

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

pub(crate) fn vote_tally(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    reviewers: &[ReviewerChip],
) -> egui::Response {
    let shown: Vec<&ReviewerChip> = voters(reviewers).take(MAX_MARKS).collect();
    let extra = voters(reviewers).count().saturating_sub(shown.len());
    let extra_galley = (extra > 0).then(|| {
        ui.painter().layout_no_wrap(
            format!("+{extra}"),
            theme.fonts.mono.clone(),
            theme.colors.text_muted,
        )
    });
    let marks_w = shown.len() as f32 * (MARK + GAP);
    let width = marks_w + extra_galley.as_ref().map_or(0.0, |g| g.size().x);
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, ui.available_height()), Sense::hover());
    if ui.is_rect_visible(rect) {
        let mut x = rect.left();
        for reviewer in &shown {
            let mark = egui::Rect::from_min_size(
                egui::pos2(x, rect.center().y - MARK / 2.0),
                vec2(MARK, MARK),
            );
            if let Some(icon) = vote_icon(reviewer.vote) {
                crate::theme::icons::paint(
                    ui.painter(),
                    mark,
                    icon,
                    vote_color(theme, reviewer.vote),
                );
            }
            x += MARK + GAP;
        }
        if let Some(galley) = extra_galley {
            let pos = egui::pos2(x, rect.center().y - galley.size().y / 2.0);
            ui.painter().galley(pos, galley, theme.colors.text_muted);
        }
    }
    let summary = tooltip(reviewers);
    let response = if summary.is_empty() {
        response
    } else {
        response.on_hover_text(&summary)
    };
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &summary));
    response
}

/// "Ana: Approved · Ben: Waiting for author".
pub(crate) fn tooltip(reviewers: &[ReviewerChip]) -> String {
    voters(reviewers)
        .map(|r| format!("{}: {}", r.name, copy::vote_word(r.vote)))
        .collect::<Vec<_>>()
        .join(" · ")
}
