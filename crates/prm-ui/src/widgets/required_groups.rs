//! The Required column: each required reviewer group as an avatar with its vote on the corner,
//! or an empty ring while it has not reviewed. Names are never in the cell; the tooltip and the
//! accessible name carry them.

use eframe::egui;
use prm_domain::view::GroupChip;

use crate::{
    avatars::AvatarStore,
    copy,
    theme::UiTheme,
    widgets::{
        avatar::{AvatarFace, paint_avatar},
        vote_tally::{describe, marks, paint_pending_badge, paint_vote_badge, vote_icon},
    },
};

/// One mark per required group; blank when the pull request has none.
pub(crate) fn required_groups(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    avatars: &AvatarStore,
    groups: &[GroupChip],
) {
    if groups.is_empty() {
        return;
    }
    let mut next = groups.iter();
    let response = marks(ui, theme, groups.len(), |ui, face_rect| {
        if let Some(group) = next.next() {
            paint_group(ui, theme, avatars, group, face_rect);
        }
    });
    describe(response, &tooltip(groups));
}

/// A group's avatar into `rect`, with its vote or the empty ring on the corner.
pub(crate) fn paint_group(
    ui: &egui::Ui,
    theme: &UiTheme,
    avatars: &AvatarStore,
    group: &GroupChip,
    rect: egui::Rect,
) {
    paint_avatar(ui, theme, avatars, &AvatarFace::of_group(group), rect);
    if vote_icon(group.vote).is_some() {
        paint_vote_badge(ui.painter(), theme, rect, group.vote);
    } else {
        paint_pending_badge(ui.painter(), theme, rect);
    }
}

/// "IPCO Repo Approver: approved via Kevin Haro", one line per group.
fn tooltip(groups: &[GroupChip]) -> String {
    groups
        .iter()
        .map(copy::group_status)
        .collect::<Vec<_>>()
        .join("\n")
}
