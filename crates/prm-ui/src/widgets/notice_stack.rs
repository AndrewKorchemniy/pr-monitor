//! Transient notices, stacked in the bottom-right corner above the status bar.

use eframe::egui::{self, Align2, RichText};

use crate::{
    actions::UiAction,
    copy,
    frame::FrameCx,
    notices::{NoticeStack, ShownNotice},
    theme::adapter,
    widgets::banner::severity_tone,
};

const WIDTH: f32 = 320.0;

pub(crate) fn notice_stack(
    ctx: &egui::Context,
    cx: &mut FrameCx<'_>,
    notices: &NoticeStack,
    bottom_inset: f32,
) {
    if notices.items().is_empty() {
        return;
    }
    let theme = cx.theme.clone();
    egui::Area::new(egui::Id::unique("notices"))
        .anchor(
            Align2::RIGHT_BOTTOM,
            egui::vec2(-12.0, -(bottom_inset + 12.0)),
        )
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.set_max_width(WIDTH);
            for (index, notice) in notices.items().iter().enumerate() {
                let tone = theme.colors.tone(severity_tone(notice.severity));
                adapter::frame_popup(&theme, theme.policy)
                    .stroke(egui::Stroke::new(1.0, tone.solid))
                    .show(ui, |ui| notice_row(ui, cx, notice, index));
                ui.add_space(6.0);
            }
        });
}

fn notice_row(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, notice: &ShownNotice, index: usize) {
    ui.set_width(WIDTH);
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(RichText::new(notice.text.as_ref())).wrap());
        let right = egui::Layout::right_to_left(egui::Align::Center);
        ui.with_layout(right, |ui| notice_buttons(ui, cx, notice, index));
    });
}

fn notice_buttons(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, notice: &ShownNotice, index: usize) {
    if ui.small_button(copy::CLOSE).clicked() {
        cx.push(UiAction::DismissNotice(index));
    }
    if let Some(action) = &notice.action
        && ui.small_button(copy::user_action_label(action)).clicked()
    {
        cx.push(UiAction::User {
            org: None,
            action: action.clone(),
        });
    }
}
