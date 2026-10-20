//! Counts for the status bar and tray, and the tray badge and tooltip.

use crate::{
    app::APP_DISPLAY_NAME, classify::PrStatus, view::row::PrRow, view::text::truncate_utf16,
};

/// The Windows tray tooltip holds 128 UTF-16 units including the terminator.
pub const MAX_TOOLTIP_UTF16: usize = 127;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct StatusSummary {
    pub total: u32,
    pub needs_review: u32,
    pub active_comments: u32,
    pub reviewed: u32,
    pub drafts: u32,
    pub unread: u32,
    pub stale: u32,
}

impl StatusSummary {
    pub const fn count(&self, status: PrStatus) -> u32 {
        match status {
            PrStatus::NeedsReview => self.needs_review,
            PrStatus::ActiveComments => self.active_comments,
            PrStatus::Reviewed => self.reviewed,
        }
    }

    /// PRs that want the user's attention: needing review, with open comments, or changed.
    pub const fn attention(&self) -> u32 {
        self.needs_review + self.active_comments + self.unread
    }
}

pub fn summarize<'a>(rows: impl IntoIterator<Item = &'a PrRow>) -> StatusSummary {
    let mut summary = StatusSummary::default();
    for row in rows {
        summary.total += 1;
        let slot = match row.status {
            PrStatus::NeedsReview => &mut summary.needs_review,
            PrStatus::ActiveComments => &mut summary.active_comments,
            PrStatus::Reviewed => &mut summary.reviewed,
        };
        *slot += 1;
        summary.drafts += u32::from(row.draft);
        summary.unread += u32::from(row.unread);
        summary.stale += u32::from(row.stale);
    }
    summary
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum TrayBadge {
    #[default]
    Idle,
    Attention,
    Syncing,
    Error,
}

/// Error beats syncing (only while nothing is shown yet) beats attention beats idle.
pub fn tray_badge(summary: &StatusSummary, any_org_broken: bool, refreshing: bool) -> TrayBadge {
    if any_org_broken {
        TrayBadge::Error
    } else if refreshing && summary.total == 0 {
        TrayBadge::Syncing
    } else if summary.attention() > 0 {
        TrayBadge::Attention
    } else {
        TrayBadge::Idle
    }
}

/// "PR Monitor · 3 need review · 2 active comments", plus " · sign-in needed", cut to fit.
pub fn tray_tooltip(summary: &StatusSummary, any_org_broken: bool) -> String {
    let mut parts = vec![APP_DISPLAY_NAME.to_owned()];
    if summary.needs_review > 0 {
        parts.push(format!("{} need review", summary.needs_review));
    }
    if summary.active_comments > 0 {
        parts.push(format!("{} active comments", summary.active_comments));
    }
    if summary.unread > 0 {
        parts.push(format!("{} changed", summary.unread));
    }
    if any_org_broken {
        parts.push("sign-in needed".to_owned());
    }
    truncate_utf16(&parts.join(" · "), MAX_TOOLTIP_UTF16)
}
