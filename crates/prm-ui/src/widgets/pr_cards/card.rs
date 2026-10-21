//! One pull request as a card: the status glyph, the title on up to two lines and the flags;
//! under the title a muted line with the number, where it lives, who opened it and its age,
//! then the review marks and the comment count. Painted with the table's cells.

use std::sync::Arc;

use eframe::egui::{
    self, Align, Galley, Layout, Rect, UiBuilder, pos2,
    text::{LayoutJob, TextWrapping},
    vec2,
};
use prm_domain::view::PrRow;

use crate::{
    copy,
    theme::UiTheme,
    view::columns::FLAG_SLOT,
    widgets::{
        pr_table::{cells, rows::RowCx},
        vote_tally::vote_tally,
    },
};

/// A title wraps to at most this many lines, then ends in an ellipsis.
const TITLE_LINES: usize = 2;
/// The status glyph's lane before the title (points).
const STATUS_LANE: f32 = 9.0;
/// A part of the detail line that has to be cut narrower than this is left out (points).
const MIN_PART: f32 = 24.0;

/// A card's height: two title lines, the detail line, and padding above and below (points).
pub(super) fn height(ui: &egui::Ui, theme: &UiTheme) -> f32 {
    let m = &theme.tokens.metrics;
    let (title, detail) = line_heights(ui, theme);
    TITLE_LINES as f32 * title + m.space.xxs + detail + 2.0 * m.space.md
}

/// The height of a title line and of the detail line (points).
fn line_heights(ui: &egui::Ui, theme: &UiTheme) -> (f32, f32) {
    let row = |font: &egui::FontId| ui.ctx().fonts_mut(|f| f.row_height(font));
    let detail = row(&theme.fonts.small).max(row(&theme.fonts.mono));
    (row(&theme.fonts.body), detail)
}

/// Lays the card out in the cell: the title block is centred, so a one-line title does not
/// leave a gap under it.
pub(super) fn card(ui: &mut egui::Ui, rcx: &RowCx<'_, '_>, pr: &PrRow, index: usize) {
    let m = &rcx.theme.tokens.metrics;
    let cell = ui.max_rect();
    let inner = Rect::from_min_max(cell.min, pos2(cell.right() - m.space.md, cell.bottom()));
    let (title_line, detail_line) = line_heights(ui, rcx.theme);
    let flags = cells::flags(rcx.theme, pr);
    let marks = flags.iter().filter(|(on, ..)| *on).count() as f32;
    let flags_w = if marks > 0.0 {
        marks * FLAG_SLOT + m.space.sm
    } else {
        0.0
    };
    let text_left = inner.left() + STATUS_LANE + m.space.sm;
    let galley = title_galley(ui, rcx.theme, pr, inner.right() - flags_w - text_left);
    let block = galley.size().y + m.space.xxs + detail_line;
    let top = (inner.center().y - block / 2.0).max(inner.top());
    let first = Rect::from_x_y_ranges(inner.x_range(), top..=top + title_line);
    let mut lane = ui.new_child(
        UiBuilder::new()
            .max_rect(first.with_max_x(text_left))
            .layout(Layout::left_to_right(Align::Center)),
    );
    cells::status_cell(&mut lane, rcx.theme, pr, true);
    let title = Rect::from_min_size(pos2(text_left, top), galley.size());
    let response = ui.put(title, egui::Label::new(galley));
    cells::title_hover(response, rcx, pr);
    for (slot, (_, icon, color, tip)) in flags.into_iter().filter(|(on, ..)| *on).enumerate() {
        let x = first.right() - (marks - slot as f32 - 0.5) * FLAG_SLOT;
        cells::flag(ui, pos2(x, first.center().y), (icon, color), tip);
    }
    let below = title.bottom() + m.space.xxs;
    let line = Rect::from_x_y_ranges(text_left..=inner.right(), below..=below + detail_line);
    detail(ui, rcx, (pr, index), line);
}

/// The title in the table's face, wrapped to at most `TITLE_LINES` lines of `width` points.
fn title_galley(ui: &egui::Ui, theme: &UiTheme, pr: &PrRow, width: f32) -> Arc<Galley> {
    let (font, color) = cells::title_style(theme, pr);
    let mut job = LayoutJob::simple(pr.title.to_string(), font, color, width.max(0.0));
    job.wrap = TextWrapping {
        max_width: width.max(0.0),
        max_rows: TITLE_LINES,
        ..TextWrapping::default()
    };
    ui.painter().layout_job(job)
}

/// The review marks and the comment count at the right of `line`, and the facts left of them.
fn detail(ui: &mut egui::Ui, rcx: &RowCx<'_, '_>, (pr, index): (&PrRow, usize), line: Rect) {
    let theme = rcx.theme;
    let mut right = ui.new_child(
        UiBuilder::new()
            .max_rect(line)
            .layout(Layout::right_to_left(Align::Center)),
    );
    cells::comments_cell(&mut right, theme, pr);
    vote_tally(&mut right, theme, &pr.reviewers);
    let end = right.min_rect().left() - theme.tokens.metrics.space.md;
    let facts = Rect::from_min_max(line.min, pos2(end.max(line.left()), line.bottom()));
    let mut chain = ui.new_child(
        UiBuilder::new()
            .max_rect(facts)
            .layout(Layout::left_to_right(Align::Center)),
    );
    chain.spacing_mut().item_spacing.x = 0.0;
    facts_line(&mut chain, rcx, (pr, index));
}

/// "!4711 · contoso · pr-monitor · Maya Patel · 3h": the number and the age whole, and the
/// organization (when the table would show it), repository and author sharing what is left.
fn facts_line(ui: &mut egui::Ui, rcx: &RowCx<'_, '_>, (pr, index): (&PrRow, usize)) {
    let theme = rcx.theme;
    let author = pr.author.display_name();
    let org = if rcx.input.show_org {
        pr.org_label.as_ref()
    } else {
        ""
    };
    let parts = [org, pr.repo_name.as_ref(), author.as_ref()];
    let small = |text: &str| natural(ui, &theme.fonts.small, text);
    let fixed = natural(ui, &theme.fonts.mono, &pr.number_text)
        + natural(ui, &theme.fonts.mono, cells::age_text(rcx, pr, index));
    let present = parts.iter().filter(|p| !p.is_empty()).count();
    let separator = small(copy::DETAIL_SEPARATOR);
    let room = ui.available_width() - fixed - separator * (present + 1) as f32;
    let whole = parts.map(|p| if p.is_empty() { 0.0 } else { small(p) });
    let widths = share(whole, room);
    ui.label(cells::mono(theme, &pr.number_text));
    for ((text, width), whole) in parts.into_iter().zip(widths).zip(whole) {
        // why: a short name ("api") is narrower than `MIN_PART` even whole.
        if !text.is_empty() && (width >= whole - 0.5 || width >= MIN_PART) {
            dot(ui, theme, separator);
            ui.add(egui::Label::new(cut(ui, theme, text, width)));
        }
    }
    dot(ui, theme, separator);
    cells::age_label(ui, rcx, pr, index);
}

/// How wide `text` is on one line in `font` (points).
fn natural(ui: &egui::Ui, font: &egui::FontId, text: &str) -> f32 {
    let galley =
        ui.painter()
            .layout_no_wrap(text.to_owned(), font.clone(), egui::Color32::PLACEHOLDER);
    galley.size().x
}

/// A separator painted in its own space, so screen readers hear only the facts.
fn dot(ui: &mut egui::Ui, theme: &UiTheme, width: f32) {
    let (_, rect) = ui.allocate_space(vec2(width, ui.available_height()));
    let color = theme.colors.text_muted;
    let galley = ui.painter().layout_no_wrap(
        copy::DETAIL_SEPARATOR.to_owned(),
        theme.fonts.small.clone(),
        color,
    );
    let at = pos2(rect.left(), rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(at, galley, color);
}

/// `text` in the detail face, cut with an ellipsis to `width` points.
fn cut(ui: &egui::Ui, theme: &UiTheme, text: &str, width: f32) -> Arc<Galley> {
    let mut job = LayoutJob::simple_singleline(
        text.to_owned(),
        theme.fonts.small.clone(),
        theme.colors.text_secondary,
    );
    job.wrap = TextWrapping::truncate_at_width(width);
    ui.painter().layout_job(job)
}

/// Widths for parts whose `natural` widths must share `room` (points): all of them while they
/// fit; else the narrow ones keep theirs and the rest split what is left evenly.
pub(super) fn share<const N: usize>(natural: [f32; N], room: f32) -> [f32; N] {
    let mut order: [usize; N] = std::array::from_fn(|i| i);
    order.sort_by(|a, b| natural[*a].total_cmp(&natural[*b]));
    let mut left = room.max(0.0);
    let mut cap = f32::INFINITY;
    // why: narrowest first, each takes its width while it is no more than an even share; the
    // first that is more caps itself and every wider one at that share.
    for (done, index) in order.into_iter().enumerate() {
        let even = left / (N - done) as f32;
        if natural[index] > even {
            cap = even;
            break;
        }
        left -= natural[index];
    }
    natural.map(|width| width.min(cap))
}
