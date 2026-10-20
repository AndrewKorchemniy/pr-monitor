//! The toolbar: the filter field, grouping, refresh and settings, with the refresh progress
//! line along its bottom edge.

use eframe::egui::{self, RichText};
use prm_domain::view::GroupBy;

use crate::{
    actions::UiAction,
    commands::CommandId,
    copy,
    frame::FrameCx,
    theme::icons::{self, Icon},
    widgets,
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
        icons::show(ui, Icon::Search, 14.0, theme.colors.text_muted);
        let edit = egui::TextEdit::singleline(filter)
            .id(filter_id())
            .hint_text(copy::FILTER_HINT)
            .desired_width((ui.available_width() - 260.0).max(160.0))
            .frame(egui::Frame::NONE);
        let response = ui.add(edit);
        if response.changed() {
            cx.push(UiAction::SetFilterText(filter.clone()));
        }
        focused = response.has_focus();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
    let color = cx.theme.colors.text_secondary;
    let title = cx.titled(command);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), egui::Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 4.0, cx.theme.colors.control_hover);
    }
    icons::paint(ui.painter(), rect.shrink(6.0), icon, color);
    let response = response.on_hover_text(&title);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, command.title()));
    if response.clicked() {
        cx.run(command);
    }
}

fn group_menu(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, group: GroupBy) {
    let text = RichText::new(group_label(group)).font(cx.theme.fonts.label.clone());
    egui::ComboBox::new("group", copy::GROUP)
        .selected_text(text)
        .show_ui(ui, |ui| {
            for (value, label, command) in GROUPS {
                if ui.selectable_label(value == group, label).clicked() {
                    cx.run(command);
                }
            }
        });
}
