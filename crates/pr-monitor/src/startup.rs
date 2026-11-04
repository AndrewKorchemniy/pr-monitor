//! Startup, in a fixed order: the panic hook, arguments, data root, the single-instance lock (a
//! second launch forwards and exits before any logging or GPU work, or takes over from a primary
//! that is quitting), files and logging, shell identity, activation, config, the service, the
//! renderer, the window; then shutdown or a relaunch.

use std::{
    ffi::OsString,
    io::Write as _,
    panic::AssertUnwindSafe,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

use prm_config::{AppPaths, Roots};
use prm_domain::{
    app::{DATA_DIR_NAME, ENV_HOME, LEGACY_DATA_DIR_NAME},
    ports::{PortError, Updater},
    uri::AppUri,
};
use prm_platform::{
    Acquired, ForwardOutcome, HandoffRequest, InstanceKey, InstanceLock, InstanceScope,
    acquire_instance, console, forward_to_primary, is_same_root,
};
use prm_ui::RendererKind;

use crate::{
    build_info,
    cli::{self, Args, Mode},
    copy,
    panic::{self, Phase},
    renderer::{
        MarkerFile, RendererChoice, RendererInputs, RendererReason, choose_renderer,
        is_renderer_init_error,
    },
    self_test,
    shutdown::{self, Held},
    ui_launch::{self, LaunchCtx},
    unregister,
    update::{self, Boot, VelopackUpdater},
};

mod base;
mod service;

use self::{
    base::base,
    service::{Launching, Running, activation, start_service},
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
/// Between attempts at the instance lock while another process still holds it.
pub(crate) const LOCK_RETRY: Duration = Duration::from_millis(100);

/// What this process knows from its first line on.
struct Launch {
    started: Instant,
    boot: Boot,
    /// True until the first frame (from `panic::install`).
    init_phase: Arc<AtomicBool>,
}

/// Whether this process is the renderer fallback that a failed launch started.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Attempt {
    First,
    Fallback,
}

impl Attempt {
    const fn of(args: &Args) -> Self {
        if args.relaunched_after.is_some() {
            Self::Fallback
        } else {
            Self::First
        }
    }
}

/// A renderer that failed to start gets one relaunch with the other, but only before the first
/// frame: after that the window worked and a relaunch would hide a real bug.
pub(crate) const fn should_relaunch(init_error: bool, attempt: Attempt, phase: Phase) -> bool {
    init_error && matches!(attempt, Attempt::First) && matches!(phase, Phase::Init)
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

pub(crate) fn run(boot: Boot) -> ExitCode {
    let launch = Launch {
        boot,
        init_phase: panic::install(),
        started: Instant::now(),
    };
    let args = match cli::parse_args(std::env::args_os().skip(1)) {
        Ok(args) => args,
        Err(error) => return usage_error(&error),
    };
    if args.debug || args.mode != Mode::Run {
        console::attach_parent_console(args.debug);
    }
    match args.mode {
        Mode::Version => {
            panic::suppress_dialogs();
            print_line(&build_info::version_line())
        }
        Mode::Help => {
            panic::suppress_dialogs();
            print_line(&cli::usage())
        }
        Mode::SelfTest => {
            panic::suppress_dialogs();
            self_test::run()
        }
        Mode::Unregister { purge } => unregister(&args, purge),
        Mode::Run => match data_root(&args) {
            Ok(root) => primary_or_forward(&args, &root, &launch),
            Err(error) => fatal(&error),
        },
    }
}

fn run_unregister(root: &Root, purge: bool) -> ExitCode {
    // why: the purge guard needs the user's real home folder, not the `--home` override the data
    // root came from.
    unregister::run(root, purge, dirs::home_dir().as_deref())
}

/// `--unregister`.
fn unregister(args: &Args, purge: bool) -> ExitCode {
    match data_root(args) {
        Ok(root) => run_unregister(&root, purge),
        Err(error) => fatal(&error),
    }
}

/// Velopack's uninstall hook: the same cleanup, keeping the data folder. It runs inside the
/// uninstaller, which stops it after 30 seconds, so it never shows a dialog. Windows only: Velopack
/// has no uninstall hook on Linux or macOS.
#[cfg(windows)]
pub(crate) fn uninstall_cleanup() {
    if let Ok(root) = data_root(&Args::default()) {
        let _ = run_unregister(&root, false);
    }
}

fn usage_error(error: &cli::ArgError) -> ExitCode {
    let text = copy::usage_error(&error.to_string(), &cli::usage());
    // why: from a terminal, print; from a shortcut or the protocol handler, show a dialog.
    if console::attach_parent_console(false) {
        let _ = writeln!(console::console_out(), "{text}");
    } else {
        console::fatal_dialog(copy::STARTUP_FAILED, &text);
    }
    ExitCode::from(EXIT_USAGE)
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Root {
    /// The local folder: state, cache, logs, instance lock.
    pub(crate) path: PathBuf,
    /// Where `config.toml`, `keymap.toml` and `themes` live (equal to `path` for `--home`).
    pub(crate) config_root: PathBuf,
    pub(crate) scope: InstanceScope,
    /// The resolved absolute root when it is not the default one (from `--home` or
    /// `PRMONITOR_HOME`), repeated in the launch-at-login command.
    home: Option<PathBuf>,
    /// The old single data folder, for the default roots only.
    pub(crate) legacy: Option<PathBuf>,
}

impl Root {
    /// `resolved` holds the absolute roots; `explicit` when they came from `--home` or
    /// `PRMONITOR_HOME`. An explicit root that names the default folder, however spelled, is the
    /// default instance under the default spelling, so one folder never gets two instance locks.
    fn new(resolved: Roots, explicit: bool, default: Option<Roots>) -> Self {
        let home = explicit.then_some(resolved.local.as_path());
        let default_local = default.as_ref().map(|d| d.local.as_path());
        match root_scope(home, &resolved.local, default_local) {
            InstanceScope::Default => {
                let roots = default.unwrap_or(resolved);
                Self {
                    path: roots.local,
                    config_root: roots.config,
                    scope: InstanceScope::Default,
                    home: None,
                    legacy: None,
                }
            }
            InstanceScope::Isolated => Self {
                home: Some(resolved.local.clone()),
                path: resolved.local,
                config_root: resolved.config,
                scope: InstanceScope::Isolated,
                legacy: None,
            },
        }
    }
}

fn data_root(args: &Args) -> Result<Root, prm_config::ConfigError> {
    let env_home = std::env::var_os(ENV_HOME).filter(|v| !v.is_empty());
    let explicit = args.home.is_some() || env_home.is_some();
    let cwd = std::env::current_dir().unwrap_or_default();
    let (data_local, config_dir) = (dirs::data_local_dir(), dirs::config_dir());
    let default_roots = data_local
        .as_ref()
        .zip(config_dir.as_ref())
        .map(|(local, config)| Roots {
            config: config.join(DATA_DIR_NAME),
            local: local.join(DATA_DIR_NAME),
        });
    let legacy = data_local.as_ref().map(|l| l.join(LEGACY_DATA_DIR_NAME));
    let resolved =
        prm_config::resolve_root(args.home.clone(), env_home, data_local, config_dir, &cwd)?;
    let mut root = Root::new(resolved, explicit, default_roots);
    if root.scope == InstanceScope::Default {
        root.legacy = legacy;
    }
    Ok(root)
}

/// An override (`home`) that names the default folder, however spelled, is still the default
/// instance, so it shares the default instance's single-instance names instead of running
/// beside it on the same files.
pub(crate) fn root_scope(
    home: Option<&Path>,
    root: &Path,
    default_root: Option<&Path>,
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

fn primary_or_forward(args: &Args, root: &Root, launch: &Launch) -> ExitCode {
    if let Some(pid) = args.relaunched_after {
        prm_platform::process::wait_for_exit(pid, PARENT_EXIT_WAIT);
    }
    match acquire(root, args.relaunched_after.map(|_| PARENT_EXIT_WAIT)) {
        Ok((Acquired::Primary(lock), key)) => primary(args, root, lock, &key, launch),
        Ok((Acquired::Secondary, key)) => forward(args, root, &key, launch),
        Err(error) => fatal(&error),
    }
}

fn forward(args: &Args, root: &Root, key: &InstanceKey, launch: &Launch) -> ExitCode {
    let Some(request) = forward_request(args) else {
        return ExitCode::SUCCESS;
    };
    match forward_to_primary(key, &request, FORWARD_BUDGET) {
        ForwardOutcome::Delivered => ExitCode::SUCCESS,
        ForwardOutcome::Rejected(reason) => fatal(&reason),
        ForwardOutcome::NotResponding => not_responding(),
        // Nobody is listening: the primary is quitting (or crashed), so wait for the lock.
        ForwardOutcome::NoPrimary => take_over(args, root, launch),
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

/// Hands a downloaded update to Update.exe; `true` means this process must exit now. It runs
/// after the lock (a second launch must never stop the running primary) and the log (a failed
/// apply is recorded), but before the handoff listener, the service and any window. The log
/// files are written unbuffered, so nothing is left to flush when the process exits.
fn handed_to_updater(updater: &dyn Updater, boot: Boot) -> bool {
    let restart_args: Vec<OsString> = std::env::args_os().skip(1).collect();
    update::apply_pending_at_start(updater, boot, &restart_args)
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
    let updater: Arc<dyn Updater> = Arc::new(VelopackUpdater::new(root.scope));
    if handed_to_updater(updater.as_ref(), launch.boot) {
        return ExitCode::SUCCESS;
    }
    let act = activation(args, key);
    let launching = Launching {
        args,
        root,
        key,
        updater,
    };
    let running = match start_service(&launching, &base, &act) {
        Ok(running) => running,
        Err(code) => return code,
    };
    tracing::info!(ms = launch.started.elapsed().as_millis(), "service started");
    let (choice, marker) = renderer(args, &base.paths, &running);
    let held = Held {
        handle: running.handle,
        handoff: act.handoff,
        lock,
    };
    let ctx = LaunchCtx {
        choice,
        fallback_from: args.relaunched_after.map(|_| choice.kind.other()),
        start_hidden: running.start_hidden,
        remote_session: running.remote,
        window: running.window,
        paths: &base.paths,
        shell: running.shell,
        clock: base.clock,
        handle: &held.handle,
        events: running.events,
        waker: act.waker,
        quit: act.quit,
        marker: Arc::clone(&marker),
        init_phase: Arc::clone(&launch.init_phase),
    };
    // why: a panic on the UI thread (the hook already reported it) must still stop the service,
    // so its final persist runs, and release the handoff listener and the lock.
    let Ok(result) = std::panic::catch_unwind(AssertUnwindSafe(|| ui_launch::run(ctx))) else {
        tracing::error!("the window panicked; shutting down");
        let _ = shutdown::graceful(held, launch.started);
        return ExitCode::FAILURE;
    };
    let end = RunEnd {
        held,
        exe: &base.exe,
        marker: &marker,
        tried: choice.kind,
        result,
    };
    finish(args, end, launch)
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
    if matches!(choice.reason, RendererReason::GlowFailed) {
        tracing::warn!(marker = ?previous, "OpenGL failed before on this version; using wgpu");
    }
    tracing::info!(
        renderer = choice.kind.as_arg(),
        reason = choice.reason.as_str(),
        "renderer chosen"
    );
    marker.begin(previous.as_ref(), choice.kind);
    (choice, marker)
}

/// How the window run ended, and what the run still holds.
struct RunEnd<'a> {
    held: Held,
    /// This process's exe, for the relaunch.
    exe: &'a Path,
    marker: &'a MarkerFile,
    tried: RendererKind,
    result: Result<(), eframe::Error>,
}

fn finish(args: &Args, end: RunEnd<'_>, launch: &Launch) -> ExitCode {
    let phase = Phase::of(&launch.init_phase);
    let RunEnd {
        held,
        exe,
        marker,
        tried,
        result,
    } = end;
    match result {
        Ok(()) => shutdown::graceful(held, launch.started),
        Err(error)
            if should_relaunch(
                is_renderer_init_error(&error, tried),
                Attempt::of(args),
                phase,
            ) =>
        {
            tracing::warn!(%error, renderer = tried.as_arg(), "renderer failed; relaunching with the other one");
            marker.mark_failed(tried);
            let relaunch: Vec<OsString> =
                cli::relaunch_args(args, tried.other(), std::process::id());
            shutdown::fallback_relaunch(held, exe, relaunch)
        }
        Err(error) => {
            tracing::error!(%error, ?phase, "the window failed");
            // why: let go of the lock, the listener and the service before the modal dialog, so
            // a launch meanwhile starts afresh instead of reaching a process with no window.
            let _ = shutdown::graceful(held, launch.started);
            let detail = error.to_string();
            match phase {
                Phase::Running => {
                    console::fatal_dialog(copy::CRASHED_TITLE, &copy::crashed(&detail, None));
                }
                Phase::Init => {
                    console::fatal_dialog(copy::STARTUP_FAILED, &copy::graphics_failed(&detail));
                }
            }
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests;
