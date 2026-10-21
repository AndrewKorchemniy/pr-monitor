//! What each column draws for one pull request row. The card list paints a card from the same
//! pieces, so a row and a card read alike.

use eframe::egui::{self, Align, Color32, Layout, Rect, RichText, Sense, pos2, vec2};
use prm_domain::view::PrRow;

use super::rows::RowCx;
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
    rcx: &RowCx<'_, '_>,
    spec: &ColumnSpec,
    pr: &PrRow,
    index: usize,
) {
    let theme = rcx.theme;
    let text = |s: &str| {
        RichText::new(s)
            .font(theme.fonts.body.clone())
            .color(theme.colors.text_primary)
    };
    match spec.id {
        ColumnId::Margin => change_bar(ui, rcx, pr),
        ColumnId::Status => {
            let compact = ui.available_width() < COMPACT_STATUS;
            status_cell(ui, theme, pr, compact);
        }
        ColumnId::Number => number_cell(ui, theme, spec, &pr.number_text),
        ColumnId::Flags => flags_cell(ui, theme, pr),
        ColumnId::Title => title_cell(ui, rcx, pr),
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
        ColumnId::Age => {
            right(ui, |ui| age_label(ui, rcx, pr, index));
        }
        ColumnId::Org => {
            ui.add(egui::Label::new(text(&pr.org_label)).truncate());
        }
    }
}

/// The status glyph (and label unless `compact`), explained on hover.
pub(crate) fn status_cell(ui: &mut egui::Ui, theme: &UiTheme, pr: &PrRow, compact: bool) {
    status_pill(ui, theme, pr.status, compact).on_hover_text(copy::status_tooltip(
        pr.status,
        pr.tally,
        pr.active_threads,
    ));
}

/// Numbers and ages: the mono face in the secondary text color.
pub(crate) fn mono(theme: &UiTheme, text: &str) -> RichText {
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

/// "2/5" active of total threads, right-aligned, in the active-comments tone while any is open.
pub(crate) fn comments_cell(ui: &mut egui::Ui, theme: &UiTheme, pr: &PrRow) {
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

/// "3h" for the row at display `index`, kept fresh by the age cache.
pub(crate) fn age_text<'a>(rcx: &'a RowCx<'_, '_>, pr: &'a PrRow, index: usize) -> &'a str {
    let age = rcx
        .input
        .ages
        .get(rcx.view.display.pr_at(index).unwrap_or(usize::MAX))
        .unwrap_or(&pr.age);
    &age.text
}

/// The age (`age_text`), with the creation time on hover.
pub(crate) fn age_label(ui: &mut egui::Ui, rcx: &RowCx<'_, '_>, pr: &PrRow, index: usize) {
    let when = crate::format::date_time(pr.created_at, rcx.frame.offset);
    ui.label(mono(rcx.theme, age_text(rcx, pr, index)))
        .on_hover_text(when);
}

fn right(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.with_layout(Layout::right_to_left(Align::Center), add);
}

/// Draft, merge conflicts and auto-complete: whether each is set, its glyph, color and tooltip.
pub(crate) fn flags(theme: &UiTheme, pr: &PrRow) -> [(bool, Icon, Color32, &'static str); 3] {
    let c = &theme.colors;
    [
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
    ]
}

/// Each flag in its own slot so the marks line up down the table and never move the title.
fn flags_cell(ui: &mut egui::Ui, theme: &UiTheme, pr: &PrRow) {
    let cell = ui.max_rect();
    let mut x = cell.left();
    for (on, icon, color, tip) in flags(theme, pr) {
        if on {
            let center = pos2(x + FLAG_SLOT / 2.0, cell.center().y);
            flag(ui, center, (icon, color), tip);
        }
        x += FLAG_SLOT;
    }
}

/// One flag glyph centred on `center`; it explains itself on hover.
pub(crate) fn flag(
    ui: &mut egui::Ui,
    center: egui::Pos2,
    (icon, color): (Icon, Color32),
    tip: &str,
) {
    let rect = Rect::from_center_size(center, vec2(FLAG_GLYPH, FLAG_GLYPH));
    icons::paint(ui.painter(), rect, icon, color);
    let response = ui.allocate_rect(rect, Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Image, true, tip));
    let _ = response.on_hover_text(tip);
}

/// The title's face and color: muted while the row could not be refreshed.
pub(crate) fn title_style(theme: &UiTheme, pr: &PrRow) -> (egui::FontId, Color32) {
    let color = if pr.stale {
        theme.colors.text_muted
    } else {
        theme.colors.text_primary
    };
    (theme.fonts.body.clone(), color)
}

fn title_cell(ui: &mut egui::Ui, rcx: &RowCx<'_, '_>, pr: &PrRow) {
    let (font, color) = title_style(rcx.theme, pr);
    let text = RichText::new(pr.title.as_ref()).font(font).color(color);
    let response = ui.add(egui::Label::new(text).truncate());
    title_hover(response, rcx, pr);
}

/// What changed (behind the change bar), else why the row is stale, on hover over the title.
pub(crate) fn title_hover(response: egui::Response, rcx: &RowCx<'_, '_>, pr: &PrRow) {
    if let Some(note) = rcx.frame.changes.note(&pr.key) {
        response.on_hover_text(note.text.as_ref());
    } else if pr.stale {
        let when = crate::format::clock(pr.last_activity, rcx.frame.offset);
        response.on_hover_text(copy::stale_row(&when));
    }
}

/// The 3 px blue-pencil bar for rows changed since last looked; draws downward when it appears.
pub(crate) fn change_bar(ui: &mut egui::Ui, rcx: &RowCx<'_, '_>, pr: &PrRow) {
    let theme = rcx.theme;
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
