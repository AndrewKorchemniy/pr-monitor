//! Routing activations (a second launch, a toast button, `prmonitor://` links) to the service.
//! Activations that arrive before the service exists are buffered in order.

use std::sync::{
    Arc, Mutex, OnceLock, PoisonError,
    atomic::{AtomicBool, Ordering},
};

use prm_domain::uri::AppUri;
use prm_platform::{ActivationSink, HandoffRequest};
use prm_service::{Command, CommandSender};

/// At most this many activations wait for the service; the oldest are dropped.
const BUFFER: usize = 16;

pub(crate) trait CommandSink: Send + Sync + 'static {
    fn send(&self, command: Command) -> bool;
}

impl CommandSink for CommandSender {
    fn send(&self, command: Command) -> bool {
        Self::send(self, command)
    }
}

/// Wakes the UI (so `logic` runs even while hidden) once the context exists.
#[derive(Default)]
pub(crate) struct UiWaker(OnceLock<eframe::egui::Context>);

impl UiWaker {
    pub(crate) fn set(&self, ctx: eframe::egui::Context) {
        let _ = self.0.set(ctx);
    }

    pub(crate) fn wake(&self) {
        if let Some(ctx) = self.0.get() {
            ctx.request_repaint();
        }
    }
}

enum RouterState {
    Buffering(Vec<HandoffRequest>),
    Attached(Arc<dyn CommandSink>),
}

pub(crate) struct ActivationRouter {
    state: Mutex<RouterState>,
    waker: Arc<UiWaker>,
    quit: Arc<AtomicBool>,
}

impl ActivationRouter {
    pub(crate) fn new(waker: Arc<UiWaker>, quit: Arc<AtomicBool>) -> Self {
        Self {
            state: Mutex::new(RouterState::Buffering(Vec::new())),
            waker,
            quit,
        }
    }

    /// Connects the service and replays what was buffered.
    pub(crate) fn attach(&self, sink: Arc<dyn CommandSink>) {
        let buffered = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            match std::mem::replace(&mut *state, RouterState::Attached(Arc::clone(&sink))) {
                RouterState::Buffering(items) => items,
                RouterState::Attached(_) => Vec::new(),
            }
        };
        for request in buffered {
            self.dispatch(sink.as_ref(), request);
        }
    }

    /// `--activate <uri>` from our own command line; invalid links are logged and ignored.
    pub(crate) fn activate_raw(&self, raw: &str) {
        match AppUri::parse(raw) {
            Ok(uri) => self.activate(HandoffRequest::Activate(uri)),
            Err(error) => tracing::warn!(%error, "ignored an invalid activation link"),
        }
    }

    fn dispatch(&self, sink: &dyn CommandSink, request: HandoffRequest) {
        let command = match request {
            HandoffRequest::Show => Command::Activate(AppUri::Show { key: None }),
            HandoffRequest::Activate(uri) => Command::Activate(uri),
            HandoffRequest::Quit => {
                self.quit.store(true, Ordering::Relaxed);
                self.waker.wake();
                return;
            }
        };
        if !sink.send(command) {
            tracing::warn!("service stopped; activation dropped");
        }
        self.waker.wake();
    }
}

/// Keeps the newest `BUFFER` activations.
fn buffer(items: &mut Vec<HandoffRequest>, request: HandoffRequest) {
    if items.len() == BUFFER {
        items.remove(0);
    }
    items.push(request);
}

impl ActivationSink for ActivationRouter {
    fn activate(&self, request: HandoffRequest) {
        let sink = {
            let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
            match &mut *state {
                RouterState::Attached(sink) => Arc::clone(sink),
                RouterState::Buffering(items) => return buffer(items, request),
            }
        };
        // why: never call into the service while holding the lock.
        self.dispatch(sink.as_ref(), request);
    }
}

#[cfg(test)]
mod tests;
