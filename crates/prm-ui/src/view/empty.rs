//! Which empty state to show, as a pure decision over the app state and the view.

use std::sync::Arc;

use prm_domain::{PrStatus, view::StatusFilter};
use prm_service::{AppState, AuthStateView, RefreshPhase};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EmptyKind {
    Loading {
        phase: Option<RefreshPhase>,
        done: u32,
        total: u32,
        org: Arc<str>,
    },
    SignedOut,
    /// No open pull requests by anyone tracked (everyone, when no people are listed).
    NoActivePrs,
    NoMatch {
        query: String,
    },
    StatusEmpty(PrStatus),
    NothingChanged,
}

/// The view's filters (from `ViewQuery`) and how many rows pass them.
pub(crate) struct EmptyInputs<'a> {
    pub state: &'a AppState,
    pub visible: usize,
    pub text: &'a str,
    pub status: StatusFilter,
    pub unread_only: bool,
}

/// `None` when there is something to show.
pub(crate) fn empty_kind(input: &EmptyInputs<'_>) -> Option<EmptyKind> {
    let state = input.state;
    if state.rows.is_empty() {
        return Some(no_rows(state));
    }
    if input.visible > 0 {
        return None;
    }
    let status = match input.status {
        StatusFilter::All => None,
        StatusFilter::NeedsReview => Some(PrStatus::NeedsReview),
        StatusFilter::ActiveComments => Some(PrStatus::ActiveComments),
        StatusFilter::Reviewed => Some(PrStatus::Reviewed),
    };
    Some(if !input.text.trim().is_empty() {
        EmptyKind::NoMatch {
            query: input.text.trim().to_owned(),
        }
    } else if let Some(status) = status {
        EmptyKind::StatusEmpty(status)
    } else if input.unread_only {
        EmptyKind::NothingChanged
    } else {
        EmptyKind::NoMatch {
            query: String::new(),
        }
    })
}

fn no_rows(state: &AppState) -> EmptyKind {
    let enabled: Vec<_> = state.orgs.iter().filter(|o| o.enabled).collect();
    let checking = enabled.iter().any(|o| {
        matches!(
            o.auth.state,
            AuthStateView::Unknown | AuthStateView::Checking | AuthStateView::SigningIn { .. }
        )
    });
    let first_refresh = state.refresh.in_progress && state.refresh.last_completed.is_none();
    if checking || first_refresh {
        let progress = state.refresh.progress.first();
        return EmptyKind::Loading {
            phase: progress.map(|p| p.phase),
            done: progress.map_or(0, |p| p.done),
            total: progress.map_or(0, |p| p.total),
            org: enabled
                .first()
                .map_or_else(|| Arc::from(""), |o| o.label.clone()),
        };
    }
    let signed_out = !enabled.is_empty() && enabled.iter().all(|o| o.needs_sign_in());
    if signed_out {
        EmptyKind::SignedOut
    } else {
        EmptyKind::NoActivePrs
    }
}

#[cfg(test)]
mod tests;
