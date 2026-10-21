//! Dashboard view state: query, sort, grouping, collapsed groups, the selection (a key, never
//! an index), the cached view model and display list, and pending scroll requests.

pub(crate) mod anchor;
pub(crate) mod changes;
pub(crate) mod columns;
pub(crate) mod display;
pub(crate) mod empty;

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use eframe::egui::Align;
use prm_config::UiState;
use prm_domain::{
    PrKey,
    view::{
        GroupBy, PrRow, Sort, SortKey, StatusFilter, StatusSummary, ViewModel, ViewQuery,
        build_view, filtered_summary,
    },
};

use crate::view::{
    anchor::{AnchorResult, capture, restore},
    columns::{Choices, ColumnId, Visibility},
    display::{DisplayList, DisplayRow, Pitch, build_display, group_id},
};

/// Rows a page-up/down moves before the list has been drawn.
const PAGE: usize = 15;

/// The sort one click on `key`'s header gives: the sorted column reverses, any other column
/// sorts ascending. One click is always enough to see a change.
pub(crate) fn next_sort(current: Sort, key: SortKey) -> Sort {
    Sort {
        key,
        descending: current.key == key && !current.descending,
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub(crate) enum ScrollIntent {
    #[default]
    None,
    ToRow(usize, Option<Align>),
    Offset(f32),
}

pub(crate) struct DashboardView {
    rows: Arc<[PrRow]>,
    rows_generation: u64,
    pub query: ViewQuery,
    pub sort: Sort,
    pub group: GroupBy,
    pub collapsed: BTreeSet<String>,
    pub selected: Option<PrKey>,
    pub model: ViewModel,
    /// What the status bar's filter toggles count: the rows the text filter keeps, whatever
    /// the status and changed-only filters.
    pub bar_counts: StatusSummary,
    pub display: DisplayList,
    pub scroll: ScrollIntent,
    /// The last selection change came from the keyboard (shows the focus ring).
    pub keyboard_nav: bool,
    pub widths: BTreeMap<String, f32>,
    pub hidden_columns: Vec<String>,
    pub offset_y: f32,
    pub top_visible: Option<usize>,
    /// How far apart the list drew its rows last frame (table rows, or cards and headers).
    pub pitch: Pitch,
    /// Bumped by "reset columns" so the table forgets its stored widths.
    pub table_epoch: u32,
    /// The table's id salt last frame (column set, epoch); when it changes the table keeps its
    /// scroll offset.
    pub table_salt: Option<(u64, u32)>,
    /// Chosen columns the table had no room for last frame (not persisted).
    pub squeezed: Vec<ColumnId>,
    /// Pull request rows a page-up/down moves: one fewer than the list showed whole last frame,
    /// so a page keeps one row in view.
    pub page_rows: usize,
}

impl DashboardView {
    pub(crate) fn new(
        ui: &UiState,
        rows: Arc<[PrRow]>,
        rows_generation: u64,
        row_pitch: f32,
    ) -> Self {
        let mut view = Self {
            rows,
            rows_generation,
            query: ui.filter.clone(),
            sort: ui.sort,
            group: ui.group,
            collapsed: ui.collapsed_groups.iter().cloned().collect(),
            selected: ui.selected.clone(),
            model: ViewModel::default(),
            bar_counts: StatusSummary::default(),
            display: DisplayList::default(),
            scroll: ScrollIntent::None,
            keyboard_nav: false,
            widths: ui.columns.clone(),
            hidden_columns: ui.hidden_columns.clone(),
            offset_y: 0.0,
            top_visible: None,
            pitch: Pitch::uniform(row_pitch),
            table_epoch: 0,
            table_salt: None,
            squeezed: Vec::new(),
            page_rows: PAGE,
        };
        view.rebuild();
        if let Some(index) = view.selected_display() {
            view.scroll = ScrollIntent::ToRow(index, Some(Align::Center));
        }
        view
    }

    pub(crate) fn rows(&self) -> &Arc<[PrRow]> {
        &self.rows
    }

    pub(crate) const fn rows_generation(&self) -> u64 {
        self.rows_generation
    }

    /// New data: keep the anchor (selected or top row) at the same screen position.
    pub(crate) fn on_rows(&mut self, rows: Arc<[PrRow]>, generation: u64) {
        let snapshot = capture(
            &self.display,
            self.selected.as_ref(),
            (self.top_visible, self.offset_y),
            self.pitch,
        );
        self.rows = rows;
        self.rows_generation = generation;
        self.rebuild();
        let Some(snapshot) = snapshot else {
            return;
        };
        match restore(&snapshot, &self.display, self.pitch) {
            AnchorResult::Offset(offset) => self.scroll = ScrollIntent::Offset(offset),
            AnchorResult::SelectNeighbor { index } => {
                if self.selected.is_some() {
                    self.selected = self.key_at(index);
                }
            }
            AnchorResult::None => {}
        }
    }

    pub(crate) fn rebuild(&mut self) {
        self.model = build_view(&self.rows, &self.query, self.sort, self.group);
        self.display = build_display(&self.rows, &self.model, self.group, &self.collapsed);
        self.bar_counts = filtered_summary(&self.rows, &self.query);
    }

    /// Filter typing: keep the selection if it still matches, else select the first row.
    pub(crate) fn set_text(&mut self, text: &str) {
        if self.query.text == text {
            return;
        }
        text.clone_into(&mut self.query.text);
        self.after_filter_change();
    }

    pub(crate) fn set_status(&mut self, status: StatusFilter) {
        self.query.status = if self.query.status == status {
            StatusFilter::All
        } else {
            status
        };
        self.after_filter_change();
    }

    pub(crate) fn toggle_unread_only(&mut self) {
        self.query.unread_only = !self.query.unread_only;
        self.after_filter_change();
    }

    pub(crate) fn toggle_hide_drafts(&mut self) {
        self.query.hide_drafts = !self.query.hide_drafts;
        self.after_filter_change();
    }

    /// Clears text, status, changed-only and hide-drafts.
    pub(crate) fn clear_filters(&mut self) {
        self.query = ViewQuery {
            org: self.query.org.clone(),
            ..ViewQuery::default()
        };
        self.after_filter_change();
    }

    pub(crate) fn filters_active(&self) -> bool {
        !self.query.text.is_empty()
            || self.query.status != StatusFilter::All
            || self.query.unread_only
            || self.query.hide_drafts
    }

    fn after_filter_change(&mut self) {
        self.rebuild();
        if let Some(index) = self.selected_display() {
            self.scroll = ScrollIntent::ToRow(index, None);
        } else {
            self.selected = self.first_pr();
            self.scroll = ScrollIntent::Offset(0.0);
        }
    }

    pub(crate) fn set_group(&mut self, group: GroupBy) {
        self.group = group;
        self.rebuild();
        self.scroll_to_selection();
    }

    /// One click on a header: see `next_sort`.
    pub(crate) fn sort_by(&mut self, key: SortKey) {
        self.sort = next_sort(self.sort, key);
        self.rebuild();
        self.scroll_to_selection();
    }

    /// Shows or hides a column from the column menu or settings. Only optional columns change,
    /// so Status and Title always show.
    pub(crate) fn toggle_column(&mut self, id: ColumnId) {
        let Some(column) = columns::column(id) else {
            return;
        };
        if column.visibility() != Visibility::Optional {
            return;
        }
        if let Some(at) = self.hidden_columns.iter().position(|k| k == column.key) {
            self.hidden_columns.remove(at);
        } else {
            self.hidden_columns.push(column.key.to_owned());
        }
    }

    /// What the column menu and the settings checklist show.
    pub(crate) fn column_choices(&self) -> Choices<'_> {
        Choices {
            hidden: &self.hidden_columns,
            squeezed: &self.squeezed,
        }
    }

    /// A width dragged in the header (points), or `None` to go back to the default.
    pub(crate) fn set_column_width(&mut self, key: &str, width: Option<f32>) {
        match width {
            Some(width) if self.widths.get(key) != Some(&width) => {
                self.widths.insert(key.to_owned(), width);
            }
            Some(_) => {}
            None => {
                self.widths.remove(key);
            }
        }
    }

    fn scroll_to_selection(&mut self) {
        if let Some(index) = self.selected_display() {
            self.scroll = ScrollIntent::ToRow(index, None);
        }
    }

    pub(crate) fn selected_display(&self) -> Option<usize> {
        self.display.index_of.get(self.selected.as_ref()?).copied()
    }

    pub(crate) fn selected_row(&self) -> Option<&PrRow> {
        let index = self.selected_display()?;
        self.display.pr_at(index).and_then(|row| self.rows.get(row))
    }

    pub(crate) fn row_at(&self, display: usize) -> Option<&PrRow> {
        self.display
            .pr_at(display)
            .and_then(|row| self.rows.get(row))
    }

    fn key_at(&self, display: usize) -> Option<PrKey> {
        self.row_at(display).map(|r| r.key.clone())
    }

    fn first_pr(&self) -> Option<PrKey> {
        (0..self.display.rows.len()).find_map(|i| self.key_at(i))
    }

    pub(crate) fn select_key(&mut self, key: &PrKey) {
        self.selected = Some(key.clone());
        if let Some(index) = self.selected_display() {
            self.scroll = ScrollIntent::ToRow(index, Some(Align::Center));
        }
    }

    /// Display positions of PR rows (not group headers), in order.
    fn pr_positions(&self) -> Vec<usize> {
        (0..self.display.rows.len())
            .filter(|i| self.display.pr_at(*i).is_some())
            .collect()
    }

    /// Moves the selection by `delta` PR rows, clamped. A selection hidden in a collapsed group
    /// moves from that group's header; with no selection, starts at an end.
    pub(crate) fn nav(&mut self, delta: isize) {
        let prs = self.pr_positions();
        let Some(last) = prs.len().checked_sub(1) else {
            return;
        };
        let current = self
            .selected_display()
            .and_then(|d| prs.iter().position(|p| *p == d));
        let target = match (current, self.selected_header()) {
            (Some(at), _) => at.saturating_add_signed(delta).min(last),
            (None, Some(header)) => {
                // why: the header sits between PR `below - 1` and PR `below`.
                let below = prs.partition_point(|p| *p < header);
                let step = if delta > 0 { delta - 1 } else { delta };
                below.saturating_add_signed(step).min(last)
            }
            (None, None) if delta > 0 => 0,
            (None, None) => last,
        };
        self.select_keyboard(prs.get(target).copied());
    }

    pub(crate) fn nav_page(&mut self, down: bool) {
        let rows = isize::try_from(self.page_rows.max(1)).unwrap_or(1);
        self.nav(if down { rows } else { -rows });
    }

    pub(crate) fn nav_edge(&mut self, last: bool) {
        let len = self.display.rows.len();
        let found = if last {
            (0..len).rev().find(|i| self.display.pr_at(*i).is_some())
        } else {
            (0..len).find(|i| self.display.pr_at(*i).is_some())
        };
        self.select_keyboard(found);
    }

    /// The first PR of the next (or previous) group; collapsed groups have none and are
    /// skipped. Backward from inside a group goes to that group's first PR first.
    pub(crate) fn nav_group(&mut self, forward: bool) {
        let current = self
            .selected_display()
            .or_else(|| self.selected_header())
            .unwrap_or(0);
        let headers: Vec<usize> = (0..self.display.rows.len())
            .filter(|i| matches!(self.display.rows[*i], DisplayRow::Group(_)))
            .collect();
        let target = if forward {
            headers
                .iter()
                .filter(|h| **h > current)
                .find_map(|h| self.first_pr_under(*h))
        } else {
            headers
                .iter()
                .rev()
                .filter(|h| **h < current)
                .filter_map(|h| self.first_pr_under(*h))
                .find(|pr| *pr != current)
        };
        self.select_keyboard(target);
    }

    /// The first PR row right under a group header, before the next header.
    fn first_pr_under(&self, header: usize) -> Option<usize> {
        let next = header + 1;
        self.display.pr_at(next).map(|_| next)
    }

    fn select_keyboard(&mut self, display: Option<usize>) {
        if let Some(index) = display {
            self.selected = self.key_at(index);
            self.keyboard_nav = true;
            self.scroll = ScrollIntent::ToRow(index, None);
        }
    }

    /// The group containing `key` (its span index), from the model, so it is found even when
    /// the group is collapsed. `None` when not grouping or the key is filtered out.
    fn group_of(&self, key: &PrKey) -> Option<u32> {
        if self.group == GroupBy::None {
            return None;
        }
        let row = self.rows.iter().position(|r| r.key == *key)?;
        let at = self
            .model
            .indices
            .iter()
            .position(|r| usize::try_from(*r).ok() == Some(row))?;
        let span = self.model.groups.iter().position(|g| {
            let start = usize::try_from(g.start).unwrap_or(usize::MAX);
            let len = usize::try_from(g.len).unwrap_or(0);
            (start..start.saturating_add(len)).contains(&at)
        })?;
        u32::try_from(span).ok()
    }

    /// The group containing the selection (its span index).
    fn selected_group(&self) -> Option<u32> {
        self.group_of(self.selected.as_ref()?)
    }

    /// The display position of the selected PR's group header.
    fn selected_header(&self) -> Option<usize> {
        let span = self.selected_group()?;
        self.display
            .rows
            .iter()
            .position(|r| *r == DisplayRow::Group(span))
    }

    /// Loaded, but the query (text, status, changed-only, drafts) hides it.
    pub(crate) fn is_filtered_out(&self, key: &PrKey) -> bool {
        let Some(row) = self.rows.iter().position(|r| r.key == *key) else {
            return false;
        };
        !self
            .model
            .indices
            .iter()
            .any(|r| usize::try_from(*r).ok() == Some(row))
    }

    /// Expands the group holding `key`, if it is collapsed.
    pub(crate) fn expand_group_of(&mut self, key: &PrKey) {
        if let Some(span) = self.group_of(key) {
            self.set_group_collapsed(span, false);
        }
    }

    pub(crate) fn toggle_group(&mut self, span: u32) {
        let Some(group) = self
            .model
            .groups
            .get(usize::try_from(span).unwrap_or(usize::MAX))
        else {
            return;
        };
        let id = group_id(&group.key);
        if !self.collapsed.remove(&id) {
            self.collapsed.insert(id);
        }
        self.rebuild();
    }

    /// Collapses or expands the selection's group. The selection is kept while hidden, so
    /// expanding shows it again.
    pub(crate) fn set_selected_group_collapsed(&mut self, collapsed: bool) {
        if let Some(span) = self.selected_group() {
            self.set_group_collapsed(span, collapsed);
        }
    }

    fn set_group_collapsed(&mut self, span: u32, collapsed: bool) {
        let Some(group) = self
            .model
            .groups
            .get(usize::try_from(span).unwrap_or(usize::MAX))
        else {
            return;
        };
        let id = group_id(&group.key);
        let changed = if collapsed {
            self.collapsed.insert(id)
        } else {
            self.collapsed.remove(&id)
        };
        if changed {
            self.rebuild();
            self.scroll_to_selection();
        }
    }

    /// Restores default column widths and visibility.
    pub(crate) fn reset_columns(&mut self) {
        self.widths.clear();
        self.hidden_columns.clear();
        self.table_epoch = self.table_epoch.wrapping_add(1);
    }

    /// Writes the view's persisted parts.
    pub(crate) fn write_ui_state(&self, ui: &mut UiState) {
        ui.filter = self.query.clone();
        ui.sort = self.sort;
        ui.group = self.group;
        ui.collapsed_groups = self.collapsed.iter().cloned().collect();
        ui.selected.clone_from(&self.selected);
        ui.columns.clone_from(&self.widths);
        ui.hidden_columns.clone_from(&self.hidden_columns);
    }

    /// Whether `write_ui_state` would change `ui`, compared in place (runs every frame).
    pub(crate) fn differs_from(&self, ui: &UiState) -> bool {
        ui.filter != self.query
            || ui.sort != self.sort
            || ui.group != self.group
            || !ui.collapsed_groups.iter().eq(self.collapsed.iter())
            || ui.selected != self.selected
            || ui.columns != self.widths
            || ui.hidden_columns != self.hidden_columns
    }
}

#[cfg(test)]
mod tests;
