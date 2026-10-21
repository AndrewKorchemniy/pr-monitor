//! The personal access token dialog: a masked field, how to create a token, and why the last one
//! was refused. The app owns the typed text and sends it to the service; this only draws.

use eframe::egui::{self, RichText};

use crate::{
    copy,
    theme::{Tone, UiTheme},
    widgets::{
        self,
        modal::{ModalOutcome, ModalSpec, modal},
    },
};

/// The dialog for the org labelled `org`, editing `input` and showing `error`. Returns what the
/// user did, and whether they asked how to create a token.
pub(crate) fn token_dialog(
    ctx: &egui::Context,
    theme: &UiTheme,
    org: &str,
    (input, error): (&mut String, Option<&str>),
) -> (ModalOutcome, bool) {
    let title = copy::pat_title(org);
    let spec = ModalSpec {
        id: "pat",
        title: &title,
        subtitle: None,
        buttons: &[copy::SAVE_TOKEN, copy::CANCEL],
        danger: false,
        width: widgets::modal::WIDTH,
    };
    let mut docs = false;
    let outcome = modal(ctx, theme, &spec, |ui| {
        let label = widgets::field_label(ui, theme, copy::PAT_LABEL);
        ui.add(
            egui::TextEdit::singleline(input)
                .password(true)
                .desired_width(f32::INFINITY),
        )
        .labelled_by(label.id);
        widgets::muted(ui, theme, copy::PAT_HELP);
        docs = ui.link(copy::HOW_TO_CREATE_TOKEN).clicked();
        if let Some(error) = error {
            ui.label(RichText::new(error).color(theme.colors.tone(Tone::Danger).fg));
        }
    });
    (outcome, docs)
}
