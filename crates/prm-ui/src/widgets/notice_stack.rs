//! Transient notices, stacked in the top-right corner under the toolbar. Each fades and slides
//! in, fades out before it expires, and the stack glides when one leaves, so nothing jumps.

use std::time::Duration;

use eframe::egui::{self, Align2, Rect, RichText, UiBuilder, pos2, vec2};

use crate::{
    actions::UiAction,
    copy,
    frame::FrameCx,
    notices::{NoticeStack, ShownNotice},
    theme::adapter,
    widgets::{banner::severity_tone, controls},
};

const WIDTH: f32 = 340.0;
const GAP: f32 = 8.0;
const FADE_IN: Duration = Duration::from_millis(180);
const FADE_OUT: Duration = Duration::from_millis(260);
/// How far a notice slides while it fades in, in points.
const SLIDE: f32 = 8.0;

pub(crate) fn notice_stack(
    ctx: &egui::Context,
    cx: &mut FrameCx<'_>,
    notices: &NoticeStack,
    top_inset: f32,
) {
    if notices.items().is_empty() {
        return;
    }
    let theme = cx.theme.clone();
    let animate = theme.policy.animations;
    egui::Area::new(egui::Id::unique("notices"))
        .anchor(Align2::RIGHT_TOP, vec2(-12.0, top_inset + 8.0))
        .order(egui::Order::Foreground)
        .movable(false)
        .show(ctx, |ui| {
            ui.set_width(WIDTH);
            let origin = ui.cursor().min;
            let mut y = 0.0;
            for (index, notice) in notices.items().iter().enumerate() {
                let key = ui.scope_id().with(("notice", notice.text.as_ref()));
                let target = if animate {
                    ctx.animate_value_with_time(
                        key.with("y"),
                        y,
                        theme.tokens.metrics.motion.standard,
                    )
                } else {
                    y
                };
                let alpha = if animate { opacity(cx, notice) } else { 1.0 };
                let top = origin.y + target - (1.0 - alpha) * SLIDE;
                let rect = Rect::from_min_size(pos2(origin.x, top), vec2(WIDTH, f32::INFINITY));
                let height = ui
                    .scope_builder(UiBuilder::new().id_salt(key).max_rect(rect), |ui| {
                        ui.multiply_opacity(alpha);
                        card(ui, cx, notice, index)
                    })
                    .inner;
                y += height + GAP;
                if alpha < 1.0 {
                    ctx.request_repaint();
                }
                // why: wake in time to draw the fade-out, not only at expiry.
                let until_fade = notice.expires.since(cx.now).saturating_sub(FADE_OUT);
                ctx.request_repaint_after(until_fade);
            }
            ui.allocate_space(vec2(WIDTH, y));
        });
}

/// Fades in over `FADE_IN` after it appears and out over `FADE_OUT` before it expires.
fn opacity(cx: &FrameCx<'_>, notice: &ShownNotice) -> f32 {
    let shown = cx.now.since(notice.shown).as_secs_f32() / FADE_IN.as_secs_f32();
    let left = notice.expires.since(cx.now).as_secs_f32() / FADE_OUT.as_secs_f32();
    shown.min(left).clamp(0.0, 1.0)
}

/// One notice in a popup card with a tone-colored edge; returns its height.
fn card(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, notice: &ShownNotice, index: usize) -> f32 {
    let theme = cx.theme.clone();
    let tone = theme.colors.tone(severity_tone(notice.severity));
    adapter::frame_popup(&theme, theme.policy)
        .stroke(egui::Stroke::new(1.0, tone.solid))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(RichText::new(notice.text.as_ref())).wrap());
                let right = egui::Layout::right_to_left(egui::Align::Center);
                ui.with_layout(right, |ui| buttons(ui, cx, notice, index));
            });
        })
        .response
        .rect
        .height()
}

fn buttons(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, notice: &ShownNotice, index: usize) {
    let theme = cx.theme.clone();
    if controls::close_button(ui, &theme, copy::CLOSE).clicked() {
        cx.push(UiAction::DismissNotice(index));
    }
    if let Some(action) = &notice.action
        && ui.button(copy::user_action_label(action)).clicked()
    {
        cx.push(UiAction::User {
            org: None,
            action: action.clone(),
        });
    }
}
