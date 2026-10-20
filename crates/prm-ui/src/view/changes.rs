//! What changed on each PR, for the change bar's tooltip and the peek ("2 new comments · 14m").

use std::{collections::HashMap, sync::Arc};

use prm_domain::{PrKey, Timestamp, diff::ChangeEvent};

use crate::copy;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChangeNote {
    pub text: Arc<str>,
    pub at: Timestamp,
}

/// The newest note per PR (session only; not persisted).
#[derive(Clone, Debug, Default)]
pub(crate) struct ChangeLog {
    notes: HashMap<PrKey, ChangeNote>,
}

impl ChangeLog {
    /// Keeps the highest-priority event per PR in this batch.
    pub(crate) fn record(&mut self, events: &[ChangeEvent]) {
        let mut best: HashMap<&PrKey, &ChangeEvent> = HashMap::new();
        for event in events {
            let slot = best.entry(&event.pr.key).or_insert(event);
            if event.kind.sort_order() < slot.kind.sort_order() {
                *slot = event;
            }
        }
        for (key, event) in best {
            let note = ChangeNote {
                text: Arc::from(copy::change_short(&event.kind)),
                at: event.at,
            };
            self.notes.insert(key.clone(), note);
        }
    }

    pub(crate) fn note(&self, key: &PrKey) -> Option<&ChangeNote> {
        self.notes.get(key)
    }

    pub(crate) fn forget(&mut self, key: &PrKey) {
        self.notes.remove(key);
    }
}

#[cfg(test)]
mod tests;
