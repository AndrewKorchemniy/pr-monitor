//! The org tabs at the top of Organizations: one tab per org with its sign-in state as a dot,
//! then Add organization, on one line over a baseline. When they are wider than the page, Add
//! organization shrinks to its "+" at the line's end and the tabs scroll sideways before it,
//! keeping the selected tab in view.

use std::sync::Arc;

use eframe::egui::{self, Galley, Rect, Sense, Stroke, pos2, vec2};
use prm_config::OrgProfile;
use prm_service::AuthStateView;

use crate::{
    copy,
    frame::FrameCx,
    screens::settings::{Editor, Pickers, SettingsState},
    theme::{
        Tone, UiTheme,
        icons::{self, Icon},
    },
    widgets::controls,
};

/// The diameter of a tab's sign-in dot, in points.
const DOT: f32 = 7.0;
/// The side of Add organization's "+", in points.
const PLUS: f32 = 12.0;

/// An org's tab, measured: its label, cut short to fit the page, and its sign-in state.
struct Tab<'a> {
    label: String,
    state: Option<&'a AuthStateView>,
    galley: Arc<Galley>,
}

pub(super) fn org_tabs(ui: &mut egui::Ui, cx: &FrameCx<'_>, st: &mut SettingsState) {
    let theme = cx.theme.clone();
    let room = ui.available_width();
    let tabs_room = room - add_room(ui, &theme, true);
    let tabs: Vec<Tab<'_>> = st
        .draft
        .orgs
        .iter()
        .enumerate()
        .map(|(index, org)| measure(ui, &theme, cx, org, (index == st.org, tabs_room)))
        .collect();
    let compact = strip_width(ui, &theme, &tabs) > room;
    let (picked, add) = strip(ui, &theme, &tabs, (st.org, compact));
    if add {
        st.new_org_url.clear();
        st.url_error = false;
        st.editing = Some(Editor::AddOrg);
    }
    if let Some(index) = picked
        && index != st.org
    {
        st.org = index;
        st.pickers = Pickers::default();
    }
}

fn measure<'a>(
    ui: &egui::Ui,
    theme: &UiTheme,
    cx: &FrameCx<'a>,
    org: &OrgProfile,
    (selected, room): (bool, f32),
) -> Tab<'a> {
    let state = org
        .org_id()
        .and_then(|id| cx.state.org(id))
        .map(|o| &o.auth.state);
    let label = org.display_label().to_string();
    let (color, font) = tab_style(theme, selected);
    // why: a long org name is cut short rather than running out of a narrow page.
    let mut job = egui::text::LayoutJob::simple_singleline(label.clone(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(room - tab_chrome(theme));
    let galley = ui.painter().layout_job(job);
    Tab {
        label,
        state,
        galley,
    }
}

/// The text colour and font of a tab: stronger when selected.
fn tab_style(theme: &UiTheme, selected: bool) -> (egui::Color32, egui::FontId) {
    let c = &theme.colors;
    if selected {
        (c.text_primary, theme.fonts.body_strong.clone())
    } else {
        (c.text_secondary, theme.fonts.body.clone())
    }
}

/// A tab's width besides its label: the padding and the dot.
fn tab_chrome(theme: &UiTheme) -> f32 {
    let m = &theme.tokens.metrics;
    m.space.lg + DOT + m.space.sm + m.space.lg
}

/// Add organization's width: its "+" and, unless `compact`, its label.
fn add_width(ui: &egui::Ui, theme: &UiTheme, compact: bool) -> f32 {
    let m = &theme.tokens.metrics;
    let label = if compact {
        0.0
    } else {
        m.space.sm + add_label(ui, theme).size().x
    };
    m.space.lg + PLUS + label + m.space.lg
}

fn add_label(ui: &egui::Ui, theme: &UiTheme) -> Arc<Galley> {
    ui.painter().layout_no_wrap(
        copy::ADD_ORG.to_owned(),
        theme.fonts.body.clone(),
        theme.colors.text_secondary,
    )
}

/// The width of the whole line with Add organization at full width.
fn strip_width(ui: &egui::Ui, theme: &UiTheme, tabs: &[Tab<'_>]) -> f32 {
    let m = &theme.tokens.metrics;
    let tabs: f32 = tabs
        .iter()
        .map(|tab| tab_chrome(theme) + tab.galley.size().x + m.space.xs)
        .sum();
    tabs + m.space.sm + add_width(ui, theme, false)
}

/// The room between Add organization's gap and the line's start that the tabs may take.
fn add_room(ui: &egui::Ui, theme: &UiTheme, compact: bool) -> f32 {
    let m = &theme.tokens.metrics;
    m.space.xs + m.space.sm + add_width(ui, theme, compact)
}

/// The tabs, on a line that scrolls sideways when they do not all fit, then Add organization
/// (so it stays at the right end when they scroll), over a baseline. Returns the clicked tab,
/// and whether Add organization was clicked.
fn strip(
    ui: &mut egui::Ui,
    theme: &UiTheme,
    tabs: &[Tab<'_>],
    (selected, compact): (usize, bool),
) -> (Option<usize>, bool) {
    let m = &theme.tokens.metrics;
    let room = ui.available_width() - add_room(ui, theme, compact);
    let row = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = m.space.xs;
        let picked = egui::ScrollArea::horizontal()
            .id_salt("org_tabs")
            .max_width(room)
            .auto_shrink([true, true])
            .show(ui, |ui| {
                ui.horizontal(|ui| line(ui, theme, tabs, selected)).inner
            })
            .inner;
        ui.add_space(m.space.sm);
        (picked, add_tab(ui, theme, compact).clicked())
    });
    let stroke = Stroke::new(m.stroke.hairline, theme.colors.border_default);
    let rect = row.response.rect;
    ui.painter()
        .hline(rect.left()..=ui.max_rect().right(), rect.bottom(), stroke);
    row.inner
}

/// The tabs, left to right. Returns the clicked one.
fn line(ui: &mut egui::Ui, theme: &UiTheme, tabs: &[Tab<'_>], selected: usize) -> Option<usize> {
    let mut picked = None;
    for (index, tab) in tabs.iter().enumerate() {
        let response = org_tab(ui, theme, tab, index == selected);
        if index == selected {
            keep_in_view(ui, &response, index);
        }
        if response.clicked() {
            picked = Some(index);
        }
    }
    picked
}

/// Scrolls the selected tab into view once each time the selection changes (an added org's tab
/// ends the line), and otherwise leaves the line where it was scrolled.
fn keep_in_view(ui: &egui::Ui, tab: &egui::Response, index: usize) {
    let id = egui::Id::unique("org_tabs_shown");
    if ui.data(|d| d.get_temp::<usize>(id)) == Some(index) {
        return;
    }
    ui.data_mut(|d| d.insert_temp(id, index));
    let clip = ui.clip_rect();
    if tab.rect.left() < clip.left() || tab.rect.right() > clip.right() {
        tab.scroll_to_me(None);
    }
}

const fn auth_tone(state: &AuthStateView) -> Tone {
    match state {
        AuthStateView::SignedIn => Tone::Success,
        AuthStateView::NeedsSignIn { .. } => Tone::Warning,
        AuthStateView::Blocked { .. } => Tone::Danger,
        AuthStateView::Unknown
        | AuthStateView::Checking
        | AuthStateView::SigningIn { .. }
        | AuthStateView::SignedOut
        | AuthStateView::Disabled => Tone::Neutral,
    }
}

fn org_tab(ui: &mut egui::Ui, theme: &UiTheme, tab: &Tab<'_>, selected: bool) -> egui::Response {
    let m = &theme.tokens.metrics;
    let c = &theme.colors;
    let width = tab_chrome(theme) + tab.galley.size().x;
    let size = vec2(width, theme.control_height());
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if response.hovered() && !selected {
        let top = egui::CornerRadius {
            nw: m.radius.md,
            ne: m.radius.md,
            sw: 0,
            se: 0,
        };
        ui.painter().rect_filled(rect, top, c.control_hover);
    }
    if selected {
        let bar = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 2.0), rect.max);
        ui.painter().rect_filled(bar, 1.0, c.accent_base);
    }
    let tone = tab.state.map_or(Tone::Neutral, auth_tone);
    let dot = pos2(rect.left() + m.space.lg + DOT / 2.0, rect.center().y);
    ui.painter()
        .circle_filled(dot, DOT / 2.0, c.tone(tone).solid);
    let at = pos2(
        dot.x + DOT / 2.0 + m.space.sm,
        rect.center().y - tab.galley.size().y / 2.0,
    );
    let (text, _) = tab_style(theme, selected);
    ui.painter().galley(at, tab.galley.clone(), text);
    let label = tab.label.as_str();
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, selected, label));
    match tab.state.map(copy::auth_state_short) {
        Some(tip) => response.on_hover_text(tip),
        None => response,
    }
}

/// "+ Add organization" drawn like a tab, so the line reads as one control; only the "+" (named
/// for screen readers and on hover) when `compact`.
fn add_tab(ui: &mut egui::Ui, theme: &UiTheme, compact: bool) -> egui::Response {
    let m = &theme.tokens.metrics;
    let size = vec2(add_width(ui, theme, compact), theme.control_height());
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    controls::ghost_fill(ui, theme, rect, &response);
    let color = if response.hovered() {
        theme.colors.text_primary
    } else {
        theme.colors.text_secondary
    };
    let glyph = Rect::from_center_size(
        pos2(rect.left() + m.space.lg + PLUS / 2.0, rect.center().y),
        vec2(PLUS, PLUS),
    );
    icons::paint(ui.painter(), glyph, Icon::Plus, color);
    if !compact {
        let galley = add_label(ui, theme);
        let at = pos2(
            glyph.right() + m.space.sm,
            rect.center().y - galley.size().y / 2.0,
        );
        ui.painter().galley(at, galley, color);
    }
    let name = copy::ADD_ORG;
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, name));
    if compact {
        response.on_hover_text(name)
    } else {
        response
    }
}
