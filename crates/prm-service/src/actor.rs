//! The service actor: owns all mutable state, handles commands and job results on one task, and
//! publishes `AppState`. Handlers never await I/O; they change state and spawn jobs.

mod auth;
mod commands;
mod config;
mod cycle;
mod people;
mod persist;
mod refresh;
mod results;

use std::{
    collections::{BTreeMap, HashMap},
    future::Future,
    ops::ControlFlow,
    sync::Arc,
    time::Duration,
};

use prm_ado::ClientPolicy;
use prm_config::{Config, ConfigIssue, UiState};
use prm_domain::{OrgId, Timestamp, snapshot::SeenMap, view::PrRow};
use tokio::{
    sync::{
        mpsc::{self, UnboundedReceiver, UnboundedSender},
        watch,
    },
    task::{AbortHandle, Id, JoinError, JoinSet},
    time::Instant,
};
use tokio_util::sync::CancellationToken;

use crate::{
    command::Command,
    consts::{PUBLISH_THROTTLE, SCHEDULER_MAX_SLEEP},
    event::{AppEvent, Notice},
    handle::{InitialData, ServiceDeps},
    jobs::{JobKind, JobOutput, JobTag, blocking},
    org::{Connectors, OrgData, OrgLink, OrgLive, OrgRuntime},
    ports::NotifierCapability,
    publish::{self, Composition, NoticeLog, OrgView},
    scheduler::{Moment, Policy},
    state::{AppState, AutostartStatus, NotificationsStatus, PauseReason, RefreshProgress},
};

use cycle::Cycles;
use persist::PersistState;

#[derive(Copy, Clone, Debug, Default)]
struct Presence {
    visible: bool,
    focused: bool,
}

struct Publisher {
    tx: watch::Sender<Arc<AppState>>,
    generation: u64,
    /// Something besides progress changed: publish at the end of this turn.
    dirty: bool,
    /// Only progress changed: publish at most every `PUBLISH_THROTTLE`.
    progress_dirty: bool,
    last_at: Option<Instant>,
    deadline: Option<Instant>,
}

struct Rows {
    rows: Arc<[PrRow]>,
    generation: u64,
    dirty: bool,
}

pub(crate) struct Service {
    deps: ServiceDeps,
    config: Arc<Config>,
    config_issues: Arc<[ConfigIssue]>,
    orgs: Vec<OrgRuntime>,
    seen: SeenMap,
    ui_state: Arc<UiState>,
    paused_until: Option<Timestamp>,
    presence: Presence,
    capability: NotifierCapability,
    last_suppressed: u32,
    autostart: AutostartStatus,
    /// The newest autostart job; older results are stale and ignored.
    autostart_seq: u64,
    jobs: JoinSet<JobOutput>,
    tags: HashMap<Id, JobTag>,
    progress_tx: UnboundedSender<RefreshProgress>,
    progress_rx: UnboundedReceiver<RefreshProgress>,
    progress: BTreeMap<OrgId, RefreshProgress>,
    cycles: Cycles,
    rows: Rows,
    publisher: Publisher,
    persist: PersistState,
    notices: NoticeLog,
    rng: fastrand::Rng,
    last_wake: Option<Moment>,
    root: CancellationToken,
    startup_delay: Duration,
    shutting_down: bool,
}

impl Service {
    pub(crate) fn new(
        deps: ServiceDeps,
        init: InitialData,
        tx: watch::Sender<Arc<AppState>>,
    ) -> Self {
        let InitialData {
            config,
            config_issues,
            mut persisted,
            catalogs,
            startup_delay,
            rng_seed,
        } = init;
        let orgs = connect_orgs(&deps, &config, &mut persisted, &catalogs);
        let (progress_tx, progress_rx) = mpsc::unbounded_channel();
        Self {
            startup_delay: startup_delay.unwrap_or(Duration::from_secs(u64::from(
                config.refresh.startup_delay_secs,
            ))),
            autostart: AutostartStatus {
                desired: config.ui.launch_at_login,
                ..AutostartStatus::default()
            },
            autostart_seq: 0,
            config: Arc::new(config),
            config_issues: config_issues.into(),
            orgs,
            seen: persisted.seen,
            ui_state: Arc::new(persisted.ui),
            paused_until: persisted.notifications_paused_until,
            presence: Presence::default(),
            capability: NotifierCapability::Unknown,
            last_suppressed: 0,
            jobs: JoinSet::new(),
            tags: HashMap::new(),
            progress_tx,
            progress_rx,
            progress: BTreeMap::new(),
            cycles: Cycles::default(),
            rows: Rows {
                rows: Arc::from([]),
                generation: 0,
                dirty: true,
            },
            publisher: Publisher {
                tx,
                generation: 0,
                dirty: true,
                progress_dirty: false,
                last_at: None,
                deadline: None,
            },
            persist: PersistState::default(),
            notices: NoticeLog::default(),
            rng: rng_seed.map_or_else(fastrand::Rng::new, fastrand::Rng::with_seed),
            last_wake: None,
            root: CancellationToken::new(),
            deps,
            shutting_down: false,
        }
    }

    pub(crate) async fn run(mut self, mut commands: UnboundedReceiver<Command>) {
        self.on_start();
        self.after_turn();
        loop {
            let wake = self.next_wake();
            let flow = tokio::select! {
                biased;
                command = commands.recv() => match command {
                    Some(command) => self.on_command(command),
                    None => ControlFlow::Break(()),
                },
                Some(joined) = self.jobs.join_next_with_id(), if !self.jobs.is_empty() => {
                    self.on_joined(joined);
                    ControlFlow::Continue(())
                }
                Some(progress) = self.progress_rx.recv() => {
                    self.on_progress(progress);
                    ControlFlow::Continue(())
                }
                () = tokio::time::sleep_until(wake) => {
                    self.on_wake();
                    ControlFlow::Continue(())
                }
            };
            if flow.is_break() {
                break;
            }
            self.after_turn();
        }
        self.shutdown().await;
    }

    fn now(&self) -> Moment {
        Moment {
            mono: Instant::now(),
            wall: self.deps.clock.now(),
        }
    }

    fn policy(&self) -> Policy {
        Policy::from_config(&self.config.refresh)
    }

    fn index(&self, org: &OrgId) -> Option<usize> {
        self.orgs.iter().position(|o| &o.data.id == org)
    }

    fn connectors_policy(&self) -> ClientPolicy {
        client_policy(&self.config)
    }

    /// Spawns a job and remembers what it is, for panic attribution.
    fn spawn<F>(&mut self, tag: JobTag, job: F) -> AbortHandle
    where
        F: Future<Output = JobOutput> + Send + 'static,
    {
        let handle = self.jobs.spawn(job);
        self.tags.insert(handle.id(), tag);
        handle
    }

    fn notice(&mut self, notice: Notice) {
        if self
            .notices
            .admit(&notice.text, notice.org.as_ref(), Instant::now())
        {
            self.deps.events.emit(AppEvent::Notice(notice));
        }
    }

    fn on_start(&mut self) {
        let now = self.now();
        self.last_wake = Some(now);
        for org in &mut self.orgs {
            org.live = OrgLive::initial(&org.data);
            if org.data.profile.enabled && !org.data.signed_out {
                org.live.schedule.due_at(now, self.startup_delay);
            }
        }
        self.spawn_capability_check();
        self.spawn_autostart(None);
        tracing::info!(orgs = self.orgs.len(), "service started");
    }

    /// The earliest deadline, never more than `SCHEDULER_MAX_SLEEP` away.
    fn next_wake(&self) -> Instant {
        let now = self.now();
        let mut wake = now.mono + SCHEDULER_MAX_SLEEP;
        for org in self.orgs.iter().filter(|o| o.link.in_flight.is_none()) {
            let schedule = &org.live.schedule;
            let next = match schedule.paused {
                None => schedule.due,
                Some(PauseReason::Auth) => schedule.next_probe,
                Some(PauseReason::SignedOut | PauseReason::RateLimited) => None,
            };
            wake = next.map_or(wake, |at| wake.min(at));
        }
        let deadlines = [
            self.publisher.deadline,
            self.persist.deadline,
            self.paused_until.map(|until| now.mono_of(until)),
        ];
        deadlines.into_iter().flatten().fold(wake, Instant::min)
    }

    fn on_wake(&mut self) {
        let now = self.now();
        if let Some(previous) = self.last_wake
            && crate::scheduler::detect_resume(previous, now)
        {
            self.settle_after_resume(now);
        }
        self.last_wake = Some(now);
        self.start_due_refreshes(now);
        self.start_due_probes(now);
        if self.paused_until.is_some_and(|until| until <= now.wall) {
            self.paused_until = None;
            self.publisher.dirty = true;
            self.mark_persist();
        }
    }

    fn on_progress(&mut self, progress: RefreshProgress) {
        self.progress.insert(progress.org.clone(), progress);
        self.publisher.progress_dirty = true;
    }

    fn on_joined(&mut self, joined: Result<(Id, JobOutput), JoinError>) {
        self.publisher.dirty = true;
        match joined {
            Ok((id, output)) => {
                self.tags.remove(&id);
                self.on_output(output);
            }
            Err(error) => {
                let tag = self.tags.remove(&error.id());
                if error.is_panic() {
                    self.on_panic(tag);
                }
            }
        }
    }

    fn after_turn(&mut self) {
        let now = Instant::now();
        if self.persist.deadline.is_some_and(|at| at <= now) {
            self.start_persist();
        }
        if self.publisher.dirty {
            self.publish();
        } else if self.publisher.progress_dirty {
            let next = self.publisher.last_at.map(|at| at + PUBLISH_THROTTLE);
            match next {
                Some(at) if at > now => self.publisher.deadline = Some(at),
                Some(_) | None => self.publish(),
            }
        }
    }

    fn views(&self) -> Vec<OrgView<'_>> {
        self.orgs
            .iter()
            .map(|o| OrgView {
                data: &o.data,
                live: &o.live,
            })
            .collect()
    }

    fn publish(&mut self) {
        let now = self.now();
        if self.rows.dirty {
            let rows = publish::rows_for(&self.views(), &self.seen, now.wall);
            self.rows = Rows {
                rows: rows.into(),
                generation: self.rows.generation + 1,
                dirty: false,
            };
        }
        self.publisher.generation += 1;
        let state = publish::compose(Composition {
            config: &self.config,
            config_issues: &self.config_issues,
            orgs: &self.views(),
            rows: self.rows.rows.clone(),
            rows_generation: self.rows.generation,
            generation: self.publisher.generation,
            progress: self.progress.values().cloned().collect(),
            notifications: self.notifications_status(),
            ui_state: &self.ui_state,
            autostart: &self.autostart,
            shutting_down: self.shutting_down,
            now,
        });
        self.publisher.tx.send_replace(Arc::new(state));
        self.publisher.dirty = false;
        self.publisher.progress_dirty = false;
        self.publisher.last_at = Some(now.mono);
        self.publisher.deadline = None;
        self.deps.events.wake();
    }

    fn notifications_status(&self) -> NotificationsStatus {
        NotificationsStatus {
            enabled: self.config.notifications.enabled,
            paused_until: self.paused_until,
            capability: self.capability,
            last_suppressed: self.last_suppressed,
        }
    }

    async fn shutdown(mut self) {
        tracing::info!("service stopping");
        self.root.cancel();
        self.jobs.abort_all();
        let state = self.build_persisted();
        let repo = self.deps.repo.clone();
        if let Some(Err(error)) = blocking(move || repo.save_state(&state)).await {
            tracing::warn!(%error, "final save failed");
        }
        self.shutting_down = true;
        self.publish();
        self.deps.events.emit(AppEvent::Stopped);
    }
}

fn client_policy(config: &Config) -> ClientPolicy {
    ClientPolicy {
        max_concurrency: usize::from(config.network.max_concurrency.max(1)),
        ..ClientPolicy::default()
    }
}

/// Runtimes for every configured org with an id, starting from what was persisted.
fn connect_orgs(
    deps: &ServiceDeps,
    config: &Config,
    persisted: &mut prm_config::PersistedState,
    catalogs: &HashMap<OrgId, prm_domain::model::Catalog>,
) -> Vec<OrgRuntime> {
    let now = deps.clock.now();
    let policy = client_policy(config);
    let connectors = Connectors {
        auth: deps.auth.as_ref(),
        ado: deps.ado.as_ref(),
        policy: &policy,
    };
    config
        .orgs
        .iter()
        .filter_map(|profile| {
            let id = profile.org_id()?.clone();
            let saved = persisted.orgs.remove(&id);
            let catalog = catalogs.get(&id).cloned();
            let data = OrgData::new(id, profile.clone(), saved, catalog, now);
            Some(runtime(data, &connectors))
        })
        .collect()
}

fn runtime(data: OrgData, connectors: &Connectors<'_>) -> OrgRuntime {
    let link = OrgLink::connect(&data, connectors);
    let live = OrgLive::initial(&data);
    OrgRuntime { data, live, link }
}

impl Service {
    fn new_runtime(&self, data: OrgData) -> OrgRuntime {
        let policy = self.connectors_policy();
        let connectors = Connectors {
            auth: self.deps.auth.as_ref(),
            ado: self.deps.ado.as_ref(),
            policy: &policy,
        };
        runtime(data, &connectors)
    }

    fn spawn_capability_check(&mut self) {
        let notifier = self.deps.notifier.clone();
        self.spawn(JobTag::global(JobKind::Port), async move {
            let capability = blocking(move || notifier.capability()).await;
            JobOutput::Capability(capability.unwrap_or_default())
        });
    }
}
