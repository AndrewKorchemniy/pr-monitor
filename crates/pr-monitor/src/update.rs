//! Self-updating with Velopack: its hooks at the top of `main`, the `Updater` port over its
//! `UpdateManager`, and the one place a pending update is applied at start.
//!
//! Packages come from the GitHub releases of this repository. Velopack picks the channel baked
//! into the package (`win-x64`, `linux-arm64`, ...), so nothing here names an architecture.

use std::{
    cell::Cell,
    ffi::OsString,
    sync::{Arc, Mutex, PoisonError, mpsc},
};

use prm_domain::{
    app::{ENV_UPDATE_FEED, REPO_URL},
    ports::{PortError, PortErrorKind, Updater},
    update::{ApplyMode, DONE_PERCENT, DisabledReason, UpdateOffer},
};
use prm_platform::InstanceScope;
use velopack::{
    Error as VelopackError, UpdateCheck, UpdateInfo, UpdateManager, VelopackApp,
    sources::{FileSource, GithubSource, UpdateSource},
};

/// What the Velopack hooks learned before our own startup began.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Boot {
    /// Velopack just restarted this process after applying an update. A pending update is then
    /// not applied again, so a package that cannot be applied can never loop.
    pub restarted: bool,
}

/// Runs Velopack's startup hooks. It must be the first thing `main` does: the installer and
/// updater start the app with `--veloapp-*` flags that it answers and then exits on, before our
/// argument parser (which rejects unknown flags) sees them.
///
/// Pending updates are *not* applied here (`set_auto_apply_on_startup(false)`): that would also
/// happen in a second launch, and applying stops every process in the app folder, including
/// the running primary. `startup` applies it once it holds the instance lock.
pub(crate) fn boot() -> Boot {
    let restarted = Cell::new(false);
    let mut app = VelopackApp::build()
        .set_auto_apply_on_startup(false)
        .on_restarted(|_| restarted.set(true));
    #[cfg(windows)]
    {
        // why: Update.exe runs this on uninstall: the same cleanup as `--unregister`, keeping the
        // user's data folder. The process AUMID is not set here: `ensure_shell_identity` sets it on
        // every primary launch, and the Start-menu shortcut gets it from vpk's `--aumid`.
        app = app.on_before_uninstall_fast_callback(|_| crate::startup::uninstall_cleanup());
    }
    // why: Velopack's own log lines from here are dropped (our logging starts later); Update.exe
    // logs to %LocalAppData%elopackelopack_AndrewKorchemniy.PRMonitor.log.
    app.run();
    Boot {
        restarted: restarted.get(),
    }
}

struct Ready {
    manager: UpdateManager,
    /// What the last check found, which `download` needs and the port's offer does not carry.
    found: Mutex<Option<Box<UpdateInfo>>>,
}

/// The port over Velopack.
pub(crate) struct VelopackUpdater {
    /// The manager, or why there is none.
    backing: Result<Ready, DisabledReason>,
}

impl VelopackUpdater {
    /// Updating needs an installed copy (Setup or the portable zip) and the default data
    /// folder: a side-by-side `--home` instance is never updated.
    pub(crate) fn new(scope: InstanceScope) -> Self {
        let backing = match scope {
            InstanceScope::Isolated => Err(DisabledReason::IsolatedHome),
            InstanceScope::Default => match UpdateManager::new_boxed(source(), None, None) {
                Ok(manager) => Ok(Ready {
                    manager,
                    found: Mutex::new(None),
                }),
                Err(error) => {
                    tracing::info!(%error, "not installed by Velopack; updating is off");
                    Err(DisabledReason::NotInstalled)
                }
            },
        };
        Self { backing }
    }

    fn ready(&self) -> Result<&Ready, PortError> {
        self.backing
            .as_ref()
            .map_err(|_| PortError::new(PortErrorKind::Unsupported, "updating is off in this run"))
    }
}

/// The releases of this repository; in debug builds a local feed folder when
/// `PRMONITOR_UPDATE_FEED` names one.
fn source() -> Box<dyn UpdateSource> {
    if cfg!(debug_assertions)
        && let Some(dir) = std::env::var_os(ENV_UPDATE_FEED)
    {
        tracing::info!(?dir, "updating from a local feed");
        return Box::new(FileSource::new(dir));
    }
    // Only published, non-prerelease releases: a draft stays invisible until it goes live.
    Box::new(GithubSource::new(REPO_URL, None, false))
}

/// The offer a newer release makes.
fn offer_of(info: &UpdateInfo) -> UpdateOffer {
    UpdateOffer {
        version: info.TargetFullRelease.Version.as_str().into(),
    }
}

/// Velopack reports percent as `i16`.
fn percent(value: i16) -> u8 {
    u8::try_from(value.max(0)).map_or(DONE_PERCENT, |percent| percent.min(DONE_PERCENT))
}

fn port_error(error: &VelopackError) -> PortError {
    PortError::new(PortErrorKind::Failed, error.to_string())
}

impl Updater for VelopackUpdater {
    fn availability(&self) -> Result<(), DisabledReason> {
        self.backing.as_ref().map(|_| ()).map_err(|reason| *reason)
    }

    fn pending(&self) -> Option<Arc<str>> {
        let ready = self.ready().ok()?;
        let asset = ready.manager.get_update_pending_restart()?;
        Some(asset.Version.as_str().into())
    }

    fn check(&self) -> Result<Option<UpdateOffer>, PortError> {
        let ready = self.ready()?;
        let check = ready
            .manager
            .check_for_updates()
            .map_err(|e| port_error(&e))?;
        // No release, or none newer, is "nothing to do", not an error.
        let found = match check {
            UpdateCheck::UpdateAvailable(info) => Some(info),
            UpdateCheck::NoUpdateAvailable | UpdateCheck::RemoteIsEmpty => None,
        };
        let offer = found.as_deref().map(offer_of);
        *ready.found.lock().unwrap_or_else(PoisonError::into_inner) = found;
        Ok(offer)
    }

    fn download(
        &self,
        offer: &UpdateOffer,
        progress: &(dyn Fn(u8) + Sync),
    ) -> Result<(), PortError> {
        let ready = self.ready()?;
        let info = ready
            .found
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .filter(|info| info.TargetFullRelease.Version.as_str() == &*offer.version)
            .ok_or_else(|| PortError::new(PortErrorKind::NotFound, "no such update was found"))?;
        let (tx, rx) = mpsc::channel::<i16>();
        // why: Velopack reports progress through a channel while it blocks; forward it from
        // this thread so the caller's callback runs here.
        let result = std::thread::scope(|scope| {
            let worker = scope.spawn(|| ready.manager.download_updates(&info, Some(tx)));
            for value in rx {
                progress(percent(value));
            }
            worker.join()
        });
        match result {
            Ok(downloaded) => downloaded.map_err(|e| port_error(&e)),
            Err(_) => Err(PortError::new(
                PortErrorKind::Failed,
                "download task panicked",
            )),
        }
    }

    fn apply_on_exit(&self, restart_args: &[OsString], mode: ApplyMode) -> Result<(), PortError> {
        let ready = self.ready()?;
        let asset = ready
            .manager
            .get_update_pending_restart()
            .ok_or_else(|| PortError::new(PortErrorKind::NotFound, "no update is waiting"))?;
        ready
            .manager
            .wait_exit_then_apply_updates(&asset, mode == ApplyMode::Silent, true, restart_args)
            .map_err(|e| port_error(&e))
    }
}

/// At start, as the primary instance: hands a downloaded update to Update.exe, which waits for
/// this process to exit, swaps the files and starts the app again with the same arguments.
/// `true` means the caller must now exit without opening a window. Applied whether or not
/// automatic checks are on: the package is only there because the user, or an earlier automatic
/// check, fetched it.
pub(crate) fn apply_pending_at_start(
    updater: &dyn Updater,
    boot: Boot,
    restart_args: &[OsString],
) -> bool {
    if boot.restarted {
        return false;
    }
    let Some(version) = updater.pending() else {
        return false;
    };
    tracing::info!(%version, "applying the downloaded update");
    // why: this start is usually a login, not something the user asked for.
    match updater.apply_on_exit(restart_args, ApplyMode::Silent) {
        Ok(()) => true,
        Err(error) => {
            tracing::warn!(error = %error.message, "the downloaded update could not be applied");
            false
        }
    }
}

#[cfg(test)]
mod tests;
