//! The command palette: fuzzy matching over commands, sign-in shortcuts, themes, and "!123"
//! jumps to a pull request.

use prm_domain::{OrgId, PrKey, view::PrRow};

use crate::commands::{COMMANDS, CommandId};

/// Case-insensitive subsequence match: +32 prefix, +16 word start, +8 consecutive, −1 per
/// skipped character. `None` when the query is not a subsequence.
pub(crate) fn score(query: &str, candidate: &str) -> Option<i32> {
    let query: Vec<char> = query
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if query.is_empty() {
        return Some(0);
    }
    let text: Vec<char> = candidate.to_lowercase().chars().collect();
    let mut total = 0;
    let mut qi = 0;
    let mut previous: Option<usize> = None;
    for (i, c) in text.iter().enumerate() {
        if qi == query.len() {
            break;
        }
        if *c != query[qi] {
            continue;
        }
        let word_start = i == 0 || !text[i - 1].is_alphanumeric();
        total += match (i, word_start, previous) {
            (0, _, _) => 32,
            (_, true, _) => 16,
            (_, false, Some(p)) if p + 1 == i => 8,
            _ => 0,
        };
        let skipped = previous.map_or(i, |p| i - p - 1);
        total -= i32::try_from(skipped).unwrap_or(i32::MAX);
        previous = Some(i);
        qi += 1;
    }
    (qi == query.len()).then_some(total)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PaletteItem {
    Command(CommandId),
    SignIn(OrgId),
    Theme(String),
    GoTo(PrKey),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct PaletteState {
    pub query: String,
    pub selected: usize,
    /// The row last scrolled into view. The highlight is scrolled into view once each time it
    /// moves, so between moves the list stays where the wheel left it.
    pub revealed: Option<usize>,
    /// Recently run commands, newest first.
    pub recent: Vec<CommandId>,
}

impl PaletteState {
    /// Ready to open again: no query, and the first row highlighted and scrolled into view.
    pub(crate) fn reopen(&mut self) {
        self.query.clear();
        self.selected = 0;
        self.revealed = None;
    }
}

/// What the palette can offer besides the static commands.
pub(crate) struct PaletteSources<'a> {
    pub sign_in: Vec<(OrgId, String)>,
    pub themes: Vec<(String, String)>,
    pub rows: &'a [PrRow],
}

/// Most relevant first; recent commands lead an empty query.
pub(crate) fn build_items(
    state: &PaletteState,
    sources: &PaletteSources<'_>,
) -> Vec<(PaletteItem, String)> {
    let query = state.query.trim();
    if let Some(number) = query.strip_prefix(['!', '#']) {
        return go_to(number, sources.rows);
    }
    let mut scored: Vec<(i32, PaletteItem, String)> = Vec::new();
    let mut add = |item: PaletteItem, title: String| {
        if let Some(points) = score(query, &title) {
            scored.push((points, item, title));
        }
    };
    for spec in COMMANDS.iter().filter(|c| c.palette) {
        add(PaletteItem::Command(spec.id), spec.title.to_owned());
    }
    for (org, label) in &sources.sign_in {
        add(
            PaletteItem::SignIn(org.clone()),
            format!("Sign in to {label}"),
        );
    }
    for (id, name) in &sources.themes {
        add(PaletteItem::Theme(id.clone()), format!("Theme: {name}"));
    }
    if query.is_empty() {
        let rank = |item: &PaletteItem| match item {
            PaletteItem::Command(id) => state.recent.iter().position(|r| r == id),
            PaletteItem::SignIn(_) | PaletteItem::Theme(_) | PaletteItem::GoTo(_) => None,
        };
        scored.sort_by_key(|(_, item, _)| rank(item).unwrap_or(usize::MAX));
    } else {
        scored.sort_by_key(|(points, _, _)| std::cmp::Reverse(*points));
    }
    scored
        .into_iter()
        .map(|(_, item, title)| (item, title))
        .collect()
}

/// Rows whose number starts with the digits (or whose title matches the rest).
fn go_to(text: &str, rows: &[PrRow]) -> Vec<(PaletteItem, String)> {
    let text = text.trim();
    rows.iter()
        .filter(|row| {
            row.key.number.0.to_string().starts_with(text) || score(text, &row.title).is_some()
        })
        .take(20)
        .map(|row| {
            let title = format!("{} {}", row.number_text, row.title);
            (PaletteItem::GoTo(row.key.clone()), title)
        })
        .collect()
}

#[cfg(test)]
mod tests;
