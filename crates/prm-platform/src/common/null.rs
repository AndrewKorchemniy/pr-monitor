//! Do-nothing adapters for when the real one cannot start: the notifier on Windows, launch at
//! login on macOS and Linux.

#[cfg(windows)]
use prm_domain::notify::Notification;
use prm_domain::ports::PortError;
#[cfg(not(windows))]
use prm_service::ports::{Autostart, AutostartState};
#[cfg(windows)]
use prm_service::ports::{Notifier, NotifierCapability};

/// Shows nothing; used when the notification thread cannot start.
#[cfg(windows)]
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct NullNotifier;

#[cfg(windows)]
impl Notifier for NullNotifier {
    fn show(&self, _notification: &Notification) -> Result<(), PortError> {
        Ok(())
    }

    fn retract(&self, _tag: &str, _group: &str) -> Result<(), PortError> {
        Ok(())
    }

    fn capability(&self) -> NotifierCapability {
        NotifierCapability::Unsupported
    }
}

/// Reports launch at login as unsupported; used when the OS entry cannot be built.
#[cfg(not(windows))]
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct NullAutostart;

#[cfg(not(windows))]
impl Autostart for NullAutostart {
    fn state(&self) -> Result<AutostartState, PortError> {
        Ok(AutostartState::Unsupported)
    }

    fn set_enabled(&self, _enabled: bool) -> Result<(), PortError> {
        Err(super::unsupported())
    }
}
