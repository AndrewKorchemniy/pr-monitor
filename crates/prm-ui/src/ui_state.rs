//! Persisting UI layout (window rect, columns, filters, peek): debounced 1 s after the last
//! change, flushed immediately when hiding or quitting.

use std::time::Duration;

use prm_config::{UiState, WindowRect};
use prm_domain::Timestamp;
use prm_service::Command;

use crate::{link::ServiceLink, view::DashboardView, window::WindowController};

pub(crate) const DEBOUNCE: Duration = Duration::from_secs(1);

/// Peek drawer layout (persisted with the view).
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct PeekLayout {
    pub open: bool,
    pub width: Option<f32>,
}

#[derive(Default)]
pub(crate) struct UiStateSync {
    last_sent: Option<UiState>,
    pending_since: Option<Timestamp>,
}

pub(crate) fn collect(
    view: &DashboardView,
    window: &WindowController,
    peek: PeekLayout,
    base: &UiState,
) -> UiState {
    let mut state = base.clone();
    view.write_ui_state(&mut state);
    state.peek_open = peek.open;
    state.peek_width = peek.width;
    if let Some(rect) = window.outer_rect {
        state.window = Some(WindowRect {
            x: rect.min.x,
            y: rect.min.y,
            w: rect.width(),
            h: rect.height(),
            maximized: window.maximized,
        });
    }
    state
}

impl UiStateSync {
    pub(crate) fn new(initial: UiState) -> Self {
        Self {
            last_sent: Some(initial),
            pending_since: None,
        }
    }

    /// The deadline of a pending save, for repaint scheduling.
    pub(crate) fn deadline(&self) -> Option<Timestamp> {
        self.pending_since.map(|at| at.saturating_add(DEBOUNCE))
    }

    /// Sends once the state has been stable for `DEBOUNCE`.
    pub(crate) fn tick(&mut self, now: Timestamp, current: UiState, link: &ServiceLink) {
        if self.last_sent.as_ref() == Some(&current) {
            self.pending_since = None;
            return;
        }
        let since = *self.pending_since.get_or_insert(now);
        if now >= since.saturating_add(DEBOUNCE) {
            self.send(current, link);
        }
    }

    pub(crate) fn flush(&mut self, current: UiState, link: &ServiceLink) {
        if self.last_sent.as_ref() != Some(&current) {
            self.send(current, link);
        }
    }

    fn send(&mut self, current: UiState, link: &ServiceLink) {
        link.send(Command::SaveUiState(Box::new(current.clone())));
        self.last_sent = Some(current);
        self.pending_since = None;
    }
}

#[cfg(test)]
mod tests;
