//! Transient notices (top right): at most three, each until its TTL runs out.

use std::{sync::Arc, time::Duration};

use prm_config::Severity;
use prm_domain::{Timestamp, auth::UserAction};
use prm_service::Notice;

const MAX_VISIBLE: usize = 3;
/// How long a confirmation ("Link copied") stays.
const INFO_TTL: Duration = Duration::from_secs(4);
/// How long a failure the UI reports stays: long enough to read and not mistake for a
/// confirmation.
const ERROR_TTL: Duration = Duration::from_secs(12);

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ShownNotice {
    pub text: Arc<str>,
    pub severity: Severity,
    pub action: Option<UserAction>,
    /// When it appeared (it fades in from here).
    pub shown: Timestamp,
    pub expires: Timestamp,
}

#[derive(Default)]
pub(crate) struct NoticeStack {
    items: Vec<ShownNotice>,
}

impl NoticeStack {
    pub(crate) fn push(&mut self, notice: &Notice, now: Timestamp) {
        self.items.retain(|n| n.text != notice.text);
        self.items.push(ShownNotice {
            text: notice.text.clone(),
            severity: notice.severity,
            action: notice.action.clone(),
            shown: now,
            expires: now.saturating_add(notice.ttl),
        });
        if self.items.len() > MAX_VISIBLE {
            self.items.remove(0);
        }
    }

    /// A plain info notice from the UI itself ("Link copied").
    pub(crate) fn info(&mut self, text: &str, now: Timestamp) {
        self.push_text(Severity::Info, text, INFO_TTL, now);
    }

    /// Something the user asked for failed ("Changes couldn't be saved").
    pub(crate) fn error(&mut self, text: &str, now: Timestamp) {
        self.push_text(Severity::Error, text, ERROR_TTL, now);
    }

    fn push_text(&mut self, severity: Severity, text: &str, ttl: Duration, now: Timestamp) {
        self.push(
            &Notice {
                severity,
                text: Arc::from(text),
                org: None,
                action: None,
                ttl,
            },
            now,
        );
    }

    pub(crate) fn prune(&mut self, now: Timestamp) {
        self.items.retain(|n| n.expires > now);
    }

    pub(crate) fn dismiss(&mut self, index: usize) {
        if index < self.items.len() {
            self.items.remove(index);
        }
    }

    pub(crate) fn items(&self) -> &[ShownNotice] {
        &self.items
    }

    pub(crate) fn next_expiry(&self) -> Option<Timestamp> {
        self.items.iter().map(|n| n.expires).min()
    }
}

#[cfg(test)]
mod tests;
