use eframe::egui;

use super::*;

/// `#[derive(Default)]` gave a dialog width 0, which then shrank to nothing in any window.
#[test]
fn a_new_spec_is_usually_wide_and_has_no_buttons() {
    let spec = ModalSpec::new("id", "Title");

    assert_eq!(spec.width, WIDTH);
    let none: &[&str] = &[];
    assert_eq!(spec.buttons, none);
    assert!(!spec.danger && !spec.nested);
}

/// The frame's margin and stroke, on each axis.
const CHROME: egui::Vec2 = egui::vec2(2.0, 2.0);

/// `spec` placed in a window of `window` points, in a frame of `CHROME`.
fn placed(window: egui::Vec2, spec: &ModalSpec<'_>) -> Placement {
    let ctx = egui::Context::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, window)),
        ..egui::RawInput::default()
    };
    ctx.begin_pass(input);
    let placement = Placement::of(&ctx, &crate::theme::fallback(), spec, CHROME);
    let mut output = ctx.end_pass();
    output.textures_delta.clear();
    placement
}

/// The top is what keeps a dialog from moving as its content changes height, so it may depend
/// on the window and the spec's height only, never on the title, subtitle or text it holds.
#[test]
fn the_top_depends_on_the_window_and_the_body_height_only() {
    let window = egui::vec2(1280.0, 760.0);
    let plain = placed(window, &ModalSpec::new("a", "Title"));
    let named = ModalSpec {
        subtitle: Some("contoso"),
        ..ModalSpec::new("b", "A much longer title that wraps")
    };
    assert_eq!(placed(window, &named).top, plain.top);
    let fixed = ModalSpec {
        body_height: Some(MAX_BODY),
        ..ModalSpec::new("a", "Title")
    };
    assert!(placed(window, &fixed).top < plain.top);
}

#[test]
fn a_short_dialog_is_centred_for_its_nominal_height() {
    let window = egui::vec2(1280.0, 760.0);
    let spec = ModalSpec::new("a", "Title");
    let place = placed(window, &spec);
    let nominal = CHROME.y + HEAD_HEIGHT + NOMINAL_BODY;
    assert_eq!(place.top, (window.y - nominal) / 2.0);
}

/// However short the window, a dialog starts below the window's margin and its room ends above
/// the one at the foot, so it never overflows the window.
#[test]
fn the_room_below_the_top_ends_inside_the_window() {
    let specs = [
        ModalSpec::new("a", "Title"),
        ModalSpec {
            body_height: Some(MAX_BODY),
            nested: true,
            ..ModalSpec::new("b", "Title")
        },
    ];
    for spec in &specs {
        for height in [420.0, 500.0, 600.0, 760.0, 1400.0] {
            let window = egui::vec2(360.0, height);
            let place = placed(window, spec);
            let margin = fit::window_margin(height) + if spec.nested { NESTED_INSET } else { 0.0 };
            assert!(
                place.top >= margin,
                "{spec_id} at {height}",
                spec_id = spec.id
            );
            let foot = place.top + place.below + CHROME.y;
            assert!(
                foot <= height - margin + 0.01,
                "{} at {height}: foot {foot}",
                spec.id
            );
        }
    }
}

/// A fixed body is as tall as asked in a roomy window and as tall as the room in a short one,
/// but never less than `MIN_BODY`; any other body stops growing at `MAX_BODY`.
#[test]
fn a_fixed_body_keeps_its_height_until_the_window_is_too_short() {
    let cases = [
        (Some(240.0), 1000.0, 240.0),
        (Some(240.0), 150.0, 150.0),
        (Some(240.0), 10.0, MIN_BODY),
        (Some(40.0), 1000.0, MIN_BODY),
        (None, 1000.0, MAX_BODY),
        (None, 300.0, 300.0),
    ];
    for (fixed, room, expected) in cases {
        assert_eq!(body_height(fixed, room), expected, "{fixed:?} in {room} pt");
    }
}
