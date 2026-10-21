//! Persisting UI layout (window rect, columns, filters, peek): debounced 1 s after the last
//! change, flushed immediately when hiding or quitting. Checked every frame without allocating;
//! the state is only built when it changed.

use std::time::Duration;

use prm_config::UiState;
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

/// The live parts of the persisted UI state.
#[derive(Copy, Clone)]
pub(crate) struct Layout<'a> {
    pub view: &'a DashboardView,
    pub window: &'a WindowController,
    pub peek: PeekLayout,
}

impl Layout<'_> {
    /// `base` with the live parts written over it.
    pub(crate) fn collect(&self, base: &UiState) -> UiState {
        let mut state = base.clone();
        self.view.write_ui_state(&mut state);
        state.peek_open = self.peek.open;
        state.peek_width = self.peek.width;
        if let Some(window) = self.window.placement() {
            state.window = Some(window);
        }
        state
    }

    /// Whether `collect` would change any live part of `saved` (compared in place).
    pub(crate) fn differs(&self, saved: &UiState) -> bool {
        self.view.differs_from(saved)
            || saved.peek_open != self.peek.open
            || saved.peek_width != self.peek.width
            || self
                .window
                .placement()
                .is_some_and(|window| saved.window != Some(window))
    }
}

#[derive(Default)]
pub(crate) struct UiStateSync {
    last_sent: Option<UiState>,
    pending_since: Option<Timestamp>,
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

    fn changed(&self, layout: &Layout<'_>) -> bool {
        self.last_sent
            .as_ref()
            .is_none_or(|sent| layout.differs(sent))
    }

    /// Sends once the layout has been stable for `DEBOUNCE`.
    pub(crate) fn tick(
        &mut self,
        now: Timestamp,
        layout: &Layout<'_>,
        base: &UiState,
        link: &ServiceLink,
    ) {
        if !self.changed(layout) {
            self.pending_since = None;
            return;
        }
        let since = *self.pending_since.get_or_insert(now);
        if now >= since.saturating_add(DEBOUNCE) {
            self.send(layout.collect(base), link);
        }
    }

    pub(crate) fn flush(&mut self, layout: &Layout<'_>, base: &UiState, link: &ServiceLink) {
        if self.changed(layout) {
            self.send(layout.collect(base), link);
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
