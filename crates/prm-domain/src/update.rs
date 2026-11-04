//! Where self-updating stands: one pure status the UI draws, and the offer a check finds. The
//! check, download and restart themselves are the `Updater` port; the service owns the schedule.

use std::sync::Arc;

/// Which step of an update failed. Only the step is shown; the detail goes to the log.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum UpdateStage {
    Check,
    Download,
    /// Handing the downloaded package to the updater for the restart.
    Apply,
}

/// Why updating is not possible at all in this run.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DisabledReason {
    /// Not installed by the installer or the portable zip (a bare exe, or `cargo run`).
    NotInstalled,
    /// Started with `--home` (or `PRMONITOR_HOME`): a side-by-side instance is never updated.
    IsolatedHome,
}

/// A newer release a check found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateOffer {
    /// The release's version, without a leading `v`.
    pub version: Arc<str>,
}

/// How the updater behaves while it swaps the files after this process exits.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ApplyMode {
    /// No windows: for a start that nobody asked for, such as autostart at login. Update.exe
    /// would otherwise be free to ask the user to close processes that lock the folder.
    Silent,
    /// Update.exe may show its progress window: for "Restart to update".
    Interactive,
}

/// The highest download progress value.
pub const DONE_PERCENT: u8 = 100;

/// What the About section shows. `auto = false` is not a status: it stops the schedule, and the
/// UI reads that setting itself.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum UpdateStatus {
    /// Nothing checked yet in this run.
    #[default]
    Idle,
    Checking,
    UpToDate,
    /// Percent downloaded, 0 to 100.
    Downloading(u8),
    /// Downloaded and waiting for a restart; the version it will become.
    Ready(Arc<str>),
    Failed(UpdateStage),
    Disabled(DisabledReason),
}

impl UpdateStatus {
    /// Progress of a download, clamped to 100.
    pub fn downloading(percent: u8) -> Self {
        Self::Downloading(percent.min(DONE_PERCENT))
    }

    /// A check may start: nothing is running, nothing is waiting for a restart, and updating is
    /// possible here.
    pub const fn can_check(&self) -> bool {
        matches!(self, Self::Idle | Self::UpToDate | Self::Failed(_))
    }

    /// The version waiting for a restart.
    pub fn ready_version(&self) -> Option<&str> {
        match self {
            Self::Ready(version) => Some(version),
            Self::Idle
            | Self::Checking
            | Self::UpToDate
            | Self::Downloading(_)
            | Self::Failed(_)
            | Self::Disabled(_) => None,
        }
    }
}

#[cfg(test)]
mod tests;
