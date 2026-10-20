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

fn banner(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    item: &Banner,
    more: Option<usize>,
    expanded: &mut bool,
) {
    let theme = cx.theme.clone();
    let tone = severity_tone(item.severity);
    adapter::frame_banner(&theme, tone).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            icons::show(
                ui,
                severity_icon(item.severity),
                12.0,
                theme.colors.tone(tone).solid,
            );
            ui.vertical(|ui| text(ui, &theme, item));
            let right = egui::Layout::right_to_left(egui::Align::Center);
            ui.with_layout(right, |ui| buttons(ui, cx, item, more, expanded));
        });
    });
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
