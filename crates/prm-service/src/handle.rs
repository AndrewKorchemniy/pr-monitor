//! Starting and stopping the service: the dependencies it needs, the data it starts from, and
//! the handle the bin keeps.

use std::{
    collections::HashMap,
    future::Future,
    sync::Arc,
    thread::JoinHandle,
    time::{Duration, Instant},
};

use prm_ado::ClientPolicy;
use prm_config::{Config, ConfigIssue, PersistedState};
use prm_domain::{OrgId, Timestamp, model::Catalog};
use tokio::sync::{mpsc, watch};

use crate::{
    actor::Service,
    command::{Command, CommandSender},
    consts::MAX_BLOCKING_THREADS,
    org::{Connectors, OrgLive, startup_orgs},
    ports::{
        AdoConnector, AuthFactory, Autostart, Clock, EventSink, Notifier, StateRepo, UrlOpener,
    },
    publish::{self, Composition, OrgView},
    scheduler::Moment,
    state::{AppState, AutostartStatus, NotificationsStatus},
};

/// Every port the service talks to.
#[derive(Clone)]
pub struct ServiceDeps {
    pub ado: Arc<dyn AdoConnector>,
    pub auth: Arc<dyn AuthFactory>,
    pub notifier: Arc<dyn Notifier>,
    pub events: Arc<dyn EventSink>,
    pub clock: Arc<dyn Clock>,
    pub opener: Arc<dyn UrlOpener>,
    pub autostart: Arc<dyn Autostart>,
    pub repo: Arc<dyn StateRepo>,
}

impl ServiceDeps {
    /// The ports an org's clients are built from.
    pub(crate) fn connectors<'a>(&'a self, policy: &'a ClientPolicy) -> Connectors<'a> {
        Connectors {
            auth: self.auth.as_ref(),
            ado: self.ado.as_ref(),
            policy,
        }
    }
}

/// What the service starts from (loaded by the bin before any thread starts).
#[derive(Clone, Debug, Default)]
pub struct InitialData {
    pub config: Config,
    pub config_issues: Vec<ConfigIssue>,
    pub persisted: PersistedState,
    pub catalogs: HashMap<OrgId, Catalog>,
    /// Overrides `refresh.startup_delay_secs` (tests, `--debug`).
    pub startup_delay: Option<Duration>,
    /// Seeds scheduling jitter; random when `None`.
    pub rng_seed: Option<u64>,
}

/// Owned by the bin. Dropping it without `shutdown` leaves the thread to exit when the command
/// channel closes.
pub struct ServiceHandle {
    commands: CommandSender,
    state: watch::Receiver<Arc<AppState>>,
    thread: Option<JoinHandle<()>>,
}

impl ServiceHandle {
    /// A sender for commands; clones are cheap and may outlive the handle.
    pub fn sender(&self) -> CommandSender {
        self.commands.clone()
    }

    /// The latest published state; `changed()` fires on each publish.
    pub fn state(&self) -> watch::Receiver<Arc<AppState>> {
        self.state.clone()
    }

    /// Asks the service to stop and waits up to `timeout`. `true` if it stopped in time.
    pub fn shutdown(mut self, timeout: Duration) -> bool {
        const POLL: Duration = Duration::from_millis(10);
        self.commands.send(Command::Shutdown);
        let Some(thread) = self.thread.take() else {
            return true;
        };
        let deadline = Instant::now() + timeout;
        while !thread.is_finished() && Instant::now() < deadline {
            std::thread::sleep(POLL);
        }
        thread.is_finished() && thread.join().is_ok()
    }
}

/// Builds the first state synchronously (cached rows are visible on the first frame), then
/// runs the service on its own thread with a current-thread runtime.
pub fn spawn(deps: ServiceDeps, init: InitialData) -> std::io::Result<ServiceHandle> {
    let (commands, state, service) = build(deps, init);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(MAX_BLOCKING_THREADS)
        .thread_name("svc-blocking")
        .build()?;
    let thread = std::thread::Builder::new()
        .name("svc".into())
        .spawn(move || runtime.block_on(service))?;
    Ok(ServiceHandle {
        commands,
        state,
        thread: Some(thread),
    })
}

/// The same wiring without a thread: tests run the future on a paused-time runtime.
pub fn build(
    deps: ServiceDeps,
    init: InitialData,
) -> (
    CommandSender,
    watch::Receiver<Arc<AppState>>,
    impl Future<Output = ()> + Send,
) {
    let now = deps.clock.now();
    let first = initial_state(&init.config, &init.persisted, &init.catalogs, now);
    let (state_tx, state_rx) = watch::channel(Arc::new(first));
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let service = Service::new(deps, init, state_tx);
    (
        CommandSender::new(command_tx),
        state_rx,
        service.run(command_rx),
    )
}

/// The state shown before the service has done anything: cached rows from `state.json`.
pub fn initial_state(
    config: &Config,
    persisted: &PersistedState,
    catalogs: &HashMap<OrgId, Catalog>,
    now: Timestamp,
) -> AppState {
    let orgs = startup_orgs(config, |id| persisted.orgs.get(id).cloned(), catalogs, now);
    let lives: Vec<OrgLive> = orgs.iter().map(OrgLive::initial).collect();
    let views: Vec<OrgView<'_>> = orgs
        .iter()
        .zip(&lives)
        .map(|(data, live)| OrgView { data, live })
        .collect();
    let config = Arc::new(config.clone());
    let issues: Arc<[ConfigIssue]> = Arc::from([]);
    let ui_state = Arc::new(persisted.ui.clone());
    publish::compose(Composition {
        config: &config,
        config_issues: &issues,
        rows: publish::rows_for(&views, &persisted.seen, now).into(),
        orgs: &views,
        rows_generation: 0,
        generation: 0,
        progress: Arc::from([]),
        notifications: NotificationsStatus {
            enabled: config.notifications.enabled,
            paused_until: persisted.notifications_paused_until,
            ..NotificationsStatus::default()
        },
        ui_state: &ui_state,
        autostart: &AutostartStatus {
            desired: config.ui.launch_at_login,
            ..AutostartStatus::default()
        },
        shutting_down: false,
        now: Moment {
            mono: tokio::time::Instant::now(),
            wall: now,
        },
    })
}
