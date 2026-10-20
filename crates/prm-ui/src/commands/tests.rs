use std::collections::HashSet;

use super::*;

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
