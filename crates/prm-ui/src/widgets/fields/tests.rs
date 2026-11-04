use eframe::egui;

use super::*;

const ID: &str = "autofocus_test_field";

fn id() -> egui::Id {
    egui::Id::unique(ID)
}

/// Runs one frame, drawing the field when `drawn`.
fn frame(ctx: &egui::Context, drawn: bool, autofocus: bool) {
    let theme = crate::theme::fallback();
    let mut text = String::new();
    let field = Field::new(id(), "hint").autofocus(autofocus);
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        if drawn {
            text_field(ui, &theme, &mut text, &field);
        }
    });
    output.textures_delta.clear();
}

fn focused(ctx: &egui::Context) -> bool {
    ctx.memory(|m| m.has_focus(id()))
}

fn blur(ctx: &egui::Context) {
    ctx.memory_mut(|m| m.surrender_focus(id()));
}

/// The field asks once when it appears, never again while it stays, and again after it was gone.
#[test]
fn an_autofocus_field_asks_for_focus_when_it_appears() {
    let ctx = egui::Context::default();
    frame(&ctx, true, true);
    assert!(focused(&ctx), "the first frame");

    blur(&ctx);
    frame(&ctx, true, true);
    frame(&ctx, true, true);
    assert!(!focused(&ctx), "a field that stays is not given focus back");

    frame(&ctx, false, true);
    frame(&ctx, true, true);
    assert!(focused(&ctx), "it was gone for a frame, so it is new again");
}

#[test]
fn a_field_without_autofocus_never_asks() {
    let ctx = egui::Context::default();
    frame(&ctx, true, false);
    assert!(!focused(&ctx));
}
