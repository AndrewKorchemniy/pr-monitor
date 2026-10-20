use eframe::egui::{Event, RawInput};

use super::*;

fn chord(s: &str) -> KeyChord {
    s.parse()
        .unwrap_or_else(|e: ChordError| panic!("bad chord {}", e.0))
}

fn key_event(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

/// Runs `dispatch` for one frame whose input is `events`.
fn dispatch(
    keymap: &Keymap,
    scope: ActiveScope,
    events: Vec<Event>,
) -> (Vec<CommandId>, Vec<Event>) {
    let ctx = egui::Context::default();
    let input = RawInput {
        events,
        ..RawInput::default()
    };
    ctx.begin_pass(input);
    let fired = keymap.dispatch(&ctx, scope);
    let left = ctx.input(|i| i.events.clone());
    ctx.end_pass().textures_delta.clear();
    (fired, left)
}

#[test]
fn chords_parse_and_print_round_trip() {
    for text in ["Ctrl+Shift+C", "F5", "Space", "/", "Ctrl+Comma", "Alt+K"] {
        let parsed = chord(text);
        assert_eq!(chord(&parsed.to_string()), parsed, "{text}");
    }
    assert_eq!(chord("ctrl+k").to_string(), "Ctrl+K");
    assert_eq!(chord("Cmd+K"), chord("Ctrl+K"));
    assert_eq!(chord("Alt+ArrowDown").to_string(), "Alt+Down");
    assert!("Hyper+K".parse::<KeyChord>().is_err());
    assert!("Ctrl+NotAKey".parse::<KeyChord>().is_err());
}

#[test]
fn the_default_keymap_has_no_conflicts() {
    let keymap = Keymap::defaults();
    assert!(keymap.conflicts().is_empty(), "{:?}", keymap.conflicts());
    assert_eq!(keymap.hint(CommandId::RefreshNow), Some(chord("F5")));
}

#[test]
fn overrides_rebind_unbind_and_report_mistakes() {
    let toml = r#"
        [bindings]
        "refresh.now" = ["Ctrl+Shift+R"]
        "palette.open" = []
        "no.such" = ["F9"]
        "settings.open" = ["Hyper+X"]
    "#;
    let (keymap, issues) = Keymap::defaults().with_overrides(toml);
    assert_eq!(
        keymap.hint(CommandId::RefreshNow),
        Some(chord("Ctrl+Shift+R"))
    );
    assert_eq!(keymap.hint(CommandId::PaletteOpen), None);
    assert_eq!(issues.len(), 2, "{issues:?}");
    let (_, broken) = Keymap::defaults().with_overrides("not toml [");
    assert_eq!(broken.len(), 1);
}

#[test]
fn rebinding_onto_a_used_chord_is_a_conflict() {
    let (keymap, issues) =
        Keymap::defaults().with_overrides("[bindings]\n\"refresh.now\" = [\"Ctrl+K\"]\n");
    assert!(!keymap.conflicts().is_empty(), "the clash is reported");
    assert!(issues.iter().any(|i| i.0.contains("used twice")));
}

#[test]
fn keymap_ctrl_c_maps_event_copy_in_table_scope() {
    let keymap = Keymap::defaults();
    let (fired, left) = dispatch(&keymap, ActiveScope::Table, vec![Event::Copy]);
    assert_eq!(fired, vec![CommandId::PrCopyLink]);
    assert!(!left.contains(&Event::Copy), "the copy event is consumed");
    let (fired, left) = dispatch(&keymap, ActiveScope::TextInput, vec![Event::Copy]);
    assert_eq!(fired, Vec::<CommandId>::new());
    assert!(
        left.contains(&Event::Copy),
        "text fields keep their own copy"
    );
}

#[test]
fn scopes_gate_bindings_and_navigation_repeats() {
    let keymap = Keymap::defaults();
    let down = || key_event(Key::ArrowDown, Modifiers::NONE);
    let (fired, _) = dispatch(&keymap, ActiveScope::Table, vec![down(), down()]);
    assert_eq!(fired, vec![CommandId::NavDown, CommandId::NavDown]);
    let (fired, _) = dispatch(&keymap, ActiveScope::TextInput, vec![down()]);
    assert_eq!(fired, Vec::<CommandId>::new());
    let f5 = key_event(Key::F5, Modifiers::NONE);
    let (fired, _) = dispatch(&keymap, ActiveScope::TextInput, vec![f5.clone()]);
    assert_eq!(fired, vec![CommandId::RefreshNow]);
    let (fired, _) = dispatch(&keymap, ActiveScope::Overlay, vec![f5]);
    assert_eq!(fired, Vec::<CommandId>::new());
}

#[test]
fn shifted_chords_win_over_their_unshifted_key() {
    let keymap = Keymap::defaults();
    let shift_c = key_event(Key::C, Modifiers::COMMAND | Modifiers::SHIFT);
    let (fired, _) = dispatch(&keymap, ActiveScope::Table, vec![shift_c]);
    assert_eq!(fired, vec![CommandId::PrCopyMarkdown]);
}
