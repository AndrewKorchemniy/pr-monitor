//! The first steps of a primary run: files, logging and crash reports, then the shell identity.

use std::{path::PathBuf, process::ExitCode, sync::Arc};

use prm_config::{
    AppPaths,
    legacy::{LegacyMove, move_legacy_folder},
};
use prm_domain::{
    app::{ENV_LOG, ENV_REGISTER},
    ports::Clock,
};
use prm_platform::{
    BuildKind, IdentityStatus, RegisterOverride, RegistrationScope, RegistrationSpec, console,
    ensure_shell_identity, registration_scope,
};
use prm_service::SystemClock;

use super::Root;
use crate::{assets, cli::Args, copy, logging, panic};

fn env_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| matches!(v.trim(), "1" | "true" | "yes"))
}

/// Files, logging, crash reports and the shell identity, before any window exists.
pub(super) struct Base {
    pub paths: AppPaths,
    pub clock: Arc<dyn Clock>,
    pub log: logging::LogHandle,
    pub toast_identity: IdentityStatus,
    pub registration_scope: RegistrationScope,
    pub exe: PathBuf,
}

fn register_override() -> RegisterOverride {
    if env_flag(ENV_REGISTER) {
        RegisterOverride::Forced
    } else {
        RegisterOverride::Auto
    }
}

fn log_legacy_move(outcome: &LegacyMove) {
    if !outcome.found {
        return;
    }
    if outcome.failed.is_empty() {
        tracing::info!(
            moved = outcome.moved.len(),
            skipped = ?outcome.skipped,
            removed = outcome.removed,
            "moved the old PRMonitor folder"
        );
    } else {
        tracing::warn!(
            moved = outcome.moved.len(),
            failed = ?outcome.failed,
            "could not move all of the old PRMonitor folder; will retry on the next start"
        );
    }
}

pub(super) fn base(args: &Args, root: &Root) -> Result<Base, ExitCode> {
    // why: before anything reads settings or opens a log; the outcome is logged once logging is up.
    let legacy = root
        .legacy
        .as_deref()
        .map(|old| move_legacy_folder(old, &root.config_root, &root.path));
    let paths = root.app_paths();
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
    if let Some(outcome) = &legacy {
        log_legacy_move(outcome);
    }
    panic::prune_crash_logs(&paths.logs, panic::CRASH_LOGS_KEPT);
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
