//! Startup, in a fixed order: the panic hook, arguments, data root, the single-instance lock (a
//! second launch forwards and exits before any logging or GPU work, or takes over from a primary
//! that is quitting), files and logging, shell identity, activation, config, the service, the
//! renderer, the window; then shutdown or a relaunch.

use std::{
    ffi::OsString,
    io::Write as _,
    path::PathBuf,
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use prm_auth::ChunkedSecretStore;
use prm_config::{
    AppPaths, ConfigLoad, FileRepo, StateLoad, load_catalogs, load_config, load_state,
};
use prm_domain::{
    app::{ENV_HOME, ENV_LOG, ENV_REGISTER, ENV_SECRET_STORE},
    ports::{Clock, PortError},
    uri::AppUri,
};
use prm_platform::{
    Acquired, BuildKind, ForwardOutcome, HandoffRequest, HandoffServer, IdentityStatus,
    InstanceKey, InstanceLock, InstanceScope, PlatformAdapters, PlatformInit, RegisterOverride,
    RegistrationScope, RegistrationSpec, SecretBackendPref, acquire_instance, adapters, console,
    ensure_shell_identity, forward_to_primary, is_same_root, registration_scope,
};
use prm_service::{ServiceHandle, SystemClock, channel_sink};

use crate::{
    activation::{ActivationRouter, UiWaker},
    assets, build_info,
    cli::{self, Args, Mode},
    copy, logging, net, panic,
    renderer::{
        GLOW_FAILED, MarkerFile, RendererChoice, RendererInputs, choose_renderer,
        is_renderer_init_error,
    },
    self_test,
    shutdown::{self, Held},
    ui_launch::{self, LaunchCtx},
    unregister,
    wiring::{self, Platform, Ports},
};

pub(crate) const EXIT_USAGE: u8 = 2;
pub(crate) const EXIT_SELF_TEST: u8 = 3;
pub(crate) const EXIT_NOT_RESPONDING: u8 = 4;

/// The fallback child waits this long for its parent to exit.
const PARENT_EXIT_WAIT: Duration = Duration::from_secs(5);
/// A secondary gives a starting primary this long to accept the connection.
const FORWARD_BUDGET: Duration = Duration::from_secs(2);
/// With no primary listening, a launch waits this long for a quitting one to release the lock.
const HANDOFF: Duration = Duration::from_secs(5);
const LOCK_RETRY: Duration = Duration::from_millis(100);
const CRASH_LOGS_KEPT: usize = 10;

/// What this process knows from its first line on.
struct Launch {
    started: Instant,
    /// True until the first frame (from `panic::install`).
    init_phase: Arc<AtomicBool>,
}

/// What a second launch does after trying to reach the primary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Next {
    /// The primary has the request.
    Done,
    Fatal(String),
    NotResponding,
    /// Nobody is listening: the primary is quitting (or crashed), so wait for the lock.
    TakeOver,
}

pub(crate) fn after_forward(outcome: ForwardOutcome) -> Next {
    match outcome {
        ForwardOutcome::Delivered => Next::Done,
        ForwardOutcome::Rejected(reason) => Next::Fatal(reason),
        ForwardOutcome::NoPrimary => Next::TakeOver,
        ForwardOutcome::NotResponding => Next::NotResponding,
    }
}

/// A renderer that failed to start gets one relaunch with the other, but only before the first
/// frame: after that the window worked and a relaunch would hide a real bug.
pub(crate) const fn should_relaunch(
    init_error: bool,
    already_relaunched: bool,
    first_frame_seen: bool,
) -> bool {
    init_error && !already_relaunched && !first_frame_seen
}

/// What this launch does, decided from the arguments alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StartupPlan {
    Print(String),
    SelfTest,
    Unregister { purge: bool },
    Run,
}

pub(crate) fn plan(args: &Args) -> StartupPlan {
    match args.mode {
        Mode::Version => StartupPlan::Print(build_info::version_line()),
        Mode::Help => StartupPlan::Print(cli::usage().to_owned()),
        Mode::SelfTest => StartupPlan::SelfTest,
        Mode::Unregister { purge } => StartupPlan::Unregister { purge },
        Mode::Run => StartupPlan::Run,
    }
}

/// What a second launch asks the running instance to do; `None` means nothing (autostart while
/// already running).
pub(crate) fn forward_request(args: &Args) -> Option<HandoffRequest> {
    match (&args.activate, args.minimized) {
        (Some(raw), _) => {
            Some(AppUri::parse(raw).map_or(HandoffRequest::Show, HandoffRequest::Activate))
        }
        (None, true) => None,
        (None, false) => Some(HandoffRequest::Show),
    }
}

pub(crate) fn run() -> ExitCode {
    let launch = Launch {
        init_phase: panic::install(),
        started: Instant::now(),
    };
    let args = match cli::parse_args(std::env::args_os().skip(1)) {
        Ok(args) => args,
        Err(error) => {
            let text = copy::usage_error(&error.to_string(), cli::usage());
            // why: from a terminal, print; from a shortcut or the protocol handler, show a dialog.
            if console::attach_parent_console(false) {
                let _ = writeln!(console::console_out(), "{text}");
            } else {
                console::fatal_dialog(copy::STARTUP_FAILED, &text);
            }
            return ExitCode::from(EXIT_USAGE);
        }
    };
    if args.debug || args.mode != Mode::Run {
        console::attach_parent_console(args.debug);
    }
    match plan(&args) {
        StartupPlan::Print(text) => {
            panic::suppress_dialogs();
            print_line(&text)
        }
        StartupPlan::SelfTest => {
            panic::suppress_dialogs();
            self_test::run()
        }
        StartupPlan::Unregister { purge } => match data_root(&args) {
            Ok((root, scope, _)) => unregister::run(&root, scope, purge),
            Err(error) => fatal(&error),
        },
        StartupPlan::Run => match data_root(&args) {
            Ok((root, scope, home)) => primary_or_forward(
                &args,
                Root {
                    path: root,
                    scope,
                    home,
                },
                &launch,
            ),
            Err(error) => fatal(&error),
        },
    }
}

fn print_line(text: &str) -> ExitCode {
    let mut out = console::console_out();
    let _ = writeln!(out, "{text}");
    ExitCode::SUCCESS
}

fn fatal(error: &dyn std::fmt::Display) -> ExitCode {
    tracing::error!(%error, "fatal");
    console::fatal_dialog(copy::STARTUP_FAILED, &error.to_string());
    ExitCode::FAILURE
}

/// The data folder and whether it is the default one.
struct Root {
    path: PathBuf,
    scope: InstanceScope,
    /// `--home` or `PRMONITOR_HOME`.
    home: Option<PathBuf>,
}

fn data_root(
    args: &Args,
) -> Result<(PathBuf, InstanceScope, Option<PathBuf>), prm_config::ConfigError> {
    let env_home = std::env::var_os(ENV_HOME).filter(|v| !v.is_empty());
    let home = args
        .home
        .clone()
        .or_else(|| env_home.clone().map(PathBuf::from));
    let cwd = std::env::current_dir().unwrap_or_default();
    let data_local = dirs::data_local_dir();
    let default_root = data_local
        .as_ref()
        .map(|dir| dir.join(prm_domain::app::DATA_DIR_NAME));
    let root = prm_config::resolve_root(args.home.clone(), env_home, data_local, &cwd)?;
    let scope = root_scope(home.as_deref(), &root, default_root.as_deref());
    Ok((root, scope, home))
}

/// An override (`home`) that names the default folder, however spelled, is still the default
/// instance, so it shares the default instance's single-instance names instead of running
/// beside it on the same files.
pub(crate) fn root_scope(
    home: Option<&std::path::Path>,
    root: &std::path::Path,
    default_root: Option<&std::path::Path>,
) -> InstanceScope {
    let is_default = home.is_none() || default_root.is_some_and(|d| is_same_root(root, d));
    if is_default {
        InstanceScope::Default
    } else {
        InstanceScope::Isolated
    }
}

/// The fallback child retries while its parent still holds the lock.
fn acquire(root: &Root, budget: Option<Duration>) -> Result<(Acquired, InstanceKey), PortError> {
    let deadline = budget.map(|b| Instant::now() + b);
    loop {
        let result = acquire_instance(&root.path, root.scope)?;
        let retry =
            matches!(result.0, Acquired::Secondary) && deadline.is_some_and(|d| Instant::now() < d);
        if !retry {
            return Ok(result);
        }
        std::thread::sleep(LOCK_RETRY);
    }
}

fn primary_or_forward(args: &Args, root: Root, launch: &Launch) -> ExitCode {
    if let Some(pid) = args.relaunched_after {
        prm_platform::process::wait_for_exit(pid, PARENT_EXIT_WAIT);
    }
    match acquire(&root, args.relaunched_after.map(|_| PARENT_EXIT_WAIT)) {
        Ok((Acquired::Primary(lock), key)) => primary(args, &root, lock, &key, launch),
        Ok((Acquired::Secondary, key)) => forward(args, &root, &key, launch),
        Err(error) => fatal(&error),
    }
}

fn forward(args: &Args, root: &Root, key: &InstanceKey, launch: &Launch) -> ExitCode {
    let Some(request) = forward_request(args) else {
        return ExitCode::SUCCESS;
    };
    match after_forward(forward_to_primary(key, &request, FORWARD_BUDGET)) {
        Next::Done => ExitCode::SUCCESS,
        Next::Fatal(reason) => fatal(&reason),
        Next::NotResponding => not_responding(),
        Next::TakeOver => take_over(args, root, launch),
    }
}

/// The primary stopped listening (it is quitting): wait for its lock and become the primary.
fn take_over(args: &Args, root: &Root, launch: &Launch) -> ExitCode {
    match acquire(root, Some(HANDOFF)) {
        Ok((Acquired::Primary(lock), key)) => {
            tracing::info!("took over from a primary that was quitting");
            primary(args, root, lock, &key, launch)
        }
        Ok((Acquired::Secondary, _)) => not_responding(),
        Err(error) => fatal(&error),
    }
}

fn not_responding() -> ExitCode {
    console::fatal_dialog(copy::STARTUP_FAILED, copy::NOT_RESPONDING);
    ExitCode::from(EXIT_NOT_RESPONDING)
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| matches!(v.trim(), "1" | "true" | "yes"))
}

/// Files, logging, crash reports and the shell identity, before any window exists.
struct Base {
    paths: AppPaths,
    clock: Arc<dyn Clock>,
    log: logging::LogHandle,
    toast_identity: IdentityStatus,
    registration_scope: RegistrationScope,
    exe: PathBuf,
}

fn register_override() -> RegisterOverride {
    if env_flag(ENV_REGISTER) {
        RegisterOverride::Forced
    } else {
        RegisterOverride::Auto
    }
}

fn base(args: &Args, root: &Root) -> Result<Base, ExitCode> {
    let paths = AppPaths::from_root(root.path.clone());
    if let Err(error) = paths.ensure_dirs() {
        console::fatal_dialog(copy::DATA_FOLDER, &error.to_string());
        return Err(ExitCode::FAILURE);
    }
    panic::set_log_dir(&paths.logs);
    // why: the local UTC offset must be read before other threads start (time crate soundness).
    let clock: Arc<dyn Clock> = Arc::new(SystemClock::new());
    let log = logging::init(&paths.logs, args.debug, std::env::var(ENV_LOG).ok());
    if !log.file_ok {
        tracing::warn!("log files unavailable; logging to the console only");
    }
    panic::prune_crash_logs(&paths.logs, CRASH_LOGS_KEPT);
    let exe =
        std::env::current_exe().unwrap_or_else(|_| PathBuf::from(prm_domain::app::APP_EXE_NAME));
    let registration = registration_scope(root.scope, BuildKind::current(), register_override());
    let toast_identity = ensure_shell_identity(&RegistrationSpec {
        exe: &exe,
        icon_png_path: &paths.icon_png,
        icon_png: assets::ICON_256_PNG,
        scope: registration,
    });
    Ok(Base {
        paths,
        clock,
        log,
        toast_identity,
        registration_scope: registration,
        exe,
    })
}

/// Activation (second launches and our own `--activate`), buffered until the service exists.
struct Activation {
    waker: Arc<UiWaker>,
    quit: Arc<AtomicBool>,
    router: Arc<ActivationRouter>,
    handoff: Option<HandoffServer>,
}

fn activation(args: &Args, key: &InstanceKey) -> Activation {
    let waker = Arc::new(UiWaker::default());
    let quit = Arc::new(AtomicBool::new(false));
    let router = Arc::new(ActivationRouter::new(Arc::clone(&waker), Arc::clone(&quit)));
    let handoff = HandoffServer::start(
        key,
        Arc::clone(&router) as Arc<dyn prm_platform::ActivationSink>,
    )
    .inspect_err(|error| tracing::warn!(%error, "activation listener disabled"))
    .ok();
    if let Some(raw) = &args.activate {
        router.activate_raw(raw);
    }
    Activation {
        waker,
        quit,
        router,
        handoff,
    }
}

/// Loaded files plus the service started on them.
struct Running {
    handle: ServiceHandle,
    events: std::sync::mpsc::Receiver<prm_service::AppEvent>,
    shell: Arc<dyn prm_platform::DesktopShell>,
    remote: bool,
    start_hidden: bool,
    window: Option<prm_config::WindowRect>,
    renderer_pref: prm_config::schema::RendererPref,
}

/// The config, the persisted state and the cached catalogs.
fn load(
    paths: &AppPaths,
) -> (
    ConfigLoad,
    StateLoad,
    std::collections::HashMap<prm_domain::OrgId, prm_domain::model::Catalog>,
) {
    let config = load_config(paths);
    let state = load_state(paths);
    let ids: Vec<_> = config
        .config
        .orgs
        .iter()
        .filter_map(|o| o.org_id().cloned())
        .collect();
    let catalogs = load_catalogs(paths, &ids);
    (config, state, catalogs)
}

fn platform(root: &Root, base: &Base, key: &InstanceKey) -> PlatformAdapters {
    adapters(&PlatformInit {
        paths: &base.paths,
        exe: &base.exe,
        scope: root.scope,
        instance_suffix: &key.suffix,
        toast_identity: base.toast_identity,
        registration_scope: base.registration_scope,
        secret_pref: SecretBackendPref::parse(std::env::var(ENV_SECRET_STORE).ok().as_deref()),
        home_override: root.home.as_deref(),
    })
}

fn start_service(
    args: &Args,
    root: &Root,
    base: &Base,
    key: &InstanceKey,
    act: &Activation,
) -> Result<Running, ExitCode> {
    let (config, state, catalogs) = load(&base.paths);
    base.log
        .apply_level(&config.config.diagnostics.log_level, args.debug);
    let PlatformAdapters {
        notifier,
        autostart,
        opener,
        shell,
        secrets,
    } = platform(root, base, key);
    let session = shell.session();
    tracing::info!(
        version = build_info::VERSION,
        sha = build_info::GIT_SHA,
        os = shell.os_description(),
        ?session,
        ?args,
        "starting"
    );
    let waker = Arc::clone(&act.waker);
    let (sink, events) = channel_sink(move || waker.wake());
    let ports = Ports {
        transport: net::deferred_transport(&config.config.network),
        secrets: Arc::new(ChunkedSecretStore::new(secrets)),
        clock: Arc::clone(&base.clock),
        events: Arc::new(sink),
    };
    let start_hidden = wiring::start_hidden(args, &config.config);
    let renderer_pref = config.config.ui.renderer;
    let window = state.state.ui.window;
    let deps = wiring::deps(
        Platform {
            notifier,
            autostart,
            opener,
        },
        ports,
        FileRepo::from_load(base.paths.clone(), &config),
    );
    let handle = prm_service::spawn(deps, wiring::initial_data(config, state, catalogs, args))
        .map_err(|e| fatal(&e))?;
    act.router.attach(Arc::new(handle.sender()));
    Ok(Running {
        handle,
        events,
        shell,
        remote: session.is_remote(),
        start_hidden,
        window,
        renderer_pref,
    })
}

fn primary(
    args: &Args,
    root: &Root,
    lock: InstanceLock,
    key: &InstanceKey,
    launch: &Launch,
) -> ExitCode {
    let base = match base(args, root) {
        Ok(base) => base,
        Err(code) => return code,
    };
    let act = activation(args, key);
    let running = match start_service(args, root, &base, key, &act) {
        Ok(running) => running,
        Err(code) => return code,
    };
    tracing::info!(ms = launch.started.elapsed().as_millis(), "service started");
    let (choice, marker) = renderer(args, &base.paths, &running);
    let result = ui_launch::run(LaunchCtx {
        choice,
        fallback_from: args.relaunched_after.map(|_| choice.kind.other()),
        start_hidden: running.start_hidden,
        remote_session: running.remote,
        window: running.window,
        paths: &base.paths,
        shell: running.shell,
        clock: base.clock,
        handle: &running.handle,
        events: running.events,
        waker: act.waker,
        quit: act.quit,
        marker: Arc::clone(&marker),
        init_phase: Arc::clone(&launch.init_phase),
    });
    let held = Held {
        handle: running.handle,
        handoff: act.handoff,
        lock,
    };
    finish(args, result, choice.kind, (held, &marker), launch)
}

/// Picks the renderer and records the attempt (so a crash before the first frame counts).
fn renderer(args: &Args, paths: &AppPaths, running: &Running) -> (RendererChoice, Arc<MarkerFile>) {
    let marker = Arc::new(MarkerFile::new(
        paths.cache.join("renderer.json"),
        build_info::VERSION,
    ));
    let previous = marker.load();
    let choice = choose_renderer(&RendererInputs {
        cli: args.renderer,
        config: running.renderer_pref,
        remote: running.remote,
        arch: std::env::consts::ARCH,
        marker: previous.as_ref(),
        version: build_info::VERSION,
    });
    if choice.reason == GLOW_FAILED {
        tracing::warn!(marker = ?previous, "OpenGL failed before on this version; using wgpu");
    }
    tracing::info!(
        renderer = choice.kind.as_arg(),
        reason = choice.reason,
        "renderer chosen"
    );
    marker.begin(choice.kind);
    (choice, marker)
}

fn finish(
    args: &Args,
    result: Result<(), eframe::Error>,
    tried: prm_ui::RendererKind,
    (held, marker): (Held, &MarkerFile),
    launch: &Launch,
) -> ExitCode {
    let first_frame_seen = !launch.init_phase.load(Ordering::Relaxed);
    match result {
        Ok(()) => shutdown::graceful(held, launch.started),
        Err(error)
            if should_relaunch(
                is_renderer_init_error(&error, tried),
                args.relaunched_after.is_some(),
                first_frame_seen,
            ) =>
        {
            tracing::warn!(%error, renderer = tried.as_arg(), "renderer failed; relaunching with the other one");
            marker.mark_failed(tried);
            let relaunch: Vec<OsString> =
                cli::relaunch_args(args, tried.other(), std::process::id());
            shutdown::fallback_relaunch(held, relaunch)
        }
        Err(error) => {
            tracing::error!(%error, first_frame_seen, "the window failed");
            let detail = error.to_string();
            if first_frame_seen {
                console::fatal_dialog(copy::CRASHED_TITLE, &copy::crashed(&detail, None));
            } else {
                console::fatal_dialog(copy::STARTUP_FAILED, &copy::graphics_failed(&detail));
            }
            let _ = shutdown::graceful(held, launch.started);
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests;
