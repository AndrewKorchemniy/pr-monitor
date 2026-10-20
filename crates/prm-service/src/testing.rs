//! Hand-written fakes for the service's own ports, a clock that follows tokio's paused time,
//! and `ServiceHarness`, which runs the real actor against fakes.
#![allow(
    clippy::panic,
    reason = "test fixtures fail loudly on bad literals; the lint is silent under cfg(test), so expect would be unfulfilled there"
)]

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use prm_ado::testing::{FakeAdoApi, FakeConnector, FakeOrg};
use prm_auth::testing::FakeAuthFactory;
use prm_config::{Config, PersistedState, store::MemoryRepo};
use prm_domain::{
    OrgId, Timestamp,
    model::Catalog,
    notify::{Notification, NotifierOp},
    ports::{Clock, PortError, PortErrorKind},
    testing::RecordingUrlOpener,
};
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
    time::Instant,
};

use crate::{
    command::{Command, CommandSender},
    event::{AppEvent, EventSink},
    handle::{InitialData, ServiceDeps, build},
    ports::{Autostart, AutostartState, Notifier, NotifierCapability},
    state::AppState,
};

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A command channel without a service, for UI tests.
pub fn command_channel() -> (CommandSender, mpsc::UnboundedReceiver<Command>) {
    let (tx, rx) = mpsc::unbounded_channel();
    (CommandSender::new(tx), rx)
}

/// Records toasts. Optionally samples a counter at each `show` (to check ordering).
#[derive(Default)]
pub struct RecordingNotifier {
    ops: Mutex<Vec<NotifierOp>>,
    capability: Mutex<NotifierCapability>,
    fail_next: AtomicBool,
    probe: Mutex<Option<Arc<dyn Fn() -> usize + Send + Sync>>>,
    probes: Mutex<Vec<usize>>,
}

impl RecordingNotifier {
    pub fn ops(&self) -> Vec<NotifierOp> {
        lock(&self.ops).clone()
    }

    pub fn shown(&self) -> Vec<Notification> {
        self.ops()
            .into_iter()
            .filter_map(|op| match op {
                NotifierOp::Show(n) => Some(n),
                NotifierOp::Retract { .. } => None,
            })
            .collect()
    }

    pub fn set_capability(&self, capability: NotifierCapability) {
        *lock(&self.capability) = capability;
    }

    pub fn fail_next(&self) {
        self.fail_next.store(true, Ordering::SeqCst);
    }

    /// `probe()` is sampled at every `show`; read the samples with `probes()`.
    pub fn set_probe(&self, probe: Arc<dyn Fn() -> usize + Send + Sync>) {
        *lock(&self.probe) = Some(probe);
    }

    pub fn probes(&self) -> Vec<usize> {
        lock(&self.probes).clone()
    }

    fn record(&self, op: NotifierOp) -> Result<(), PortError> {
        if self.fail_next.swap(false, Ordering::SeqCst) {
            return Err(PortError::new(PortErrorKind::Failed, "scripted failure"));
        }
        lock(&self.ops).push(op);
        Ok(())
    }
}

impl Notifier for RecordingNotifier {
    fn show(&self, notification: &Notification) -> Result<(), PortError> {
        let probe = lock(&self.probe).clone();
        if let Some(probe) = probe {
            lock(&self.probes).push(probe());
        }
        self.record(NotifierOp::Show(notification.clone()))
    }

    fn retract(&self, tag: &str, group: &str) -> Result<(), PortError> {
        self.record(NotifierOp::Retract {
            tag: tag.to_owned(),
            group: group.to_owned(),
        })
    }

    fn capability(&self) -> NotifierCapability {
        *lock(&self.capability)
    }
}

/// Records events and counts wakes.
#[derive(Default)]
pub struct RecordingEventSink {
    events: Mutex<Vec<AppEvent>>,
    wakes: AtomicUsize,
}

impl RecordingEventSink {
    pub fn events(&self) -> Vec<AppEvent> {
        lock(&self.events).clone()
    }

    pub fn wakes(&self) -> usize {
        self.wakes.load(Ordering::SeqCst)
    }
}

impl EventSink for RecordingEventSink {
    fn emit(&self, event: AppEvent) {
        lock(&self.events).push(event);
    }

    fn wake(&self) {
        self.wakes.fetch_add(1, Ordering::SeqCst);
    }
}

/// Launch-at-login with a settable OS state; records `set_enabled` calls.
pub struct FakeAutostart {
    state: Mutex<AutostartState>,
    sets: Mutex<Vec<bool>>,
}

impl Default for FakeAutostart {
    fn default() -> Self {
        Self {
            state: Mutex::new(AutostartState::Disabled),
            sets: Mutex::new(Vec::new()),
        }
    }
}

impl FakeAutostart {
    pub fn set_state(&self, state: AutostartState) {
        *lock(&self.state) = state;
    }

    pub fn sets(&self) -> Vec<bool> {
        lock(&self.sets).clone()
    }
}

impl Autostart for FakeAutostart {
    fn state(&self) -> Result<AutostartState, PortError> {
        Ok(*lock(&self.state))
    }

    fn set_enabled(&self, enabled: bool) -> Result<(), PortError> {
        lock(&self.sets).push(enabled);
        *lock(&self.state) = if enabled {
            AutostartState::Enabled
        } else {
            AutostartState::Disabled
        };
        Ok(())
    }
}

/// Wall time that follows tokio's (paused) clock, plus `jump` to simulate a suspend: wall time
/// moves while monotonic time does not.
pub struct TokioClock {
    base_wall: Timestamp,
    base: Instant,
    jumped: Mutex<Duration>,
}

impl TokioClock {
    /// Must be created inside the test runtime.
    pub fn new(base_wall: Timestamp) -> Self {
        Self {
            base_wall,
            base: Instant::now(),
            jumped: Mutex::new(Duration::ZERO),
        }
    }

    pub fn jump(&self, by: Duration) {
        *lock(&self.jumped) += by;
    }

    /// Wall time `by` from now.
    pub fn now_plus(&self, by: Duration) -> Timestamp {
        self.now().saturating_add(by)
    }
}

impl Clock for TokioClock {
    fn now(&self) -> Timestamp {
        let elapsed = Instant::now().saturating_duration_since(self.base);
        self.base_wall
            .saturating_add(elapsed)
            .saturating_add(*lock(&self.jumped))
    }
}

/// What a harness starts from.
pub struct HarnessSetup {
    pub config: Config,
    pub persisted: PersistedState,
    pub catalogs: HashMap<OrgId, Catalog>,
    /// One in-memory org per configured org that should answer.
    pub orgs: Vec<(OrgId, FakeOrg)>,
    pub startup_delay: Duration,
    pub base_wall: Timestamp,
}

impl Default for HarnessSetup {
    fn default() -> Self {
        Self {
            config: Config::default(),
            persisted: PersistedState::default(),
            catalogs: HashMap::new(),
            orgs: Vec::new(),
            startup_delay: Duration::ZERO,
            base_wall: prm_domain::testing::ts(prm_domain::testing::BASE_TIME_SECS),
        }
    }
}

/// The real service actor over fakes, on the test's (paused) runtime.
pub struct ServiceHarness {
    pub commands: CommandSender,
    pub state: watch::Receiver<Arc<AppState>>,
    pub events: Arc<RecordingEventSink>,
    pub notifier: Arc<RecordingNotifier>,
    pub connector: Arc<FakeConnector>,
    pub auth: Arc<FakeAuthFactory>,
    pub repo: Arc<MemoryRepo>,
    pub opener: Arc<RecordingUrlOpener>,
    pub autostart: Arc<FakeAutostart>,
    pub clock: Arc<TokioClock>,
    task: JoinHandle<()>,
}

/// The first published state for which `ready` holds; `None` if the service stopped.
async fn wait_until(
    state: &mut watch::Receiver<Arc<AppState>>,
    ready: &impl Fn(&AppState) -> bool,
) -> Option<Arc<AppState>> {
    loop {
        let current = state.borrow_and_update().clone();
        if ready(&current) {
            return Some(current);
        }
        state.changed().await.ok()?;
    }
}

/// Longest a `wait_for` lets (paused) time run before failing the test.
const WAIT_LIMIT: Duration = Duration::from_hours(48);

impl ServiceHarness {
    pub fn start(setup: HarnessSetup) -> Self {
        let connector = Arc::new(FakeConnector::default());
        for (org, contents) in setup.orgs {
            connector.insert(Arc::new(FakeAdoApi::new(&org, contents)));
        }
        let events = Arc::new(RecordingEventSink::default());
        let notifier = Arc::new(RecordingNotifier::default());
        let auth = Arc::new(FakeAuthFactory::default());
        let repo = Arc::new(MemoryRepo::default());
        let opener = Arc::new(RecordingUrlOpener::default());
        let autostart = Arc::new(FakeAutostart::default());
        let clock = Arc::new(TokioClock::new(setup.base_wall));
        let deps = ServiceDeps {
            ado: connector.clone(),
            auth: auth.clone(),
            notifier: notifier.clone(),
            events: events.clone(),
            clock: clock.clone(),
            opener: opener.clone(),
            autostart: autostart.clone(),
            repo: repo.clone(),
        };
        let (config, config_issues) = prm_config::normalize(setup.config);
        let init = InitialData {
            config,
            config_issues,
            persisted: setup.persisted,
            catalogs: setup.catalogs,
            startup_delay: Some(setup.startup_delay),
            rng_seed: Some(7),
        };
        let (commands, state, service) = build(deps, init);
        Self {
            commands,
            state,
            events,
            notifier,
            connector,
            auth,
            repo,
            opener,
            autostart,
            clock,
            task: tokio::spawn(service),
        }
    }

    pub fn send(&self, command: Command) {
        assert!(self.commands.send(command), "service stopped");
    }

    pub fn snapshot(&self) -> Arc<AppState> {
        self.state.borrow().clone()
    }

    /// Lets (paused) time run until `ready` holds; panics after `WAIT_LIMIT`.
    pub async fn wait_for(
        &mut self,
        what: &str,
        ready: impl Fn(&AppState) -> bool,
    ) -> Arc<AppState> {
        let waited = tokio::time::timeout(WAIT_LIMIT, wait_until(&mut self.state, &ready)).await;
        let found = waited.unwrap_or_else(|_| panic!("timed out waiting for {what}"));
        found.unwrap_or_else(|| panic!("service stopped while waiting for {what}"))
    }

    /// Advances paused time and lets the actor and its jobs run.
    pub async fn advance(&self, by: Duration) {
        tokio::time::advance(by).await;
        self.settle().await;
    }

    /// Yields until spawned work has had a chance to run (blocking jobs included).
    pub async fn settle(&self) {
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }
    }

    pub fn events(&self) -> Vec<AppEvent> {
        self.events.events()
    }

    pub fn api(&self, org: &OrgId) -> Arc<FakeAdoApi> {
        self.connector
            .api(org)
            .unwrap_or_else(|| panic!("no fake org {org}"))
    }

    /// Sends `Shutdown` and waits for the actor to finish.
    pub async fn stop(self) {
        self.send(Command::Shutdown);
        let _ = self.task.await;
    }
}
