//! Table columns: ids, persisted keys, default widths, sorting, who may hide each one, and which
//! the table has room for.

use std::collections::BTreeMap;

use prm_domain::view::{GroupBy, SortKey};

use crate::copy;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ColumnId {
    /// The change bar.
    Margin,
    Status,
    Number,
    /// Draft, merge conflicts and auto-complete marks, so they never push the title along.
    Flags,
    Title,
    Repo,
    Author,
    Reviews,
    /// The required reviewer groups and whether each approved.
    Required,
    Comments,
    Age,
    Org,
}

/// Who decides whether a column shows.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Visibility {
    /// Always shown and never listed: the change-bar margin.
    Unlisted,
    /// Listed in the column menu but always shown (greyed out): the table needs it.
    Locked,
    /// Listed; the user may hide it.
    Optional,
    /// Not listed: Settings › General › Organization column decides.
    BySetting,
}

impl Visibility {
    /// Whether the column menu and the settings checklist list it.
    pub(crate) const fn listed(self) -> bool {
        match self {
            Self::Locked | Self::Optional => true,
            Self::Unlisted | Self::BySetting => false,
        }
    }
}

impl ColumnId {
    /// Who decides whether this column shows; the one place each column's rule lives.
    pub(crate) const fn visibility(self) -> Visibility {
        match self {
            Self::Margin => Visibility::Unlisted,
            Self::Status | Self::Title => Visibility::Locked,
            Self::Number
            | Self::Flags
            | Self::Repo
            | Self::Author
            | Self::Reviews
            | Self::Required
            | Self::Comments
            | Self::Age => Visibility::Optional,
            Self::Org => Visibility::BySetting,
        }
    }
}

/// What the user moves as one: a header cell. The leading Status, Number and Flags columns share
/// one cell, "PR"; every other column is its own. The change-bar margin belongs to none: it is
/// pinned at the table's left edge.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ColumnUnit {
    Pr,
    Title,
    Repo,
    Author,
    Reviews,
    Required,
    Comments,
    Age,
    Org,
}

/// How many units there are.
pub(crate) const UNITS: usize = 9;

/// The units' order until the user moves one.
pub(crate) const DEFAULT_ORDER: [ColumnUnit; UNITS] = [
    ColumnUnit::Pr,
    ColumnUnit::Title,
    ColumnUnit::Repo,
    ColumnUnit::Author,
    ColumnUnit::Reviews,
    ColumnUnit::Required,
    ColumnUnit::Comments,
    ColumnUnit::Age,
    ColumnUnit::Org,
];

/// Which way a unit moves one place.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Left,
    Right,
}

impl ColumnUnit {
    /// Persisted in `UiState.column_order`.
    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::Pr => "pr",
            Self::Title => "title",
            Self::Repo => "repo",
            Self::Author => "author",
            Self::Reviews => "reviews",
            Self::Required => "required",
            Self::Comments => "comments",
            Self::Age => "age",
            Self::Org => "org",
        }
    }

    pub(crate) fn from_key(key: &str) -> Option<Self> {
        DEFAULT_ORDER.into_iter().find(|u| u.key() == key)
    }

    /// The header's text and the unit's name for screen readers.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Pr => copy::COLUMN_NUMBER,
            Self::Title => copy::COLUMN_TITLE,
            Self::Repo => copy::COLUMN_REPO,
            Self::Author => copy::COLUMN_AUTHOR,
            Self::Reviews => copy::REVIEWS,
            Self::Required => copy::COLUMN_REQUIRED,
            Self::Comments => copy::COLUMN_COMMENTS,
            Self::Age => copy::COLUMN_AGE,
            Self::Org => copy::ORGANIZATION,
        }
    }

    /// What a click on the header sorts by: the PR cell sorts by number.
    pub(crate) const fn sort(self) -> SortKey {
        match self {
            Self::Pr => SortKey::Number,
            Self::Title => SortKey::Title,
            Self::Repo => SortKey::Repo,
            Self::Author => SortKey::Author,
            Self::Reviews => SortKey::Reviews,
            Self::Required => SortKey::Required,
            Self::Comments => SortKey::Comments,
            Self::Age => SortKey::Age,
            Self::Org => SortKey::Org,
        }
    }

    /// Whether the header's text sits at the cell's right edge, over right-aligned cells.
    pub(crate) const fn align_right(self) -> bool {
        match self {
            Self::Comments | Self::Age => true,
            Self::Pr
            | Self::Title
            | Self::Repo
            | Self::Author
            | Self::Reviews
            | Self::Required
            | Self::Org => false,
        }
    }
}

impl ColumnId {
    /// The unit this column moves with; `None` for the pinned margin.
    pub(crate) const fn unit(self) -> Option<ColumnUnit> {
        match self {
            Self::Margin => None,
            Self::Status | Self::Number | Self::Flags => Some(ColumnUnit::Pr),
            Self::Title => Some(ColumnUnit::Title),
            Self::Repo => Some(ColumnUnit::Repo),
            Self::Author => Some(ColumnUnit::Author),
            Self::Reviews => Some(ColumnUnit::Reviews),
            Self::Required => Some(ColumnUnit::Required),
            Self::Comments => Some(ColumnUnit::Comments),
            Self::Age => Some(ColumnUnit::Age),
            Self::Org => Some(ColumnUnit::Org),
        }
    }
}

/// The units in the user's order: the saved keys (unknown and repeated ones ignored), then each
/// unit they lack straight after the unit that precedes it by default, or first when none does.
/// Hidden units are in it too, so they keep their place.
pub(crate) fn ordered(saved: &[String]) -> Vec<ColumnUnit> {
    let mut order: Vec<ColumnUnit> = Vec::with_capacity(UNITS);
    for unit in saved.iter().filter_map(|key| ColumnUnit::from_key(key)) {
        if !order.contains(&unit) {
            order.push(unit);
        }
    }
    let mut before = None;
    for unit in DEFAULT_ORDER {
        if !order.contains(&unit) {
            let at = before
                .and_then(|prev| order.iter().position(|u| *u == prev))
                .map_or(0, |i| i + 1);
            order.insert(at, unit);
        }
        before = Some(unit);
    }
    order
}

/// What to save for `order`: nothing while it is the default, so a unit added later takes its
/// own default place for everyone who never moved one.
pub(crate) fn order_keys(order: &[ColumnUnit]) -> Vec<String> {
    if order == DEFAULT_ORDER {
        return Vec::new();
    }
    order.iter().map(|u| u.key().to_owned()).collect()
}

/// `order` with `unit` moved to follow `among` of the other shown units (so `0` puts it first).
/// Units `shown` says are not in the table keep their place: the moved one goes just before the
/// shown unit it now precedes, or after the last shown one.
pub(crate) fn placed(
    order: &[ColumnUnit],
    unit: ColumnUnit,
    among: usize,
    shown: impl Fn(ColumnUnit) -> bool,
) -> Vec<ColumnUnit> {
    let mut rest: Vec<ColumnUnit> = order.iter().copied().filter(|u| *u != unit).collect();
    let mut seen = 0;
    let mut at = None;
    for (index, other) in rest.iter().enumerate() {
        if !shown(*other) {
            continue;
        }
        if seen == among {
            at = Some(index);
            break;
        }
        seen += 1;
    }
    let at = at.unwrap_or_else(|| {
        rest.iter()
            .rposition(|u| shown(*u))
            .map_or(rest.len(), |last| last + 1)
    });
    rest.insert(at, unit);
    rest
}

/// `order` with `unit` one shown place further `step`, or `None` at the end of the row.
pub(crate) fn stepped(
    order: &[ColumnUnit],
    (unit, step): (ColumnUnit, Step),
    shown: impl Fn(ColumnUnit) -> bool + Copy,
) -> Option<Vec<ColumnUnit>> {
    let row: Vec<ColumnUnit> = order.iter().copied().filter(|u| shown(*u)).collect();
    let now = row.iter().position(|u| *u == unit)?;
    let among = match step {
        Step::Left => now.checked_sub(1)?,
        Step::Right if now + 1 < row.len() => now + 1,
        Step::Right => return None,
    };
    Some(placed(order, unit, among, shown))
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Width {
    /// The margin, at the theme's margin width.
    Exact,
    /// Always this width (points); the user cannot resize it.
    Fixed(f32),
    /// Takes the remaining space, at least `min`; it flexes with the window.
    Remainder,
    /// User-resizable from the header, starting at `initial` (points).
    Initial(f32),
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct ColumnSpec {
    pub id: ColumnId,
    /// Persisted in `UiState.columns` and `hidden_columns` or `shown_columns` (see `Picks`);
    /// empty for the margin.
    pub key: &'static str,
    /// What the header shows; may be empty when the cells explain themselves.
    pub header: &'static str,
    /// The column's name in the column menu and for screen readers.
    pub name: &'static str,
    pub width: Width,
    pub min: f32,
    pub sort: Option<SortKey>,
    pub align_right: bool,
    /// Whether an optional column shows until the user chooses otherwise. Columns added after
    /// release start hidden: the saved lists only record what the user changed (`Picks`).
    pub default_visible: bool,
}

impl ColumnSpec {
    /// Only `Initial` columns get a drag strip, on their divider away from Title.
    pub(crate) const fn is_resizable(&self) -> bool {
        matches!(self.width, Width::Initial(_))
    }

    pub(crate) const fn visibility(&self) -> Visibility {
        self.id.visibility()
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "a const constructor for the column table"
)]
const fn spec(
    id: ColumnId,
    key: &'static str,
    header: &'static str,
    width: Width,
    min: f32,
    sort: Option<SortKey>,
) -> ColumnSpec {
    ColumnSpec {
        id,
        key,
        header,
        name: header,
        width,
        min,
        sort,
        align_right: false,
        default_visible: true,
    }
}

/// An optional column that stays hidden until the user turns it on.
const fn hidden_by_default(mut column: ColumnSpec) -> ColumnSpec {
    column.default_visible = false;
    column
}

const fn right(mut column: ColumnSpec) -> ColumnSpec {
    column.align_right = true;
    column
}

/// A column whose header stays blank; it keeps its name for the menu and screen readers.
const fn blank_header(mut column: ColumnSpec) -> ColumnSpec {
    column.header = "";
    column
}

/// One glyph slot in the Flags column (points).
pub(crate) const FLAG_SLOT: f32 = 14.0;
/// The Flags column: a slot each for draft, merge conflicts and auto-complete.
pub(crate) const FLAGS_WIDTH: f32 = 3.0 * FLAG_SLOT;

pub(crate) const COLUMNS: [ColumnSpec; 12] = [
    spec(ColumnId::Margin, "", "", Width::Exact, 0.0, None),
    spec(
        ColumnId::Status,
        "status",
        copy::COLUMN_STATUS,
        Width::Initial(140.0),
        30.0,
        Some(SortKey::Status),
    ),
    right(spec(
        ColumnId::Number,
        "number",
        copy::COLUMN_NUMBER,
        Width::Initial(64.0),
        52.0,
        Some(SortKey::Number),
    )),
    blank_header(spec(
        ColumnId::Flags,
        "flags",
        copy::COLUMN_FLAGS,
        Width::Fixed(FLAGS_WIDTH),
        FLAGS_WIDTH,
        None,
    )),
    spec(
        ColumnId::Title,
        "title",
        copy::COLUMN_TITLE,
        Width::Remainder,
        200.0,
        Some(SortKey::Title),
    ),
    spec(
        ColumnId::Repo,
        "repo",
        copy::COLUMN_REPO,
        Width::Initial(160.0),
        80.0,
        Some(SortKey::Repo),
    ),
    spec(
        ColumnId::Author,
        "author",
        copy::COLUMN_AUTHOR,
        Width::Initial(150.0),
        80.0,
        Some(SortKey::Author),
    ),
    spec(
        ColumnId::Reviews,
        "reviews",
        copy::REVIEWS,
        Width::Initial(104.0),
        56.0,
        Some(SortKey::Reviews),
    ),
    hidden_by_default(spec(
        ColumnId::Required,
        "required",
        copy::COLUMN_REQUIRED,
        Width::Initial(120.0),
        56.0,
        Some(SortKey::Required),
    )),
    right(spec(
        ColumnId::Comments,
        "comments",
        copy::COLUMN_COMMENTS,
        Width::Initial(76.0),
        56.0,
        Some(SortKey::Comments),
    )),
    right(spec(
        ColumnId::Age,
        "age",
        copy::COLUMN_AGE,
        Width::Initial(56.0),
        44.0,
        Some(SortKey::Age),
    )),
    spec(
        ColumnId::Org,
        "org",
        copy::ORGANIZATION,
        Width::Initial(120.0),
        80.0,
        Some(SortKey::Org),
    ),
];

/// What a table too narrow for its columns drops, first to last, until the rest fit. Status,
/// Number, Flags, Title and Age always stay.
const SQUEEZE_ORDER: [ColumnId; 6] = [
    ColumnId::Org,
    ColumnId::Required,
    ColumnId::Reviews,
    ColumnId::Repo,
    ColumnId::Author,
    ColumnId::Comments,
];

/// The Status column's width when grouped by status, where only the glyph shows (points).
const STATUS_GLYPH: f32 = 24.0;
/// The PR number's width when it leads the compact layout (points).
const NUMBER_COMPACT: f32 = 56.0;

/// The space the table lays its columns out in (points).
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct Room {
    /// What the columns share: the table's width less the scroll bar's lane.
    pub width: f32,
    /// The space between two columns.
    pub gap: f32,
    /// The change-bar margin's width.
    pub margin: f32,
}

/// What the user changed from each optional column's default: the keys they hid from columns
/// that show by default, and the keys they showed from columns that start hidden. Saved lists
/// from before a column existed hold neither, so it keeps its default.
#[derive(Copy, Clone, Debug, Default)]
pub(crate) struct Picks<'a> {
    pub hidden: &'a [String],
    pub shown: &'a [String],
}

impl Picks<'_> {
    /// The list that records the user's choice for `column`.
    pub(crate) fn changed(&self, column: &ColumnSpec) -> &[String] {
        if column.default_visible {
            self.hidden
        } else {
            self.shown
        }
    }
}

/// The user's choices that shape the table's columns.
#[derive(Copy, Clone, Debug)]
pub(crate) struct ColumnLayout<'a> {
    /// Grouped by status, the Status column shows only its glyph (`compact_status`).
    pub group: GroupBy,
    /// Settings › General › Organization column.
    pub show_org: bool,
    pub picks: Picks<'a>,
    /// Widths the user dragged, by key (points).
    pub widths: &'a BTreeMap<String, f32>,
    /// The units' order (`ordered`).
    pub order: &'a [ColumnUnit],
}

/// How far a resizable column may shrink to fit the window.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Give {
    /// To its default width, or its dragged width when that is narrower: a dragged width gives
    /// way before any column drops, and no column is squeezed below its default while one can.
    ToDefault,
    /// To its minimum, once only the columns that never drop are left.
    ToMinimum,
}

/// The room a column, or columns side by side, would like and the least they can do with
/// (points).
#[derive(Copy, Clone, Debug, PartialEq)]
struct Span {
    preferred: f32,
    floor: f32,
}

impl Span {
    const fn fixed(width: f32) -> Self {
        Self {
            preferred: width,
            floor: width,
        }
    }
}

impl ColumnLayout<'_> {
    /// Where a column goes: the margin first, then by its unit's place in the order.
    fn rank(&self, column: &ColumnSpec) -> usize {
        column.id.unit().map_or(0, |unit| {
            1 + self.order.iter().position(|u| *u == unit).unwrap_or(UNITS)
        })
    }

    fn wants(&self, column: &ColumnSpec) -> bool {
        match column.visibility() {
            Visibility::BySetting => self.show_org,
            Visibility::Unlisted | Visibility::Locked | Visibility::Optional => {
                chosen(column, self.picks)
            }
        }
    }

    /// A column's room: its dragged or default width, Title its minimum and the margin
    /// `room.margin`.
    fn span(&self, column: &ColumnSpec, room: Room, give: Give) -> Span {
        match column.width {
            Width::Exact => Span::fixed(room.margin),
            Width::Fixed(width) => Span::fixed(width),
            Width::Remainder => Span::fixed(column.min),
            Width::Initial(default) => {
                let saved = self.widths.get(column.key).copied();
                let preferred = saved.unwrap_or(default).max(column.min);
                let floor = match give {
                    Give::ToDefault => preferred.min(default),
                    Give::ToMinimum => column.min,
                };
                Span { preferred, floor }
            }
        }
    }

    /// The room `columns` need side by side, gaps included.
    fn needed<'c>(
        &self,
        columns: impl IntoIterator<Item = &'c ColumnSpec>,
        room: Room,
        give: Give,
    ) -> Span {
        let mut total = Span::fixed(0.0);
        for (index, column) in columns.into_iter().enumerate() {
            let gap = if index == 0 { 0.0 } else { room.gap };
            let span = self.span(column, room, give);
            total.preferred += span.preferred + gap;
            total.floor += span.floor + gap;
        }
        total
    }

    /// Gives each resizable column this frame's width: the preferred ones while they fit, else
    /// each gives up the same share of its room above its floor until they do. The dragged
    /// widths themselves stay as the user left them.
    fn fit_widths(&self, columns: &mut [ColumnSpec], room: Room, give: Give) {
        let needed = self.needed(columns.iter(), room, give);
        let spare = needed.preferred - needed.floor;
        let share = if needed.preferred <= room.width || spare <= 0.0 {
            1.0
        } else {
            ((room.width - needed.floor) / spare).clamp(0.0, 1.0)
        };
        for column in columns.iter_mut() {
            if column.is_resizable() {
                let span = self.span(column, room, give);
                let width = span.floor + (span.preferred - span.floor) * share;
                column.width = Width::Initial(width.floor().max(column.min));
            }
        }
    }
}

/// The column with this id, from the table above.
pub(crate) fn column(id: ColumnId) -> Option<ColumnSpec> {
    COLUMNS.iter().find(|c| c.id == id).copied()
}

/// The columns the column menu and the settings checklist offer, in table order.
pub(crate) fn listed() -> impl Iterator<Item = ColumnSpec> {
    COLUMNS.into_iter().filter(|c| c.visibility().listed())
}

/// Whether the user's choice keeps `column` shown: only optional columns can be hidden, so a
/// hand-edited list never drops Status or Title.
pub(crate) fn chosen(column: &ColumnSpec, picks: Picks<'_>) -> bool {
    match column.visibility() {
        Visibility::Optional => {
            let changed = picks.changed(column).iter().any(|k| k == column.key);
            changed != column.default_visible
        }
        Visibility::Unlisted | Visibility::Locked | Visibility::BySetting => true,
    }
}

/// Columns in order, each resizable one at this frame's width (`Width::Initial`). First the
/// user's choice; while that needs more room than there is, dragged widths give way (down to
/// the defaults), then the next column in `SQUEEZE_ORDER` drops out, and once only the core is
/// left its columns shrink to their minimums. So hiding a column leaves room for others, and
/// the table never runs past its right edge while its core fits.
pub(crate) fn visible_columns(room: Room, layout: &ColumnLayout<'_>) -> Vec<ColumnSpec> {
    let (mut columns, give) = pick_columns(room, layout);
    layout.fit_widths(&mut columns, room, give);
    columns
}

/// The columns `visible_columns` keeps, at their default widths, and how far they give.
fn pick_columns(room: Room, layout: &ColumnLayout<'_>) -> (Vec<ColumnSpec>, Give) {
    let mut columns: Vec<ColumnSpec> = COLUMNS.into_iter().filter(|c| layout.wants(c)).collect();
    // why: a stable sort keeps a unit's columns in their own order, and the margin first.
    columns.sort_by_key(|c| layout.rank(c));
    if layout.group == GroupBy::Status {
        compact_status(&mut columns);
    }
    let fits =
        |columns: &[ColumnSpec]| layout.needed(columns, room, Give::ToDefault).floor <= room.width;
    for id in SQUEEZE_ORDER {
        if fits(&columns) {
            break;
        }
        columns.retain(|c| c.id != id);
    }
    let give = if fits(&columns) {
        Give::ToDefault
    } else {
        Give::ToMinimum
    };
    (columns, give)
}

/// The resizable columns `visible_columns` shows narrower than the user's width for them (or
/// their default), by key, at the width shown.
fn squeezed_widths(room: Room, layout: &ColumnLayout<'_>) -> Vec<(&'static str, f32)> {
    let (picked, give) = pick_columns(room, layout);
    let mut shown = picked.clone();
    layout.fit_widths(&mut shown, room, give);
    picked
        .iter()
        .zip(&shown)
        .filter_map(|(column, fitted)| match fitted.width {
            Width::Initial(width) if width < layout.span(column, room, give).preferred - 0.5 => {
                Some((column.key, width))
            }
            Width::Initial(_) | Width::Exact | Width::Fixed(_) | Width::Remainder => None,
        })
        .collect()
}

/// The widths to save when the header drags `key` to `width` points. In a table squeezed to
/// fit, the other columns keep the widths they show first; else the drag's width would be
/// squeezed again, so the divider would land short of the pointer and the other columns grow.
pub(crate) fn dragged_widths(
    room: Room,
    layout: &ColumnLayout<'_>,
    (key, width): (&'static str, f32),
) -> Vec<(&'static str, f32)> {
    let mut saved = squeezed_widths(room, layout);
    saved.retain(|(shown, _)| *shown != key);
    saved.push((key, width));
    saved
}

/// The room a table worth showing needs (`room.width` aside): its columns that never drop and
/// the last one that does (Comments), at their default widths, ungrouped, with Title at its
/// minimum. Narrower, the dashboard lists cards instead, since a table of only its core
/// columns would hide the repository, author, reviews and comments that a card still shows.
pub(crate) fn table_minimum(room: Room) -> f32 {
    let defaults = BTreeMap::new();
    let layout = ColumnLayout {
        group: GroupBy::None,
        show_org: false,
        picks: Picks::default(),
        widths: &defaults,
        order: &DEFAULT_ORDER,
    };
    let last = SQUEEZE_ORDER.last().copied();
    let minimal = COLUMNS
        .iter()
        .filter(|c| !SQUEEZE_ORDER.contains(&c.id) || Some(c.id) == last);
    layout.needed(minimal, room, Give::ToDefault).floor
}

/// Grouped by status, the group header already names it: the status cell shows only its glyph.
fn compact_status(columns: &mut [ColumnSpec]) {
    for spec in columns.iter_mut() {
        match spec.id {
            ColumnId::Status => {
                spec.header = "";
                spec.width = Width::Fixed(STATUS_GLYPH);
                spec.min = STATUS_GLYPH;
                spec.sort = None;
            }
            ColumnId::Number => {
                spec.key = "number_left";
                spec.align_right = false;
                spec.width = Width::Initial(NUMBER_COMPACT);
            }
            ColumnId::Margin
            | ColumnId::Flags
            | ColumnId::Title
            | ColumnId::Repo
            | ColumnId::Author
            | ColumnId::Reviews
            | ColumnId::Required
            | ColumnId::Comments
            | ColumnId::Age
            | ColumnId::Org => {}
        }
    }
}

/// Listed columns the user chose that `visible` (this frame's columns) had no room for.
pub(crate) fn squeezed<'a>(
    visible: &'a [ColumnSpec],
    picks: Picks<'a>,
) -> impl Iterator<Item = ColumnId> + 'a {
    listed()
        .filter(move |c| chosen(c, picks) && !visible.iter().any(|v| v.id == c.id))
        .map(|c| c.id)
}

/// How a listed column's check reads in the column menu and the settings checklist.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum ColumnCheck {
    /// The user hid it.
    Off,
    /// In the table.
    On,
    /// Chosen, but the table has no room for it right now.
    Squeezed,
}

/// The user's column choices and what the table's width overrode last frame.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Choices<'a> {
    pub picks: Picks<'a>,
    /// Chosen columns the table had no room for last frame (`squeezed`).
    pub squeezed: &'a [ColumnId],
}

impl Choices<'_> {
    pub(crate) fn check(&self, column: &ColumnSpec) -> ColumnCheck {
        if !chosen(column, self.picks) {
            ColumnCheck::Off
        } else if self.squeezed.contains(&column.id) {
            ColumnCheck::Squeezed
        } else {
            ColumnCheck::On
        }
    }
}

/// A stable id for the visible set, so `egui_extras` width state never mismatches the count.
/// It ignores their order: moving a column must not make the table forget its scroll offset.
pub(crate) fn columns_hash(columns: &[ColumnSpec]) -> u64 {
    columns
        .iter()
        .fold(0_u64, |hash, c| hash | (1 << c.id as u64))
}

#[cfg(test)]
mod tests;
