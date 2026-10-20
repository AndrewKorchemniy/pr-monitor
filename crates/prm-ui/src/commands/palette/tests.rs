use prm_domain::testing::org;

use super::*;
use crate::testing::{reviewed, sample_rows};

fn sources(rows: &[PrRow]) -> PaletteSources<'_> {
    PaletteSources {
        sign_in: vec![(org("contoso"), "Contoso".to_owned())],
        themes: vec![("vellum".to_owned(), "Vellum".to_owned())],
        rows,
    }
}

fn titles(items: &[(PaletteItem, String)]) -> Vec<&str> {
    items.iter().map(|(_, t)| t.as_str()).collect()
}

#[test]
fn prefixes_and_word_starts_outscore_scattered_matches() {
    let prefix = score("ref", "Refresh now").expect("match");
    let word = score("now", "Refresh now").expect("match");
    let scattered = score("eow", "Refresh now").expect("match");
    assert!(prefix > word, "{prefix} > {word}");
    assert!(word > scattered, "{word} > {scattered}");
    assert_eq!(score("xyz", "Refresh now"), None);
    assert_eq!(score("", "anything"), Some(0));
    assert!(score("REF", "refresh").is_some(), "case-insensitive");
}

#[test]
fn queries_rank_commands_sign_ins_and_themes_together() {
    let rows = sample_rows();
    let state = PaletteState {
        query: "refresh".to_owned(),
        ..PaletteState::default()
    };
    let items = build_items(&state, &sources(&rows));
    assert_eq!(
        items.first().map(|(i, _)| i),
        Some(&PaletteItem::Command(CommandId::RefreshNow))
    );
    let state = PaletteState {
        query: "contoso".to_owned(),
        ..PaletteState::default()
    };
    assert_eq!(
        titles(&build_items(&state, &sources(&rows))),
        vec!["Sign in to Contoso"]
    );
    let state = PaletteState {
        query: "vellum".to_owned(),
        ..PaletteState::default()
    };
    assert_eq!(
        titles(&build_items(&state, &sources(&rows))),
        vec!["Theme: Vellum"]
    );
}

#[test]
fn an_empty_query_puts_recent_commands_first() {
    let rows = sample_rows();
    let state = PaletteState {
        recent: vec![CommandId::ShortcutsShow, CommandId::RefreshNow],
        ..PaletteState::default()
    };
    let items = build_items(&state, &sources(&rows));
    assert_eq!(items[0].0, PaletteItem::Command(CommandId::ShortcutsShow));
    assert_eq!(items[1].0, PaletteItem::Command(CommandId::RefreshNow));
}

#[test]
fn bang_numbers_jump_to_pull_requests() {
    let rows = sample_rows();
    let state = PaletteState {
        query: "!103".to_owned(),
        ..PaletteState::default()
    };
    let items = build_items(&state, &sources(&rows));
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].0, PaletteItem::GoTo(reviewed()));
    assert!(items[0].1.contains("Add retry"));
    let state = PaletteState {
        query: "#10".to_owned(),
        ..PaletteState::default()
    };
    assert_eq!(build_items(&state, &sources(&rows)).len(), 4);
}
