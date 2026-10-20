use eframe::egui;
use prm_domain::{
    PrStatus, classify::VoteTally, diff::ChangeKind, model::VoteKind, testing::person,
};

use super::*;
use crate::theme::fonts;

/// Every static string lays out with the embedded fonts alone (no tofu on machines without the
/// system fallback).
#[test]
fn copy_strings_covered_by_embedded_fonts() {
    let ctx = egui::Context::default();
    ctx.set_fonts(fonts::definitions());
    let mut output = ctx.run_ui(egui::RawInput::default(), |_| {});
    output.textures_delta.clear();
    let theme = crate::theme::fallback();
    for text in ALL {
        for font in [&theme.fonts.body, &theme.fonts.mono] {
            let covered = ctx.fonts_mut(|f| f.has_glyphs(font, text));
            assert!(covered, "missing glyphs in {font:?} for {text:?}");
        }
    }
}

#[test]
fn reviewed_tooltip_lists_only_present_votes() {
    let tally = VoteTally {
        approved: 2,
        waiting: 1,
        ..VoteTally::default()
    };
    assert_eq!(
        status_tooltip(PrStatus::Reviewed, tally, 0),
        "Reviewed: 2 approved · 1 waiting for author"
    );
    assert_eq!(
        status_tooltip(PrStatus::ActiveComments, tally, 1),
        "1 open thread waiting on the author."
    );
}

#[test]
fn change_notes_are_short_and_name_people() {
    let by = person(3, "Ana Lee");
    assert_eq!(
        change_short(&ChangeKind::ChangesRequested {
            by: by.clone(),
            vote: VoteKind::Rejected
        }),
        "Ana Lee rejected"
    );
    assert_eq!(
        change_short(&ChangeKind::NewComments {
            count: 2,
            authors: std::sync::Arc::from([]),
            snippet: None
        }),
        "2 new comments"
    );
    assert_eq!(
        change_short(&ChangeKind::NewVote {
            by,
            vote: VoteKind::Approved
        }),
        "Ana Lee approved"
    );
}

#[test]
fn every_suggested_action_has_a_label() {
    use prm_domain::{
        app::DocLink,
        auth::{SettingsSection, UserAction},
    };
    let actions = [
        UserAction::SignInWithMicrosoft,
        UserAction::SignInEntra,
        UserAction::EnterPat,
        UserAction::ChooseSignIn,
        UserAction::Retry,
        UserAction::ContactAdmin {
            message: std::sync::Arc::from("x"),
        },
        UserAction::OpenDocs(DocLink::CreatePat),
        UserAction::OpenSettings(SettingsSection::Scope),
    ];
    for action in &actions {
        assert!(
            !user_action_label(action).is_empty(),
            "{action:?} has no label"
        );
    }
}
