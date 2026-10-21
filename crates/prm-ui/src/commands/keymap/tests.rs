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

/// Pressing "/" focused the filter and the same press's text event typed "/" into it.
#[test]
fn keymap_slash_focuses_the_filter_without_typing_into_it() {
    let keymap = Keymap::defaults();
    let slash = vec![
        key_event(Key::Slash, Modifiers::NONE),
        Event::Text("/".to_owned()),
    ];
    let (fired, left) = dispatch(&keymap, ActiveScope::Table, slash);
    assert_eq!(fired, vec![CommandId::FilterFocus]);
    assert!(
        !left.iter().any(|e| matches!(e, Event::Text(_))),
        "the typed slash is dropped: {left:?}"
    );
}

#[test]
fn keymap_text_typed_into_a_field_survives_a_global_shortcut() {
    let keymap = Keymap::defaults();
    let events = vec![
        key_event(Key::F5, Modifiers::NONE),
        Event::Text("a".to_owned()),
    ];
    let (fired, left) = dispatch(&keymap, ActiveScope::TextInput, events);
    assert_eq!(fired, vec![CommandId::RefreshNow]);
    assert!(left.contains(&Event::Text("a".to_owned())));
}

/// An unmodified chord on a command with no default keys became Global, so typing "t" in the
/// filter cycled the theme.
#[test]
fn keymap_override_without_a_modifier_stays_out_of_text_fields() {
    let toml = r#"
        [bindings]
        "theme.cycle" = ["T"]
        "view.sort.title" = ["Ctrl+Alt+S"]
        "notifications.test" = ["F9"]
    "#;
    let (keymap, issues) = Keymap::defaults().with_overrides(toml);
    assert!(issues.is_empty(), "{issues:?}");
    let t = || key_event(Key::T, Modifiers::NONE);
    let (fired, _) = dispatch(&keymap, ActiveScope::TextInput, vec![t()]);
    assert_eq!(fired, Vec::<CommandId>::new());
    let (fired, _) = dispatch(&keymap, ActiveScope::Table, vec![t()]);
    assert_eq!(fired, vec![CommandId::ThemeCycle]);
    let ctrl_alt_s = key_event(Key::S, Modifiers::COMMAND | Modifiers::ALT);
    let (fired, _) = dispatch(&keymap, ActiveScope::TextInput, vec![ctrl_alt_s]);
    assert_eq!(fired, vec![CommandId::SortTitle]);
    let f9 = key_event(Key::F9, Modifiers::NONE);
    let (fired, _) = dispatch(&keymap, ActiveScope::TextInput, vec![f9]);
    assert_eq!(fired, vec![CommandId::NotificationsTest]);
}

#[test]
fn keymap_overrides_keep_shifted_chords_first() {
    let (keymap, _) = Keymap::defaults().with_overrides("[bindings]\n\"pr.mark_seen\" = [\"M\"]\n");
    let shift_m = key_event(Key::M, Modifiers::SHIFT);
    let (fired, _) = dispatch(&keymap, ActiveScope::Table, vec![shift_m]);
    assert_eq!(fired, vec![CommandId::PrMarkAllSeen]);
}

#[test]
fn shifted_chords_win_over_their_unshifted_key() {
    let keymap = Keymap::defaults();
    let shift_c = key_event(Key::C, Modifiers::COMMAND | Modifiers::SHIFT);
    let (fired, _) = dispatch(&keymap, ActiveScope::Table, vec![shift_c]);
    assert_eq!(fired, vec![CommandId::PrCopyMarkdown]);
}
