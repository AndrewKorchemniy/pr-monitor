//! The toolbar: the filter field, grouping, refresh and settings, with the refresh progress
//! line along its bottom edge.

use eframe::egui::{self, RichText};
use prm_domain::view::GroupBy;

use crate::{
    actions::UiAction,
    commands::CommandId,
    copy,
    frame::FrameCx,
    theme::icons::Icon,
    widgets::{self, controls},
};

pub(crate) const FILTER_ID: &str = "filter";

pub(crate) const GROUPS: [(GroupBy, &str, CommandId); 6] = [
    (GroupBy::Status, "Status", CommandId::GroupStatus),
    (GroupBy::None, "None", CommandId::GroupNone),
    (GroupBy::Author, "Author", CommandId::GroupAuthor),
    (GroupBy::Repo, "Repository", CommandId::GroupRepo),
    (GroupBy::Project, "Project", CommandId::GroupProject),
    (GroupBy::Org, "Organization", CommandId::GroupOrg),
];

pub(crate) fn group_label(group: GroupBy) -> &'static str {
    GROUPS
        .iter()
        .find(|(g, _, _)| *g == group)
        .map_or("None", |(_, label, _)| label)
}

pub(crate) fn filter_id() -> egui::Id {
    egui::Id::unique(FILTER_ID)
}

/// Returns whether the filter field has focus.
pub(crate) fn toolbar(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    filter: &mut String,
    group: GroupBy,
) -> bool {
    let theme = cx.theme.clone();
    let rect = ui.max_rect();
    let mut focused = false;
    ui.horizontal_centered(|ui| {
        let width = (ui.available_width() - 280.0).clamp(160.0, 520.0);
        let response =
            controls::search_field(ui, &theme, filter, (filter_id(), copy::FILTER_HINT), width);
        if response.changed() {
            cx.push(UiAction::SetFilterText(filter.clone()));
        }
        focused = response.has_focus();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = theme.tokens.metrics.space.sm;
            icon_button(ui, cx, Icon::Gear, CommandId::SettingsOpen);
            icon_button(ui, cx, Icon::Refresh, CommandId::RefreshNow);
            group_menu(ui, cx, group);
        });
    });
    if let Some(fraction) = cx.state.refresh_fraction() {
        widgets::progress_line(ui, rect, fraction, theme.colors.accent_base);
    }
    focused
}

fn icon_button(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, icon: Icon, command: CommandId) {
    let theme = cx.theme.clone();
    let title = cx.titled(command);
    let response = controls::icon_button(ui, &theme, icon, command.title()).on_hover_text(&title);
    if response.clicked() {
        cx.run(command);
    }
}

fn group_menu(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, group: GroupBy) {
    let theme = cx.theme.clone();
    let response = controls::menu_button(ui, &theme, copy::GROUP, group_label(group));
    egui::Popup::menu(&response).show(|ui| {
        ui.set_min_width(response.rect.width().max(160.0));
        for (value, label, command) in GROUPS {
            let text = RichText::new(label).font(theme.fonts.body.clone());
            if ui
                .add(egui::Button::selectable(value == group, text))
                .clicked()
            {
                cx.run(command);
            }
        }
    });
}
