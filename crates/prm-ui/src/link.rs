//! The UI's end of the service channels.

use std::sync::{Arc, mpsc};

use prm_service::{AppEvent, AppState, Command, CommandSender};
use tokio::sync::watch;

pub(crate) struct ServiceLink {
    tx: CommandSender,
    state_rx: watch::Receiver<Arc<AppState>>,
    events_rx: mpsc::Receiver<AppEvent>,
    stopped: bool,
}

/// Whether new state is waiting.
pub(crate) enum StateCheck {
    Unchanged,
    Changed,
    /// The service thread is gone (reported once).
    Stopped,
}

impl ServiceLink {
    pub(crate) fn new(
        tx: CommandSender,
        state_rx: watch::Receiver<Arc<AppState>>,
        events_rx: mpsc::Receiver<AppEvent>,
    ) -> Self {
        Self {
            tx,
            state_rx,
            events_rx,
            stopped: false,
        }
    }

    pub(crate) fn send(&self, command: Command) {
        if !self.tx.send(command) {
            tracing::warn!("service stopped; command dropped");
        }
    }

    pub(crate) fn check(&mut self) -> StateCheck {
        if self.stopped {
            return StateCheck::Unchanged;
        }
        match self.state_rx.has_changed() {
            Ok(true) => StateCheck::Changed,
            Ok(false) => StateCheck::Unchanged,
            Err(_) => {
                self.stopped = true;
                StateCheck::Stopped
            }
        }
    }

    /// Never holds the borrow past this call.
    pub(crate) fn latest(&mut self) -> Arc<AppState> {
        self.state_rx.borrow_and_update().clone()
    }

    pub(crate) fn try_event(&self) -> Option<AppEvent> {
        self.events_rx.try_recv().ok()
    }
}

#[cfg(test)]
mod tests;
