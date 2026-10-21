//! What each column draws for one pull request row.

use eframe::egui::{self, Align, Color32, Layout, Rect, RichText, Sense, pos2, vec2};
use prm_domain::view::PrRow;

use super::BodyCx;
use crate::{
    copy,
    theme::{
        Tone, UiTheme,
        icons::{self, Icon},
    },
    view::columns::{ColumnId, ColumnSpec, FLAG_SLOT},
    widgets::{status_pill::status_pill, vote_tally::vote_tally},
};

/// Below this Status column width only the glyph shows.
const COMPACT_STATUS: f32 = 100.0;
/// A flag glyph in the Flags column.
const FLAG_GLYPH: f32 = 11.0;

pub(super) fn cell(
    ui: &mut egui::Ui,
    bcx: &BodyCx<'_, '_>,
    spec: &ColumnSpec,
    pr: &PrRow,
    index: usize,
) {
    let theme = bcx.theme;
    let text = |s: &str| {
        RichText::new(s)
            .font(theme.fonts.body.clone())
            .color(theme.colors.text_primary)
    };
    match spec.id {
        ColumnId::Margin => change_bar(ui, bcx, pr),
        ColumnId::Status => {
            let compact = ui.available_width() < COMPACT_STATUS;
            status_pill(ui, theme, pr.status, compact).on_hover_text(copy::status_tooltip(
                pr.status,
                pr.tally,
                pr.active_threads,
            ));
        }
        ColumnId::Number => number_cell(ui, theme, spec, &pr.number_text),
        ColumnId::Flags => flags_cell(ui, theme, pr),
        ColumnId::Title => title_cell(ui, bcx, pr),
        ColumnId::Repo => {
            ui.add(egui::Label::new(text(&pr.repo_name)).truncate());
        }
        ColumnId::Author => {
            ui.add(egui::Label::new(text(&pr.author.display_name())).truncate());
        }
        ColumnId::Reviews => {
            vote_tally(ui, theme, &pr.reviewers);
        }
        ColumnId::Comments => comments_cell(ui, theme, pr),
        ColumnId::Age => age_cell(ui, bcx, pr, index),
        ColumnId::Org => {
            ui.add(egui::Label::new(text(&pr.org_label)).truncate());
        }
    }
}

fn mono(theme: &UiTheme, text: &str) -> RichText {
    RichText::new(text)
        .font(theme.fonts.mono.clone())
        .color(theme.colors.text_secondary)
}

/// "!4711", right-aligned unless the column says otherwise (glyph-only status mode).
fn number_cell(ui: &mut egui::Ui, theme: &UiTheme, spec: &ColumnSpec, number: &str) {
    if spec.align_right {
        right(ui, |ui| {
            ui.label(mono(theme, number));
        });
    } else {
        ui.label(mono(theme, number));
    }
}

/// "2/5" active of total threads, in the active-comments tone while any is open.
fn comments_cell(ui: &mut egui::Ui, theme: &UiTheme, pr: &PrRow) {
    if pr.total_threads == 0 {
        return;
    }
    let color = if pr.active_threads > 0 {
        theme.colors.tone(Tone::ActiveComments).fg
    } else {
        theme.colors.text_muted
    };
    right(ui, |ui| {
        ui.label(
            RichText::new(pr.comments_text.as_ref())
                .font(theme.fonts.mono.clone())
                .color(color),
        );
    });
}

/// "3h", kept fresh by the age cache, with the creation time on hover.
fn age_cell(ui: &mut egui::Ui, bcx: &BodyCx<'_, '_>, pr: &PrRow, index: usize) {
    let age = bcx
        .input
        .ages
        .get(bcx.view.display.pr_at(index).unwrap_or(usize::MAX))
        .unwrap_or(&pr.age);
    let when = crate::format::date_time(pr.created_at, bcx.cx_now.offset);
    right(ui, |ui| {
        ui.label(mono(bcx.theme, &age.text)).on_hover_text(when);
    });
}

fn right(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.with_layout(Layout::right_to_left(Align::Center), add);
}

/// Draft, merge conflicts and auto-complete, each in its own slot so the marks line up down the
/// table and never move the title; each explains itself on hover.
fn flags_cell(ui: &mut egui::Ui, theme: &UiTheme, pr: &PrRow) {
    let c = &theme.colors;
    let flags = [
        (pr.draft, Icon::Pencil, c.text_muted, copy::DRAFT),
        (
            pr.conflicts,
            Icon::Warning,
            c.tone(Tone::Warning).solid,
            copy::CONFLICTS,
        ),
        (
            pr.auto_complete,
            Icon::Bolt,
            c.tone(Tone::Success).solid,
            copy::AUTO_COMPLETE,
        ),
    ];
    let cell = ui.max_rect();
    let mut x = cell.left();
    for (on, icon, color, tip) in flags {
        if on {
            let center = pos2(x + FLAG_SLOT / 2.0, cell.center().y);
            let rect = Rect::from_center_size(center, vec2(FLAG_GLYPH, FLAG_GLYPH));
            flag(ui, rect, (icon, color), tip);
        }
        x += FLAG_SLOT;
    }
}

fn flag(ui: &mut egui::Ui, rect: Rect, (icon, color): (Icon, Color32), tip: &str) {
    icons::paint(ui.painter(), rect, icon, color);
    let response = ui.allocate_rect(rect, Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, tip));
    let _ = response.on_hover_text(tip);
}

fn title_cell(ui: &mut egui::Ui, bcx: &BodyCx<'_, '_>, pr: &PrRow) {
    let theme = bcx.theme;
    let color = if pr.stale {
        theme.colors.text_muted
    } else {
        theme.colors.text_primary
    };
    let label = egui::Label::new(
        RichText::new(pr.title.as_ref())
            .font(theme.fonts.body.clone())
            .color(color),
    )
    .truncate();
    let response = ui.add(label);
    if let Some(note) = bcx.cx_now.changes.note(&pr.key) {
        response.on_hover_text(note.text.as_ref());
    } else if pr.stale {
        let when = pr.last_activity;
        response.on_hover_text(copy::stale_row(&crate::format::clock(
            when,
            bcx.cx_now.offset,
        )));
    }
}

/// The 3 px blue-pencil bar for rows changed since last looked; draws downward when it appears.
fn change_bar(ui: &mut egui::Ui, bcx: &BodyCx<'_, '_>, pr: &PrRow) {
    let theme = bcx.theme;
    let id = ui.scope_id().with(("bar", &pr.key));
    let motion = &theme.tokens.metrics.motion;
    let duration = if theme.policy.animations {
        motion.reveal
    } else {
        0.0
    };
    let t = ui.ctx().animate_bool_with_time(id, pr.unread, duration);
    if t <= 0.0 {
        return;
    }
    let cell = ui.max_rect();
    let width = theme.tokens.metrics.row.revision_bar;
    let top = cell.top() + 4.0;
    let bottom = top + (cell.height() - 8.0) * t;
    let bar = Rect::from_min_max(
        pos2(cell.left() + 1.0, top),
        pos2(cell.left() + 1.0 + width, bottom),
    );
    ui.painter()
        .rect_filled(bar, width / 2.0, theme.colors.signature_revision);
    let _ = ui
        .allocate_rect(bar, Sense::hover())
        .on_hover_text(copy::CHANGED_TOOLTIP);
}
