use prm_domain::testing::org;

use super::*;

struct Draft {
    input: String,
    error: Option<Arc<str>>,
    org: Option<OrgId>,
}

impl Draft {
    fn typed(org: &OrgId) -> Self {
        Self {
            input: "secret-token".to_owned(),
            error: Some(Arc::from("Token rejected")),
            org: Some(org.clone()),
        }
    }

    fn reset(&mut self, open: Option<&OrgId>) {
        reset_pat_draft(
            PatDraft {
                input: &mut self.input,
                error: &mut self.error,
                org: &mut self.org,
            },
            open,
        );
    }
}

#[test]
fn pat_draft_survives_while_its_dialog_stays_open() {
    let contoso = org("contoso");
    let mut draft = Draft::typed(&contoso);
    draft.reset(Some(&contoso));
    assert_eq!(draft.input, "secret-token");
    assert!(draft.error.is_some());
}

/// A cancelled token stayed in memory for the session, and reopening the dialog from Settings
/// showed the old text and error.
#[test]
fn pat_draft_is_wiped_when_the_dialog_closes_or_switches_org() {
    let contoso = org("contoso");
    let mut draft = Draft::typed(&contoso);
    draft.reset(None);
    assert_eq!(draft.input, "");
    assert_eq!(draft.error, None);
    assert_eq!(draft.org, None);
    let mut draft = Draft::typed(&contoso);
    let fabrikam = org("fabrikam");
    draft.reset(Some(&fabrikam));
    assert_eq!(draft.input, "");
    assert_eq!(draft.org, Some(fabrikam));
}

/// The anchor restore assumed rows were `row_height` apart, but `egui_extras` adds the vertical
/// item spacing, so the selected row jumped on refresh.
#[test]
fn row_pitch_includes_the_vertical_item_spacing() {
    let theme = crate::theme::fallback();
    let metrics = &theme.tokens.metrics;
    let style = crate::theme::adapter::style(&theme, theme.policy);
    assert_eq!(
        row_pitch(&theme),
        metrics.row.row_height + style.spacing.item_spacing.y
    );
}

/// The peek's close button did nothing once the selection had gone: Space and the button ran
/// the selected row's commands, which stop without a row.
#[test]
fn the_peek_closes_without_a_selection_and_opens_only_on_one() {
    let cases = [
        ((true, false), false),
        ((true, true), false),
        ((false, true), true),
        ((false, false), false),
    ];
    for ((open, selected), after) in cases {
        assert_eq!(
            commands::toggled_peek(open, selected),
            after,
            "open {open}, selected {selected}"
        );
    }
}
