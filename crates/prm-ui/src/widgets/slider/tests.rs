use std::path::Path;

use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Pos2, Rect, Shape, pos2, vec2};
use egui_kittest::{Harness, kittest::Queryable as _};
use prm_config::UiPrefs;

use super::*;
use crate::{
    render::policy::RenderPolicy,
    theme::{ThemeRuntime, fonts},
};

/// Half a point: the closest two things drawn "in the same place" may differ.
const TOLERANCE: f32 = 0.5;
const START: u32 = 20;
const UNIT: &str = " min";

/// One slider in a window.
struct Fx {
    theme: ThemeRuntime,
    value: u32,
    /// The slider's field, as laid out on the last frame.
    field: Rect,
}

impl Fx {
    fn new() -> Self {
        let policy = RenderPolicy {
            // why: a blinking caret is absent half the time, which a test can't wait out.
            low_power: true,
            animations: false,
        };
        Self {
            theme: ThemeRuntime::new(Path::new("no-such-themes-dir"), &UiPrefs::default(), policy),
            value: START,
            field: Rect::NOTHING,
        }
    }

    fn slot(&self) -> Rect {
        value_slot(self.field)
    }

    fn draw(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.theme.apply(&ctx);
        let theme = self.theme.current(&ctx).clone();
        let spec = SliderSpec {
            id: egui::Id::unique("test_slider"),
            name: "Refresh",
            scale: 1.0,
            unit: UNIT,
            width: SLIDER_WIDTH,
        };
        let at = ui.cursor().min;
        slider_field(ui, &theme, &mut self.value, 1..=60, &spec);
        self.field = Rect::from_min_size(at, vec2(SLIDER_WIDTH, theme.control_height()));
    }
}

fn harness() -> Harness<'static, Fx> {
    let mut h = Harness::builder()
        .with_size(vec2(400.0, 120.0))
        // why: two clicks are a double-click only within 0.3 s, and the default frame is 0.25 s.
        .with_step_dt(0.05)
        .with_context_setup(fonts::install)
        .build_ui_state(|ui, fx: &mut Fx| fx.draw(ui), Fx::new());
    h.run();
    h
}

/// The point on the value, where a double-click starts an edit.
fn on_value(h: &Harness<'_, Fx>) -> Pos2 {
    let slot = h.state().slot();
    pos2(slot.right() - 20.0, slot.center().y)
}

/// Two clicks one frame apart.
fn double_click(h: &mut Harness<'_, Fx>, at: Pos2) {
    h.event(Event::PointerMoved(at));
    h.run();
    for _ in 0..2 {
        for pressed in [true, false] {
            h.event(Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
        h.step();
    }
    h.run();
}

fn start_editing() -> Harness<'static, Fx> {
    let mut h = harness();
    let at = on_value(&h);
    double_click(&mut h, at);
    h
}

fn press(h: &mut Harness<'_, Fx>, key: Key) {
    h.key_press(key);
    h.run();
}

/// Replaces the two digits being edited with `digits`.
fn type_over(h: &mut Harness<'_, Fx>, digits: &str) {
    press(h, Key::Backspace);
    press(h, Key::Backspace);
    h.get_by_role(egui::accesskit::Role::TextInput)
        .type_text(digits);
    h.run();
}

fn flatten<'a>(shape: &'a Shape, into: &mut Vec<&'a Shape>) {
    if let Shape::Vec(shapes) = shape {
        for inner in shapes {
            flatten(inner, into);
        }
    } else {
        into.push(shape);
    }
}

/// Every shape drawn on the last frame, with the clip rect it was drawn under.
fn drawn<'a>(h: &'a Harness<'_, Fx>) -> Vec<(Rect, &'a Shape)> {
    let mut all = Vec::new();
    for clipped in &h.output().shapes {
        let mut flat = Vec::new();
        flatten(&clipped.shape, &mut flat);
        all.extend(flat.into_iter().map(|shape| (clipped.clip_rect, shape)));
    }
    all
}

/// The text drawn as exactly `text` on the last frame, as the rect it covers.
fn text_rect(h: &Harness<'_, Fx>, text: &str) -> Rect {
    let found = drawn(h).into_iter().find_map(|(_, shape)| {
        if let Shape::Text(t) = shape
            && t.galley.text() == text
        {
            Some(t.galley.rect.translate(t.pos.to_vec2()))
        } else {
            None
        }
    });
    found.unwrap_or_else(|| panic!("no text {text:?} was drawn"))
}

/// The caret on the last frame (the one line in the accent colour), the clip it was drawn
/// under, and its stroke width.
fn caret(h: &Harness<'_, Fx>) -> (Rect, Rect, f32) {
    let accent = h.state().theme.current(&h.ctx).colors.accent_base;
    let found = drawn(h).into_iter().find_map(|(clip, shape)| {
        if let Shape::LineSegment { points, stroke } = shape
            && stroke.color == accent
        {
            Some((Rect::from_two_pos(points[0], points[1]), clip, stroke.width))
        } else {
            None
        }
    });
    found.unwrap_or_else(|| panic!("no caret was drawn"))
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() <= TOLERANCE
}

fn sample_slot() -> Rect {
    Rect::from_min_size(pos2(100.0, 10.0), vec2(VALUE_WIDTH, 38.0))
}

/// The editor used to start 6 points low and its digits jumped right (the unit vanished while
/// typing), so the caret sat beside the number, halfway down.
#[test]
fn slider_edit_keeps_the_number_where_it_was() {
    let mut h = harness();
    let shown = text_rect(&h, "20 min");
    let at = on_value(&h);
    double_click(&mut h, at);

    let digits = text_rect(&h, "20");
    let unit = text_rect(&h, UNIT);
    let editor = h.get_by_role(egui::accesskit::Role::TextInput).rect();
    let report = format!("shown {shown:?}, digits {digits:?}, unit {unit:?}, editor {editor:?}");
    assert!(close(digits.center().y, shown.center().y), "{report}");
    assert!(close(editor.center().y, shown.center().y), "{report}");
    assert!(
        close(digits.right(), shown.right() - unit.width()),
        "{report}"
    );
    assert!(
        close(editor.right() + unit.width(), shown.right()),
        "{report}"
    );
    assert!(close(unit.right(), shown.right()), "{report}");
    assert!(close(unit.center().y, shown.center().y), "{report}");
    assert!(
        editor.width() >= digits.width(),
        "the editor is too narrow: {report}"
    );
}

/// The caret hung off the text line, and half of it was cut off at the editor's right edge.
#[test]
fn slider_edit_caret_sits_on_the_text_line_and_is_not_clipped() {
    let mut h = harness();
    let shown = text_rect(&h, "20 min");
    let at = on_value(&h);
    double_click(&mut h, at);
    let digits = text_rect(&h, "20");
    let (line, clip, width) = caret(&h);
    assert!(
        close(line.center().y, shown.center().y),
        "{line:?} vs {shown:?}"
    );
    // egui draws the caret 1.5 points past the glyphs above and below.
    let reach = 1.5 + TOLERANCE;
    assert!(line.top() >= digits.top() - reach, "{line:?} vs {digits:?}");
    assert!(
        line.bottom() <= digits.bottom() + reach,
        "{line:?} vs {digits:?}"
    );
    assert!(
        close(line.center().x, digits.right()),
        "{line:?} vs {digits:?}"
    );
    // why: egui itself clips the caret half a point above and below; the width is ours to keep.
    let (left, right) = (line.center().x - width / 2.0, line.center().x + width / 2.0);
    let slack = 1e-3;
    assert!(
        clip.left() <= left + slack && right <= clip.right() + slack,
        "caret {left}..{right} clipped by {clip:?}"
    );
}

#[test]
fn slider_edit_applies_the_typed_digits_on_enter() {
    let mut h = start_editing();
    type_over(&mut h, "45");
    press(&mut h, Key::Enter);
    assert_eq!(h.state().value, 45);
}

#[test]
fn edit_rect_ends_where_the_unit_begins() {
    for unit_width in [0.0, 12.0, 22.0] {
        let rect = edit_rect(sample_slot(), unit_width);
        let expected = sample_slot().right() - VALUE_INSET - unit_width;
        assert!(close(rect.right(), expected), "unit {unit_width}: {rect:?}");
    }
}

#[test]
fn edit_rect_spans_the_slot_height_from_its_left() {
    let rect = edit_rect(sample_slot(), 22.0);
    assert_eq!(
        (rect.top(), rect.bottom()),
        (sample_slot().top(), sample_slot().bottom())
    );
    assert_eq!(rect.left(), sample_slot().left());
}

#[test]
fn edit_rect_never_inverts_for_a_wide_unit() {
    let rect = edit_rect(sample_slot(), 500.0);
    assert!(rect.width() >= 0.0, "{rect:?}");
}

#[test]
fn value_pos_centres_vertically_and_insets_from_the_right() {
    let size = vec2(30.0, 18.0);
    let at = value_pos(sample_slot(), size);
    assert!(
        close(at.x + size.x, sample_slot().right() - VALUE_INSET),
        "{at:?}"
    );
    assert!(
        close(at.y + size.y / 2.0, sample_slot().center().y),
        "{at:?}"
    );
}

#[test]
fn parse_typed_reads_digits_with_or_without_the_unit() {
    let spec = SliderSpec {
        id: egui::Id::unique("parse_spec"),
        name: "Size",
        scale: 100.0,
        unit: "%",
        width: SLIDER_WIDTH,
    };
    let cases = [
        ("125", Some(1.25)),
        (" 125 % ", Some(1.25)),
        ("", None),
        ("abc", None),
        ("nan", None),
        ("inf", None),
    ];
    for (typed, expected) in cases {
        let got = parse_typed(typed, &spec);
        let same = match (got, expected) {
            (Some(a), Some(b)) => (a - b).abs() < 1e-9,
            (a, b) => a == b,
        };
        assert!(same, "{typed:?} gave {got:?}, expected {expected:?}");
    }
}
