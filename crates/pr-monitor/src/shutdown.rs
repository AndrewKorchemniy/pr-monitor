//! Ending a run: the service persists and stops, handoff stops, the instance lock is released. Or,
//! when the renderer could not start, the same followed by a relaunch with the other renderer.

use std::{ffi::OsString, process::ExitCode, time::Duration};

use prm_platform::{InstanceLock, HandoffServer};
use prm_service::ServiceHandle;

const SERVICE_STOP: Duration = Duration::from_secs(3);
const SERVICE_STOP_BEFORE_RELAUNCH: Duration = Duration::from_secs(1);
const HANDOFF_STOP: Duration = Duration::from_millis(500);

/// What a run holds until it ends.
pub(crate) struct Held {
    pub handle: ServiceHandle,
    pub handoff: Option<HandoffServer>,
    pub lock: InstanceLock,
}

fn release(held: Held, service_wait: Duration) {
    if !held.handle.shutdown(service_wait) {
        tracing::warn!("the service didn't stop in time");
    }
    if let Some(handoff) = held.handoff
        && !handoff.stop(HANDOFF_STOP)
    {
        tracing::warn!("the activation listener didn't stop in time");
    }
    drop(held.lock);
}

pub(crate) fn graceful(held: Held, started: std::time::Instant) -> ExitCode {
    release(held, SERVICE_STOP);
    tracing::info!(
        total_ms = started.elapsed().as_millis(),
        "shutdown complete"
    );
    ExitCode::SUCCESS
}

/// Starts a fresh process with `args` once this one let go of everything.
pub(crate) fn fallback_relaunch(held: Held, args: Vec<OsString>) -> ExitCode {
    release(held, SERVICE_STOP_BEFORE_RELAUNCH);
    let spawned =
        std::env::current_exe().and_then(|exe| std::process::Command::new(exe).args(&args).spawn());
    match spawned {
        Ok(_) => {
            tracing::info!(?args, "relaunched with the other renderer");
            ExitCode::SUCCESS
        }
        Err(error) => {
            tracing::error!(%error, "relaunch failed");
            prm_platform::console::fatal_dialog(
                "PR Monitor couldn't start",
                &crate::copy::graphics_failed(&error.to_string()),
            );
            ExitCode::FAILURE
        }
    }
}
