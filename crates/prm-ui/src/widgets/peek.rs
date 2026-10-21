//! The peek drawer (Space): the selected PR's branches, reviewers, open threads and actions,
//! without leaving the list. Docked beside the list, or over it in a narrow window; its close
//! button and Space put the list back.

use eframe::egui::{self, RichText};
use prm_domain::view::PrRow;

use crate::{
    actions::UiAction,
    commands::CommandId,
    copy,
    frame::FrameCx,
    theme::UiTheme,
    widgets::{
        self, controls,
        status_pill::status_pill,
        vote_tally::{vote_color, vote_icon},
    },
};

pub(crate) fn peek(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, pr: Option<&PrRow>) {
    let theme = cx.theme.clone();
    let Some(pr) = pr else {
        ui.horizontal(|ui| {
            widgets::muted(ui, &theme, copy::NO_OPEN_THREADS);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                close(ui, cx, &theme);
            });
        });
        return;
    };
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            header(ui, cx, &theme, pr);
            ui.add_space(theme.tokens.metrics.space.xl);
            reviewers(ui, &theme, pr);
            ui.add_space(theme.tokens.metrics.space.xl);
            threads(ui, cx, &theme, pr);
        });
}

fn header(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, theme: &UiTheme, pr: &PrRow) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(pr.number_text.as_ref())
                .font(theme.fonts.mono_strong.clone())
                .color(theme.colors.text_secondary),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            close(ui, cx, theme);
        });
    });
    ui.add(
        egui::Label::new(RichText::new(pr.title.as_ref()).font(theme.fonts.title.clone())).wrap(),
    );
    ui.horizontal(|ui| {
        status_pill(ui, theme, pr.status, false);
    });
    let branches = copy::branches(&pr.source_branch, &pr.target_branch);
    ui.label(
        RichText::new(branches)
            .font(theme.fonts.mono.clone())
            .color(theme.colors.text_secondary),
    );
    let meta = format!(
        "{} · {} · {}",
        pr.author.display_name(),
        pr.repo_name,
        pr.project_name
    );
    widgets::muted(ui, theme, &meta);
    let opened = prm_domain::view::age::age_label(cx.now, pr.created_at).text;
    let activity = prm_domain::view::age::age_label(cx.now, pr.last_activity).text;
    widgets::muted(ui, theme, &copy::opened_activity(&opened, &activity));
    if let Some(note) = cx.changes.note(&pr.key) {
        ui.label(RichText::new(note.text.as_ref()).color(theme.colors.signature_revision));
    }
    ui.add_space(theme.tokens.metrics.space.md);
    ui.horizontal_wrapped(|ui| {
        if widgets::primary_button(ui, theme, copy::OPEN_IN_BROWSER).clicked() {
            cx.push(UiAction::Open(pr.key.clone()));
        }
        if ui.button(copy::COPY_LINK).clicked() {
            cx.run(CommandId::PrCopyLink);
        }
        if pr.unread && ui.button(copy::MARK_SEEN).clicked() {
            cx.run(CommandId::PrMarkSeen);
        }
    });
}

fn close(ui: &mut egui::Ui, cx: &mut FrameCx<'_>, theme: &UiTheme) {
    if controls::close_button(ui, theme, copy::CLOSE).clicked() {
        cx.run(CommandId::PrPeek);
    }
}

fn reviewers(ui: &mut egui::Ui, theme: &UiTheme, pr: &PrRow) {
    widgets::field_label(ui, theme, copy::REVIEWS);
    if pr.reviewers.is_empty() {
        widgets::muted(ui, theme, copy::NO_REVIEWS);
    }
    for reviewer in pr.reviewers.iter() {
        ui.horizontal(|ui| {
            let color = vote_color(theme, reviewer.vote);
            match vote_icon(reviewer.vote) {
                Some(icon) => {
                    crate::theme::icons::show(ui, icon, 10.0, color);
                }
                None => {
                    ui.add_space(14.0);
                }
            }
            let name_color = if reviewer.listed {
                theme.colors.text_primary
            } else {
                theme.colors.text_muted
            };
            ui.label(RichText::new(reviewer.name.as_ref()).color(name_color));
            let mut tags = Vec::new();
            if reviewer.group {
                tags.push(copy::GROUP_TAG);
            }
            if reviewer.required {
                tags.push(copy::REQUIRED_TAG);
            }
            if !tags.is_empty() {
                widgets::muted(ui, theme, &tags.join(" · "));
            }
        });
    }
}

fn threads(ui: &mut egui::Ui, cx: &FrameCx<'_>, theme: &UiTheme, pr: &PrRow) {
    if pr.total_threads == 0 {
        widgets::field_label(ui, theme, copy::OPEN_THREADS);
    } else {
        let title = copy::open_threads(pr.active_threads, pr.total_threads);
        widgets::field_label(ui, theme, &title);
    }
    if pr.threads_preview.is_empty() {
        widgets::muted(ui, theme, copy::NO_OPEN_THREADS);
        return;
    }
    for thread in pr.threads_preview.iter() {
        crate::theme::adapter::frame_sunken(theme).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let mut line = thread.author.to_string();
            if let Some(file) = &thread.file {
                line.push_str(" · ");
                line.push_str(file);
            }
            if let Some(at) = thread.at {
                line.push_str(" · ");
                line.push_str(&prm_domain::view::age::age_label(cx.now, at).text);
            }
            widgets::muted(ui, theme, &line);
            ui.add(egui::Label::new(thread.snippet.as_ref()).wrap());
        });
        ui.add_space(theme.tokens.metrics.space.xs);
    }
}
