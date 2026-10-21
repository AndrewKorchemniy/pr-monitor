//! Desktop notifications via notify-rust. On Linux a later toast with the same tag replaces
//! the earlier one; neither Linux nor macOS retracts (a retracted toast stays until it times
//! out or is dismissed), and macOS cannot replace either.

use std::sync::Mutex;

use prm_domain::{app::APP_DISPLAY_NAME, notify::Notification, ports::PortError};
use prm_service::ports::{Notifier, NotifierCapability};

use crate::common::failed;

/// Most recent notification ids kept for replacement (oldest dropped first).
#[cfg(not(target_os = "macos"))]
const MAX_HANDLES: usize = 64;

#[derive(Default)]
pub(crate) struct NotifyRustNotifier {
    /// (tag, notification id), oldest first.
    #[cfg(not(target_os = "macos"))]
    ids: Mutex<Vec<(String, u32)>>,
    #[cfg(target_os = "macos")]
    ids: Mutex<()>,
}

#[cfg(not(target_os = "macos"))]
impl NotifyRustNotifier {
    fn ids(&self) -> std::sync::MutexGuard<'_, Vec<(String, u32)>> {
        self.ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn remember(&self, tag: &str, id: u32) {
        let mut ids = self.ids();
        ids.retain(|(t, _)| t != tag);
        ids.push((tag.to_owned(), id));
        if ids.len() > MAX_HANDLES {
            ids.remove(0);
        }
    }
}

impl Notifier for NotifyRustNotifier {
    fn show(&self, notification: &Notification) -> Result<(), PortError> {
        let mut toast = notify_rust::Notification::new();
        toast
            .appname(APP_DISPLAY_NAME)
            .summary(&notification.title)
            .body(&notification.body);
        #[cfg(not(target_os = "macos"))]
        {
            let previous = self
                .ids()
                .iter()
                .find(|(tag, _)| tag == &notification.tag)
                .map(|(_, id)| *id);
            if let Some(id) = previous {
                toast.id(id);
            }
            let handle = toast.show().map_err(failed)?;
            self.remember(&notification.tag, handle.id());
            Ok(())
        }
        #[cfg(target_os = "macos")]
        {
            let _ = &self.ids;
            toast.show().map(drop).map_err(failed)
        }
    }

    /// Not supported: closing needs the D-Bus handle (a connection per toast), which we do not
    /// keep. The id stays remembered, so the next toast with this tag still replaces the old one
    /// instead of stacking a second.
    fn retract(&self, _tag: &str, _group: &str) -> Result<(), PortError> {
        Ok(())
    }

    fn capability(&self) -> NotifierCapability {
        #[cfg(not(target_os = "macos"))]
        let capability = if notify_rust::get_server_information().is_ok() {
            NotifierCapability::Enabled
        } else {
            NotifierCapability::Unsupported
        };
        #[cfg(target_os = "macos")]
        let capability = NotifierCapability::Unknown;
        capability
    }
}
