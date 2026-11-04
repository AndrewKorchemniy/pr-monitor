//! The first steps of a primary run: files, logging and crash reports, then the shell identity.

use std::{path::PathBuf, process::ExitCode, sync::Arc};

use prm_config::AppPaths;
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

pub(super) fn base(args: &Args, root: &Root) -> Result<Base, ExitCode> {
    let paths = AppPaths::split(root.config_root.clone(), root.path.clone());
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
