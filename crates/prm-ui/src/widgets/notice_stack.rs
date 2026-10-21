//! Transient notices: compact pills stacked at the top right, under the toolbar. Each fades in,
//! fades out before it expires, and the newest sits lowest.

use std::time::Duration;

use eframe::egui::{self, Align2, RichText, vec2};

use crate::{
    actions::UiAction,
    copy,
    frame::FrameCx,
    notices::{NoticeStack, ShownNotice},
    widgets::{banner::severity_tone, controls},
};

const MAX_WIDTH: f32 = 380.0;
const FADE_IN: Duration = Duration::from_millis(180);
const FADE_OUT: Duration = Duration::from_millis(260);
/// The pill's height.
const HEIGHT: f32 = 26.0;

/// `top` is the toolbar's height; the stack starts just below it.
pub(crate) fn notice_stack(
    ctx: &egui::Context,
    cx: &mut FrameCx<'_>,
    notices: &NoticeStack,
    top: f32,
) {
    if notices.items().is_empty() {
        return;
    }
    let animate = cx.theme.policy.animations;
    egui::Area::new(egui::Id::unique("notices"))
        .anchor(Align2::RIGHT_TOP, vec2(-8.0, top + 8.0))
        // why: above dialogs and the settings window, which share the foreground layer.
        .order(egui::Order::Tooltip)
        .movable(false)
        .show(ctx, |ui| {
            ui.set_max_width(MAX_WIDTH);
            let layout = egui::Layout::top_down(egui::Align::Max);
            ui.with_layout(layout, |ui| stack(ui, cx, notices, animate));
        });
}

/// The notices, oldest on top, each at its fade opacity.
fn stack(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, notices: &NoticeStack, animate: bool) {
    ui.spacing_mut().item_spacing.y = 6.0;
    for (index, notice) in notices.items().iter().enumerate() {
        let alpha = if animate { opacity(cx, notice) } else { 1.0 };
        ui.scope(|ui| {
            ui.multiply_opacity(alpha);
            pill(ui, cx, notice, index);
        });
        if alpha < 1.0 {
            ui.ctx().request_repaint();
        }
        // why: wake in time to draw the fade-out, not only at expiry.
        let until_fade = notice.expires.since(cx.now).saturating_sub(FADE_OUT);
        ui.ctx().request_repaint_after(until_fade);
    }
}

/// Fades in over `FADE_IN` after it appears and out over `FADE_OUT` before it expires.
fn opacity(cx: &FrameCx<'_>, notice: &ShownNotice) -> f32 {
    let shown = cx.now.since(notice.shown).as_secs_f32() / FADE_IN.as_secs_f32();
    let left = notice.expires.since(cx.now).as_secs_f32() / FADE_OUT.as_secs_f32();
    shown.min(left).clamp(0.0, 1.0)
}

/// One notice: its text, an optional action and a small close button, on a slightly see-through
/// fill with a tone-coloured edge.
fn pill(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, notice: &ShownNotice, index: usize) {
    let theme = cx.theme.clone();
    let tone = theme.colors.tone(severity_tone(notice.severity));
    let m = &theme.tokens.metrics;
    egui::Frame::NONE
        .fill(theme.colors.surface_overlay.gamma_multiply(0.94))
        .stroke(egui::Stroke::new(m.stroke.hairline, tone.solid))
        .corner_radius(egui::CornerRadius::same(m.radius.md))
        .inner_margin(egui::Margin::symmetric(10, 0))
        .show(ui, |ui| {
            ui.set_height(HEIGHT);
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = m.space.sm;
                let text = RichText::new(notice.text.as_ref())
                    .font(theme.fonts.body.clone())
                    .color(theme.colors.text_primary);
                ui.add(egui::Label::new(text).truncate());
                buttons(ui, cx, notice, index);
            });
        });
}

fn buttons(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, notice: &ShownNotice, index: usize) {
    let theme = cx.theme.clone();
    if let Some(action) = &notice.action
        && ui.small_button(copy::user_action_label(action)).clicked()
    {
        cx.push(UiAction::User {
            org: None,
            action: action.clone(),
        });
    }
    if controls::small_close_button(ui, &theme, copy::CLOSE).clicked() {
        cx.push(UiAction::DismissNotice(index));
    }
}
