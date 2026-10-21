//! Ordered, one-off events from the service to the UI, and the sink that delivers them. Durable
//! state goes through the `watch` channel instead; events are for things that must not coalesce.

use std::{
    sync::{Arc, mpsc},
    time::Duration,
};

use prm_config::{ConfigIssue, Severity};
use prm_domain::{
    OrgId, Person,
    auth::{AuthMethod, UserAction},
    diff::ChangeEvent,
    ids::RequestId,
    person::PersonCandidate,
    uri::AppUri,
};

use crate::errors::UiError;

/// One event from the service to the UI, delivered in order and never coalesced.
#[derive(Clone, Debug, PartialEq)]
pub enum AppEvent {
    Notice(Notice),
    /// Bring the window forward (single-instance relaunch, toast click).
    Activate(AppUri),
    PeopleResults {
        request: RequestId,
        org: OrgId,
        query: Arc<str>,
        phase: SearchPhase,
        candidates: Arc<[PersonCandidate]>,
    },
    PersonResolved {
        request: RequestId,
        org: OrgId,
        result: Result<Person, UiError>,
    },
    SignInFinished {
        org: OrgId,
        result: Result<AuthMethod, UiError>,
    },
    /// The PAT was checked against the org; on success it was stored.
    PatResult {
        org: OrgId,
        result: Result<Person, UiError>,
    },
    ConfigApplied {
        issues: Arc<[ConfigIssue]>,
        saved: bool,
    },
    /// Every diff result, including changes whose toasts were suppressed.
    Changes(Arc<[ChangeEvent]>),
    NotificationTestResult(Result<(), UiError>),
    /// The service loop has exited; no more state will be published.
    Stopped,
}

/// People-picker results arrive twice: instantly from the local cache, then merged with the
/// directory.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SearchPhase {
    Local,
    Final,
}

/// A transient in-app message.
#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    pub severity: Severity,
    pub text: Arc<str>,
    pub org: Option<OrgId>,
    pub action: Option<UserAction>,
    pub ttl: Duration,
}

/// Where the service sends events. `wake` asks the UI to run its logic (a repaint request).
pub trait EventSink: Send + Sync + 'static {
    /// Delivers the event (in order) and wakes the UI.
    fn emit(&self, event: AppEvent);
    /// Asks the UI to run its logic without an event (a new state was published).
    fn wake(&self);
}

/// The production sink: a std channel the UI drains synchronously, plus a wake closure.
pub struct ChannelEventSink<W>
where
    W: Fn() + Send + Sync + 'static,
{
    tx: mpsc::Sender<AppEvent>,
    wake: W,
}

impl<W> EventSink for ChannelEventSink<W>
where
    W: Fn() + Send + Sync + 'static,
{
    fn emit(&self, event: AppEvent) {
        // why: a disconnected receiver means the UI is gone; there is nobody left to tell.
        let _ = self.tx.send(event);
        (self.wake)();
    }

    fn wake(&self) {
        (self.wake)();
    }
}

/// A sink and the receiver the UI drains in `App::logic`.
pub fn channel_sink<W>(wake: W) -> (ChannelEventSink<W>, mpsc::Receiver<AppEvent>)
where
    W: Fn() + Send + Sync + 'static,
{
    let (tx, rx) = mpsc::channel();
    (ChannelEventSink { tx, wake }, rx)
}
