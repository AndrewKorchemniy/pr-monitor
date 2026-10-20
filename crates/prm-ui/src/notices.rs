//! Transient notices (bottom right): at most three, each until its TTL runs out.

use std::sync::Arc;

use prm_config::Severity;
use prm_domain::{Timestamp, auth::UserAction};
use prm_service::Notice;

const MAX_VISIBLE: usize = 3;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ShownNotice {
    pub text: Arc<str>,
    pub severity: Severity,
    pub action: Option<UserAction>,
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
            expires: now.saturating_add(notice.ttl),
        });
        if self.items.len() > MAX_VISIBLE {
            self.items.remove(0);
        }
    }

    /// A plain info notice from the UI itself ("Link copied").
    pub(crate) fn info(&mut self, text: &str, now: Timestamp) {
        self.push(
            &Notice {
                severity: Severity::Info,
                text: Arc::from(text),
                org: None,
                action: None,
                ttl: std::time::Duration::from_secs(4),
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
