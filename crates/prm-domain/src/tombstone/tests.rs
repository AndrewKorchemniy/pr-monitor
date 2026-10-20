use super::*;
use crate::testing::{PrBuilder, key, record, ts};

fn event(kind: ChangeKind, at_secs: i64) -> ChangeEvent {
    let rec = record(PrBuilder::new(key("contoso", 1, 1)).build(), vec![]);
    ChangeEvent {
        pr: PrHeader::from_record(&rec),
        kind,
        at: ts(at_secs),
    }
}

fn closed(outcome: CloseOutcome, at_secs: i64) -> ChangeEvent {
    event(ChangeKind::Closed { outcome }, at_secs)
}

#[test]
fn tombstone_added_on_close() {
    let mut tombstones = Tombstones::new();
    apply_tombstones(
        &mut tombstones,
        &[closed(CloseOutcome::Completed, 100)],
        ts(100),
    );
    assert_eq!(tombstones.len(), 1);
    assert_eq!(
        tombstones.values().next().unwrap().outcome,
        CloseOutcome::Completed
    );
}

#[test]
fn tombstone_removed_on_reactivate() {
    let mut tombstones = Tombstones::new();
    apply_tombstones(
        &mut tombstones,
        &[closed(CloseOutcome::Abandoned, 100)],
        ts(100),
    );
    apply_tombstones(
        &mut tombstones,
        &[event(ChangeKind::Reactivated, 200)],
        ts(200),
    );
    assert!(tombstones.is_empty());
}

#[test]
fn tombstone_expires_after_7_days() {
    let ttl_secs = i64::try_from(TOMBSTONE_TTL.as_secs()).unwrap();
    let mut tombstones = Tombstones::new();
    apply_tombstones(&mut tombstones, &[closed(CloseOutcome::Unknown, 0)], ts(0));

    apply_tombstones(&mut tombstones, &[], ts(ttl_secs - 1));
    assert_eq!(tombstones.len(), 1);

    apply_tombstones(&mut tombstones, &[], ts(ttl_secs));
    assert!(tombstones.is_empty());
}
