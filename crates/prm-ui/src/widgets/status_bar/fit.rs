//! Which segments get a place in the bar. Room goes out rank by rank: the counts always show,
//! and every other segment shows when it fits, sized for its widest text so a ticking number
//! never changes what else shows.

use std::borrow::Cow;

use eframe::egui;

use crate::{
    theme::UiTheme,
    widgets::status_bar::{
        Hook,
        segment::{Kind, Segment},
        separator_width,
    },
};

/// Which segments give way first in a narrow window, most important first.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum Rank {
    /// The counts are the bar's filters; they show however narrow the window is.
    Counts,
    /// What a refresh is doing, or when the next one runs.
    Refresh,
    /// Notifications paused until a time.
    Paused,
    /// Each org's auth dot; the names come later.
    OrgDots,
    /// When data was last updated.
    Updated,
    /// Refreshes slowed by throttling.
    Slowed,
    /// Every org's name beside its dot, all or none.
    OrgNames,
    /// How many repositories are in scope.
    Repos,
}

/// Every rank, in the order room is handed out.
const RANKS: [Rank; 8] = [
    Rank::Counts,
    Rank::Refresh,
    Rank::Paused,
    Rank::OrgDots,
    Rank::Updated,
    Rank::Slowed,
    Rank::OrgNames,
    Rank::Repos,
];

/// A segment in the running for a place in the bar.
pub(super) struct Slot<'a> {
    pub segment: Segment<'a>,
    rank: Rank,
    pub hook: Hook<'a>,
    /// The points that decide whether it fits: its width, or more for text that ticks.
    fit: f32,
    /// The same for its short text, when it has one (`Segment::with_short`).
    compact_fit: f32,
    pub shown: bool,
}

impl<'a> Slot<'a> {
    pub(super) fn new(theme: &UiTheme, segment: Segment<'a>, rank: Rank, hook: Hook<'a>) -> Self {
        Self {
            fit: segment.width(theme),
            compact_fit: 0.0,
            segment,
            rank,
            hook,
            shown: false,
        }
    }

    /// Shows its short text from now on, if it has one.
    fn compact(&mut self, theme: &UiTheme) {
        if self.segment.has_short() {
            self.segment.compact = true;
            self.fit = self.segment.width(theme).max(self.compact_fit);
        }
    }

    /// Its short text fits as the widest of `texts`, with every digit at its widest.
    pub(super) fn with_short_room_for(
        mut self,
        ui: &egui::Ui,
        theme: &UiTheme,
        texts: &[&str],
    ) -> Self {
        self.compact_fit = texts
            .iter()
            .map(|text| sizing_width(ui, theme, text))
            .fold(0.0, f32::max);
        self
    }

    /// Plain information.
    pub(super) fn info(
        ui: &egui::Ui,
        theme: &UiTheme,
        text: Cow<'a, str>,
        rank: Rank,
        hook: Hook<'a>,
    ) -> Self {
        let segment = Segment::new(ui, theme, None, text, Kind::Info);
        Self::new(theme, segment, rank, hook)
    }

    /// The rank whose neighbours it sits with, without a hairline between: collapsed org dots,
    /// and compact counts.
    pub(super) const fn cluster(&self) -> Option<Rank> {
        if self.segment.collapsed || self.segment.compact {
            Some(self.rank)
        } else {
            None
        }
    }

    /// Fits as the widest of its own text and `texts` with every digit at its widest, so text
    /// that ticks never changes what else shows.
    pub(super) fn with_room_for(mut self, ui: &egui::Ui, theme: &UiTheme, texts: &[&str]) -> Self {
        let widest = std::iter::once(self.segment.text())
            .chain(texts.iter().copied())
            .map(|text| sizing_width(ui, theme, text))
            .fold(self.fit, f32::max);
        self.fit = widest;
        self
    }
}

/// The width of `text` as plain information with every digit as `0`, the widest digit in the
/// bar's font.
fn sizing_width(ui: &egui::Ui, theme: &UiTheme, text: &str) -> f32 {
    let zeroed: String = text
        .chars()
        .map(|c| if c.is_ascii_digit() { '0' } else { c })
        .collect();
    Segment::new(ui, theme, None, zeroed.into(), Kind::Info).width(theme)
}

/// What must fit beside the counts' labels before they give way: so widening the window never
/// trades "Updated" for longer labels.
const BESIDE_LABELS: [Rank; 5] = [
    Rank::Counts,
    Rank::Refresh,
    Rank::Paused,
    Rank::OrgDots,
    Rank::Updated,
];

/// Hands out `room` points rank by rank: the counts always show, and every other segment
/// shows when it fits. One that does not fit leaves its room to smaller ones after it. While
/// the counts, the refresh state, the paused notice, the org dots and "Updated" do not fit
/// side by side, the counts drop their labels (glyph and number); while the counts and the
/// refresh state still do not fit, the refresh state shortens ("Reading 12/40").
pub(super) fn fit(mut room: f32, theme: &UiTheme, left: &mut [Slot<'_>], right: &mut [Slot<'_>]) {
    let separator = separator_width(theme);
    let steps: [(Rank, &[Rank]); 2] = [
        (Rank::Counts, &BESIDE_LABELS),
        (Rank::Refresh, &[Rank::Counts, Rank::Refresh]),
    ];
    for (rank, beside) in steps {
        if crowded(room, separator, (left, right), beside) {
            compact(rank, theme, left);
            compact(rank, theme, right);
        }
    }
    for rank in RANKS {
        match rank {
            Rank::OrgNames => expand_orgs(&mut room, theme, left),
            Rank::Counts
            | Rank::Refresh
            | Rank::Paused
            | Rank::OrgDots
            | Rank::Updated
            | Rank::Slowed
            | Rank::Repos => {
                place(&mut room, separator, rank, left);
                place(&mut room, separator, rank, right);
            }
        }
    }
}

fn compact(rank: Rank, theme: &UiTheme, slots: &mut [Slot<'_>]) {
    for slot in slots.iter_mut().filter(|s| s.rank == rank) {
        slot.compact(theme);
    }
}

/// Whether the segments of `ranks`, as they are now, need more than `room`.
fn crowded(
    room: f32,
    separator: f32,
    (left, right): (&[Slot<'_>], &[Slot<'_>]),
    ranks: &[Rank],
) -> bool {
    let need = |rank: Rank| {
        let slots = || left.iter().chain(right).filter(move |s| s.rank == rank);
        let separators = if slots().any(|s| s.cluster().is_some()) {
            1
        } else {
            slots().count()
        };
        slots().map(|s| s.fit).sum::<f32>() + separator * separators as f32
    };
    ranks.iter().map(|rank| need(*rank)).sum::<f32>() > room
}

/// Shows each slot of `rank` that fits in `room`, charging it a separator too unless it joins
/// the cluster before it.
fn place(room: &mut f32, separator: f32, rank: Rank, slots: &mut [Slot<'_>]) {
    let mut joinable = false;
    for slot in slots.iter_mut().filter(|s| s.rank == rank) {
        let clustered = slot.cluster().is_some();
        let joins = joinable && clustered;
        let cost = if joins {
            slot.fit
        } else {
            slot.fit + separator
        };
        slot.shown = rank == Rank::Counts || cost <= *room;
        if slot.shown {
            *room -= cost;
            joinable = clustered;
        }
    }
}

/// Puts every org's name beside its dot, a hairline between each, when all the dots show and
/// all the names fit.
fn expand_orgs(room: &mut f32, theme: &UiTheme, left: &mut [Slot<'_>]) {
    let orgs = || left.iter().filter(|s| s.rank == Rank::OrgDots);
    let separators = separator_width(theme) * orgs().count().saturating_sub(1) as f32;
    let names: f32 = orgs().map(|s| s.segment.text_width(theme)).sum();
    if !orgs().all(|s| s.shown) || names + separators > *room {
        return;
    }
    *room -= names + separators;
    for slot in left.iter_mut().filter(|s| s.rank == Rank::OrgDots) {
        slot.segment.collapsed = false;
    }
}

#[cfg(test)]
mod tests;
