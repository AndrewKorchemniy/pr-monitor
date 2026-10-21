//! Banners under the toolbar: the most severe first, the rest behind "+N more". The text wraps
//! to the room the buttons leave; in a narrow window the buttons move under the text.

use eframe::egui::{self, RichText};
use prm_config::Severity;
use prm_domain::auth::UserAction;
use prm_service::state::Banner;

use crate::{
    actions::UiAction,
    copy,
    frame::FrameCx,
    theme::{Tone, UiTheme, adapter, icons},
};

pub(crate) const fn severity_tone(severity: Severity) -> Tone {
    match severity {
        Severity::Info => Tone::Info,
        Severity::Warning => Tone::Warning,
        Severity::Error => Tone::Danger,
    }
}

const fn severity_icon(severity: Severity) -> icons::Icon {
    match severity {
        Severity::Info => icons::Icon::Info,
        Severity::Warning | Severity::Error => icons::Icon::Warning,
    }
}

/// Shows `banners` (already filtered of dismissed ones); `expanded` shows all of them.
pub(crate) fn banners(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    banners: &[&Banner],
    expanded: &mut bool,
) {
    let Some((first, rest)) = banners.split_first() else {
        return;
    };
    banner(
        ui,
        cx,
        first,
        (!*expanded && !rest.is_empty()).then_some(rest.len()),
        expanded,
    );
    if *expanded {
        for item in rest {
            banner(ui, cx, item, None, expanded);
        }
    }
}

/// The severity icon, in points.
const ICON: f32 = 12.0;
/// Narrower than this beside the buttons, the text gets the whole width and the buttons go
/// under it (points).
const TEXT_MIN: f32 = 240.0;

fn banner(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    item: &Banner,
    more: Option<usize>,
    expanded: &mut bool,
) {
    let theme = cx.theme.clone();
    adapter::frame_banner(&theme, severity_tone(item.severity)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        // why: `horizontal` makes the row one control tall and starts the text at its top,
        // which left a blank band under a title-only banner; this row is as tall as its
        // content, with the text centred on the buttons and the icon on the title.
        ui.horizontal_top(|ui| row(ui, cx, (item, more), expanded));
    });
}

/// The icon, the title and detail, then the buttons on the right, or under the text when the
/// banner is too narrow for both.
fn row(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    (item, more): (&Banner, Option<usize>),
    expanded: &mut bool,
) {
    let theme = cx.theme.clone();
    let mut fit = Fit::of(ui, &theme, item, more);
    // why: the buttons' width is known once they are laid out, so it is kept from the last
    // pass; a new or changed width discards the pass, so a wrong layout is never shown.
    let id = egui::Id::unique(("banner_buttons", item.id.as_ref()));
    let known = ui.data(|d| d.get_temp::<f32>(id));
    let gap = ui.spacing().item_spacing.x;
    let beside = ui.available_width() - ICON - gap - known.unwrap_or(0.0) - gap;
    let stacked = fit.buttons && beside < TEXT_MIN;
    if stacked {
        fit.text_top = 0.0;
    }
    ui.vertical(|ui| {
        ui.add_space(fit.text_top + (fit.title - ICON) / 2.0);
        let color = theme.colors.tone(severity_tone(item.severity)).solid;
        icons::show(ui, severity_icon(item.severity), ICON, color);
    });
    if !fit.buttons {
        ui.vertical(|ui| text(ui, &theme, item));
        return;
    }
    let measured = if stacked {
        ui.vertical(|ui| {
            text(ui, &theme, item);
            ui.horizontal_wrapped(|ui| buttons(ui, cx, (item, more), (expanded, Edge::Left)))
                .inner
        })
        .inner
    } else {
        side_by_side(ui, cx, (item, more), (expanded, &fit), beside)
    };
    if known.is_none_or(|width| (width - measured).abs() > 0.5) {
        ui.data_mut(|d| d.insert_temp(id, measured));
        ui.ctx().request_discard("banner buttons measured");
    }
}

/// The text wrapped to `width` points, then the buttons on the right, centred on the text.
/// Returns the buttons' width.
fn side_by_side(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    (item, more): (&Banner, Option<usize>),
    (expanded, fit): (&mut bool, &Fit),
    width: f32,
) -> f32 {
    let theme = cx.theme.clone();
    let column = egui::Layout::top_down(egui::Align::Min);
    let text_h = ui
        .allocate_ui_with_layout(egui::vec2(width, fit.row), column, |ui| {
            ui.set_width(width);
            ui.add_space(fit.text_top);
            text(ui, &theme, item);
        })
        .response
        .rect
        .height();
    let size = egui::vec2(ui.available_width(), fit.row.max(text_h));
    let right = egui::Layout::right_to_left(egui::Align::Center);
    ui.allocate_ui_with_layout(size, right, |ui| {
        buttons(ui, cx, (item, more), (expanded, Edge::Right))
    })
    .inner
}

/// How a banner's row is sized: the title's line, the row (the text, or a button if taller)
/// and where the text starts so that it is centred in the row.
struct Fit {
    title: f32,
    row: f32,
    text_top: f32,
    buttons: bool,
}

impl Fit {
    fn of(ui: &egui::Ui, theme: &UiTheme, item: &Banner, more: Option<usize>) -> Self {
        let row_height = |font: &egui::FontId| ui.ctx().fonts_mut(|f| f.row_height(font));
        let title = row_height(&theme.fonts.body_strong);
        let detail = item.detail.as_ref().map_or(0.0, |_| {
            ui.spacing().item_spacing.y + row_height(&theme.fonts.body)
        });
        let text = title + detail;
        let actions = item.action.is_some() || item.secondary.is_some();
        let row = if actions || item.dismissible {
            text.max(ui.spacing().interact_size.y)
        } else {
            text
        };
        Self {
            title,
            row,
            text_top: ((row - text) / 2.0).max(0.0),
            buttons: actions || item.dismissible || more.is_some(),
        }
    }
}

fn text(ui: &mut egui::Ui, theme: &UiTheme, item: &Banner) {
    let title = RichText::new(item.title.as_ref()).font(theme.fonts.body_strong.clone());
    ui.add(egui::Label::new(title).wrap());
    if let Some(detail) = &item.detail {
        let detail = RichText::new(detail.as_ref()).color(theme.colors.text_secondary);
        ui.add(egui::Label::new(detail).wrap());
    }
}

/// The edge of the banner the buttons line up from.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Edge {
    /// Beside the text, close at the far right.
    Right,
    /// Under the text, the suggested action first.
    Left,
}

/// Reading left to right: the suggested actions, "+N more", then close. Returns how wide they
/// are side by side, however the layout wraps them (points).
fn buttons(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    (item, more): (&Banner, Option<usize>),
    (expanded, edge): (&mut bool, Edge),
) -> f32 {
    let widths = match edge {
        Edge::Right => [
            close_button(ui, cx, item),
            more_link(ui, more, expanded),
            action_button(ui, cx, (item, item.secondary.as_ref())),
            action_button(ui, cx, (item, item.action.as_ref())),
        ],
        Edge::Left => [
            action_button(ui, cx, (item, item.action.as_ref())),
            action_button(ui, cx, (item, item.secondary.as_ref())),
            more_link(ui, more, expanded),
            close_button(ui, cx, item),
        ],
    };
    let shown = widths.iter().flatten().count();
    let gaps = ui.spacing().item_spacing.x * shown.saturating_sub(1) as f32;
    widths.iter().flatten().sum::<f32>() + gaps
}

/// Dismisses a banner that may be dismissed; its width when shown.
fn close_button(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, item: &Banner) -> Option<f32> {
    if !item.dismissible {
        return None;
    }
    let response = ui.button(copy::CLOSE);
    if response.clicked() {
        cx.push(UiAction::DismissBanner(item.id.clone()));
    }
    Some(response.rect.width())
}

/// "+N more", which shows every banner; its width when shown.
fn more_link(ui: &mut egui::Ui, more: Option<usize>, expanded: &mut bool) -> Option<f32> {
    let response = ui.link(copy::more_banners(more?));
    if response.clicked() {
        *expanded = true;
    }
    Some(response.rect.width())
}

/// A suggested fix, when the banner has one; its width when shown.
fn action_button(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    (item, action): (&Banner, Option<&UserAction>),
) -> Option<f32> {
    let action = action?;
    let response = ui.button(copy::user_action_label(action));
    if response.clicked() {
        cx.push(UiAction::User {
            org: item.org.clone(),
            action: action.clone(),
        });
    }
    Some(response.rect.width())
}
