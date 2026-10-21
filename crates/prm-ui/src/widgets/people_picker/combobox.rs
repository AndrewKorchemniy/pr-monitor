//! The search combobox: a field whose keys act on a dropdown of matches under it.

use eframe::egui::{self, Key, Modifiers, Rect, RichText, pos2, vec2};
use prm_domain::{OrgId, Person, person::PersonCandidate};
use prm_service::Command;

use super::{MIN_QUERY, PeoplePicker, Pick, Search, Step, offered};
use crate::{
    actions::UiAction,
    copy,
    frame::FrameCx,
    theme::{Tone, UiTheme},
    widgets::{
        controls,
        fields::{self, Field},
    },
};

/// The combobox: its keys act first (so the field never sees them), then the field and its
/// dropdown are drawn. A pick keeps keyboard focus in the field for the next name.
pub(super) fn search(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    picker: &mut PeoplePicker,
    org: &OrgId,
    people: &mut Vec<Person>,
) {
    let theme = cx.theme.clone();
    let field_id = egui::Id::unique(("people", picker.id));
    let popup_id = field_id.with("matches");
    let keys = take_keys(ui, field_id, picker.open);
    if let Some(candidate) = keyed(picker, keys, people) {
        add(cx, picker, (org, people), &candidate);
    }
    let (left, width) = (ui.cursor().left(), ui.available_width());
    let field = Field::new(field_id, copy::ADD_PEOPLE).keeps_escape(picker.open);
    let response = keep_focus(ui, (popup_id, picker.open), |ui| {
        fields::text_field(ui, &theme, &mut picker.query, &field)
    });
    if response.changed()
        && let Some(request) = picker.typed()
    {
        cx.push(UiAction::Send(Command::SearchPeople {
            org: org.clone(),
            query: picker.query.trim().to_owned(),
            request,
        }));
    }
    let anchor = Rect::from_min_size(
        pos2(left, response.rect.top()),
        vec2(width, response.rect.height()),
    );
    let popup = (popup_id, anchor, &response);
    if let Some(candidate) = dropdown(ui, &theme, picker, popup, people) {
        add(cx, picker, (org, people), &candidate);
        ui.memory_mut(|m| m.request_focus(field_id));
    }
}

/// Draws the field so that pressing on its open dropdown does not take the field's focus (as
/// egui's own completion popup does).
fn keep_focus<R>(
    ui: &mut egui::Ui,
    (popup, open): (egui::Id, bool),
    draw: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let ctx = ui.ctx().clone();
    let layer = egui::LayerId::new(egui::Order::Foreground, popup);
    let pointer_layer = ctx
        .pointer_interact_pos()
        .and_then(|pos| ctx.layer_id_at(pos));
    if !open || pointer_layer != Some(layer) {
        return draw(ui);
    }
    let before = ctx.options(|o| o.input_options.surrender_focus_on);
    ctx.options_mut(|o| o.input_options.surrender_focus_on = egui::SurrenderFocusOn::Never);
    let drawn = draw(ui);
    ctx.options_mut(|o| o.input_options.surrender_focus_on = before);
    drawn
}

/// The keys the combobox takes from its field this frame.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
struct Keys {
    up: usize,
    down: usize,
    enter: bool,
    escape: bool,
    tab: bool,
}

/// While the field has focus: Down (to open the dropdown or move down), and while the dropdown
/// is open also Up, Enter and Esc. Otherwise the keys keep their usual meaning, so Esc can still
/// close the dialog around the picker.
fn take_keys(ui: &egui::Ui, field: egui::Id, open: bool) -> Keys {
    if !ui.memory(|m| m.has_focus(field)) {
        return Keys::default();
    }
    ui.input_mut(|i| {
        let down = i.count_and_consume_key(Modifiers::NONE, Key::ArrowDown);
        if !open {
            return Keys {
                down,
                ..Keys::default()
            };
        }
        Keys {
            up: i.count_and_consume_key(Modifiers::NONE, Key::ArrowUp),
            down,
            enter: i.consume_key(Modifiers::NONE, Key::Enter),
            escape: i.consume_key(Modifiers::NONE, Key::Escape),
            tab: i.key_pressed(Key::Tab),
        }
    })
}

/// Applies the keys: Esc or Tab closes the dropdown, Down opens it, the arrows move the
/// highlight, and Enter returns the highlighted match.
fn keyed(picker: &mut PeoplePicker, keys: Keys, people: &[Person]) -> Option<PersonCandidate> {
    if keys.escape || keys.tab {
        picker.close();
        return None;
    }
    if !picker.open {
        picker.open = keys.down > 0 && picker.has_rows();
        return None;
    }
    let count = offered(&picker.results, people).count();
    for _ in 0..keys.down {
        picker.step(Step::Down, count);
    }
    for _ in 0..keys.up {
        picker.step(Step::Up, count);
    }
    if keys.enter {
        picker.highlighted(people).cloned()
    } else {
        None
    }
}

/// The dropdown under the field, as wide as it; returns a clicked match. A click anywhere
/// else (the field included) closes it, so it never ends up under the dialog it belongs to.
fn dropdown(
    ui: &egui::Ui,
    theme: &UiTheme,
    picker: &mut PeoplePicker,
    (id, anchor, field): (egui::Id, Rect, &egui::Response),
    people: &[Person],
) -> Option<PersonCandidate> {
    if !picker.has_rows() {
        picker.close();
    }
    let width = anchor.width();
    let shown = egui::Popup::new(id, ui.ctx().clone(), anchor, field.layer_id)
        .open_memory(Some(egui::SetOpenCommand::Bool(picker.open)))
        .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
        .gap(controls::MENU_GAP)
        .frame(controls::menu_frame(theme))
        .width(width)
        .anchor_widget(field.id)
        .show(|ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing.y = 0.0;
            rows(ui, theme, picker, people)
        })?;
    // why: egui closes the popup itself on Esc when the field does not have the key.
    if shown.response.clicked_elsewhere() || !egui::Popup::is_id_open(ui.ctx(), id) {
        picker.close();
    }
    shown.inner
}

/// The dropdown's lines: "Adding…" or why a pick failed, then the hint, the matches and how
/// the search stands. Returns a clicked match.
fn rows(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    picker: &mut PeoplePicker,
    people: &[Person],
) -> Option<PersonCandidate> {
    if picker.resolving.is_some() {
        note(ui, theme, copy::ADDING, false);
    }
    if let Some(error) = &picker.error {
        note(ui, theme, error, true);
    }
    let typed = picker.query.trim().chars().count();
    if typed == 0 {
        return None;
    }
    if typed < MIN_QUERY {
        note(ui, theme, copy::SEARCH_MIN, false);
        return None;
    }
    let clicked = matches(ui, theme, picker, people);
    let none_left = offered(&picker.results, people).next().is_none();
    match &picker.search {
        Search::Searching(_) => note(ui, theme, copy::SEARCHING, false),
        Search::Done(empty) if none_left => note(ui, theme, empty, false),
        Search::Failed(message) => note(ui, theme, message, true),
        Search::Idle | Search::Done(_) => {}
    }
    clicked
}

/// The offered matches with the highlighted one filled; moving the pointer over a row
/// highlights it, so the keys and the mouse agree.
fn matches(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    picker: &mut PeoplePicker,
    people: &[Person],
) -> Option<PersonCandidate> {
    let count = offered(&picker.results, people).count();
    let lit = picker.highlight.min(count.saturating_sub(1));
    let moved = ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO);
    let (mut clicked, mut hovered) = (None, None);
    for (index, candidate) in offered(&picker.results, people).enumerate() {
        let detail = candidate
            .unique_name
            .as_deref()
            .or_else(|| candidate.group.then_some(copy::GROUP_TAG));
        let text = (candidate.name.as_ref(), detail);
        let row = controls::menu_item_detail(ui, theme, text, index == lit);
        if row.hovered() && moved {
            hovered = Some(index);
        }
        if row.clicked() {
            clicked = Some(candidate.clone());
        }
    }
    if let Some(index) = hovered {
        picker.highlight = index;
    }
    clicked
}

/// A dropdown line that is not a match: a hint, progress, or (in the danger tone) a failure.
fn note(ui: &mut egui::Ui, theme: &UiTheme, text: &str, failed: bool) {
    let color = if failed {
        theme.colors.tone(Tone::Danger).fg
    } else {
        theme.colors.text_secondary
    };
    let label = RichText::new(text)
        .font(theme.fonts.small.clone())
        .color(color);
    egui::Frame::NONE
        .inner_margin(note_margin(theme))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(egui::Label::new(label).wrap());
        });
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "spacing tokens are small whole points"
)]
fn note_margin(theme: &UiTheme) -> egui::Margin {
    let m = &theme.tokens.metrics;
    egui::Margin::symmetric(m.space.lg as i8, m.space.sm as i8)
}

/// Adds the chosen match now, or asks the service to resolve it (the person then arrives as
/// an event and settings adds it).
fn add(
    cx: &mut FrameCx<'_>,
    picker: &mut PeoplePicker,
    (org, people): (&OrgId, &mut Vec<Person>),
    candidate: &PersonCandidate,
) {
    match picker.pick(candidate) {
        Pick::Add(person) => {
            if !people.iter().any(|p| p.id == person.id) {
                people.push(person);
            }
        }
        Pick::Resolve(request, candidate) => cx.push(UiAction::Send(Command::ResolvePerson {
            org: org.clone(),
            candidate,
            request,
        })),
    }
}
