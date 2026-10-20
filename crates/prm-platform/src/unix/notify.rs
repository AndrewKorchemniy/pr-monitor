//! Desktop notifications via notify-rust. On Linux a later toast with the same tag replaces
//! the earlier one; macOS cannot replace or retract.

use std::sync::Mutex;

use prm_domain::{
    app::APP_DISPLAY_NAME,
    notify::Notification,
    ports::{PortError, PortErrorKind},
};
use prm_service::ports::{Notifier, NotifierCapability};

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

fn failed(error: impl std::fmt::Display) -> PortError {
    PortError::new(PortErrorKind::Failed, error.to_string())
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

    fn retract(&self, tag: &str, _group: &str) -> Result<(), PortError> {
        #[cfg(not(target_os = "macos"))]
        self.ids().retain(|(t, _)| t != tag);
        #[cfg(target_os = "macos")]
        let _ = tag;
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
