//! Tombstones remember recently closed PRs so a reopened one is reported as "reopened",
//! not "new".

use std::{collections::BTreeMap, time::Duration};

use serde::{Deserialize, Serialize};

use crate::{
    diff::{ChangeEvent, ChangeKind},
    ids::PrKey,
    snapshot::PrHeader,
    time::Timestamp,
};

/// How long a closed PR is remembered.
pub const TOMBSTONE_TTL: Duration = Duration::from_hours(7 * 24);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseOutcome {
    Completed,
    Abandoned,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tombstone {
    pub closed_at: Timestamp,
    pub outcome: CloseOutcome,
    pub header: PrHeader,
}

pub type Tombstones = BTreeMap<PrKey, Tombstone>;

/// Records `Closed` events, forgets PRs that came back (`Reactivated`), and drops tombstones
/// older than [`TOMBSTONE_TTL`].
pub fn apply_tombstones(tombstones: &mut Tombstones, events: &[ChangeEvent], now: Timestamp) {
    for event in events {
        match &event.kind {
            ChangeKind::Closed { outcome } => {
                let tombstone = Tombstone {
                    closed_at: event.at,
                    outcome: *outcome,
                    header: event.pr.clone(),
                };
                tombstones.insert(event.pr.key.clone(), tombstone);
            }
            ChangeKind::Reactivated => {
                tombstones.remove(&event.pr.key);
            }
            ChangeKind::NewPr
            | ChangeKind::ReadyForReview
            | ChangeKind::Reviewed { .. }
            | ChangeKind::NeedsReReview
            | ChangeKind::ChangesRequested { .. }
            | ChangeKind::NewVote { .. }
            | ChangeKind::NewComments { .. }
            | ChangeKind::CommentsResolved
            | ChangeKind::NewPush => {}
        }
    }
    tombstones.retain(|_, t| now.since(t.closed_at) < TOMBSTONE_TTL);
}

#[cfg(test)]
mod tests;
