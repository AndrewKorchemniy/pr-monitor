//! Banners under the toolbar: the most severe first, the rest behind "+N more".

use eframe::egui::{self, RichText};
use prm_config::Severity;
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

/// The icon, the title and detail, then the buttons on the right.
fn row(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    (item, more): (&Banner, Option<usize>),
    expanded: &mut bool,
) {
    let theme = cx.theme.clone();
    let fit = Fit::of(ui, &theme, item, more);
    ui.vertical(|ui| {
        ui.add_space(fit.text_top + (fit.title - ICON) / 2.0);
        let color = theme.colors.tone(severity_tone(item.severity)).solid;
        icons::show(ui, severity_icon(item.severity), ICON, color);
    });
    ui.vertical(|ui| {
        ui.add_space(fit.text_top);
        text(ui, &theme, item);
    });
    if fit.buttons {
        let size = egui::vec2(ui.available_width(), fit.row);
        let right = egui::Layout::right_to_left(egui::Align::Center);
        ui.allocate_ui_with_layout(size, right, |ui| buttons(ui, cx, item, more, expanded));
    }
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
        let row = if actions {
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
    ui.label(RichText::new(item.title.as_ref()).font(theme.fonts.body_strong.clone()));
    if let Some(detail) = &item.detail {
        ui.label(RichText::new(detail.as_ref()).color(theme.colors.text_secondary));
    }
}

/// Right to left: close, "+N more", then the suggested actions.
fn buttons(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    item: &Banner,
    more: Option<usize>,
    expanded: &mut bool,
) {
    if item.dismissible && ui.small_button(copy::CLOSE).clicked() {
        cx.push(UiAction::DismissBanner(item.id.clone()));
    }
    if more.is_some_and(|n| ui.link(copy::more_banners(n)).clicked()) {
        *expanded = true;
    }
    for action in [&item.secondary, &item.action].into_iter().flatten() {
        if ui.button(copy::user_action_label(action)).clicked() {
            cx.push(UiAction::User {
                org: item.org.clone(),
                action: action.clone(),
            });
        }
    }
}
