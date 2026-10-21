use std::collections::HashSet;

use super::*;
use crate::copy;

#[test]
fn pause_labels_state_the_pause_duration() {
    const SECS_PER_HOUR: u64 = 3600;
    assert_eq!(PAUSE.as_secs() % SECS_PER_HOUR, 0, "labels speak in hours");
    let hours = PAUSE.as_secs() / SECS_PER_HOUR;
    let unit = if hours == 1 { "hour" } else { "hours" };
    let duration = format!("{hours} {unit}");
    for label in [
        CommandId::NotificationsPause.title(),
        copy::TRAY_PAUSE,
        copy::PAUSE_HOUR,
    ] {
        assert!(label.ends_with(&duration), "{label:?} says {duration}");
    }
}

#[test]
fn every_command_has_one_spec_with_a_unique_key() {
    let mut keys = HashSet::new();
    let mut ids = HashSet::new();
    for spec in COMMANDS {
        assert!(keys.insert(spec.key), "duplicate key {}", spec.key);
        assert!(ids.insert(spec.id), "duplicate spec for {:?}", spec.id);
        assert!(!spec.title.is_empty(), "{} has no title", spec.key);
        assert_eq!(CommandId::from_key(spec.key), Some(spec.id));
        assert_eq!(spec.id.spec(), spec);
    }
    assert_eq!(CommandId::from_key("no.such.command"), None);
}

#[test]
fn every_default_chord_parses() {
    for spec in COMMANDS {
        for (chord, _) in spec.keys {
            assert!(
                chord.parse::<keymap::KeyChord>().is_ok(),
                "{} has a bad chord {chord}",
                spec.key
            );
        }
    }
}
