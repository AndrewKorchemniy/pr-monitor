//! The filter box: `@author repo:name org:x !123 is:draft is:unread is:conflict` plus free
//! terms (all must match, case-insensitive).

use serde::{Deserialize, Serialize};

use crate::{classify::PrStatus, ids::OrgId, view::row::PrRow};

/// What the user is filtering by, persisted with the UI state.
#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewQuery {
    pub text: String,
    pub status: StatusFilter,
    pub org: Option<OrgId>,
    pub unread_only: bool,
    pub hide_drafts: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatusFilter {
    NeedsReview,
    ActiveComments,
    Reviewed,
    /// Last because `#[serde(other)]` must be the final variant.
    #[default]
    #[serde(other)]
    All,
}

impl StatusFilter {
    pub fn matches(self, status: PrStatus) -> bool {
        match self {
            Self::All => true,
            Self::NeedsReview => status == PrStatus::NeedsReview,
            Self::ActiveComments => status == PrStatus::ActiveComments,
            Self::Reviewed => status == PrStatus::Reviewed,
        }
    }
}

/// The filter text, parsed once per keystroke.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ParsedFilter {
    /// Lowercase substrings that must all appear in the row's haystack.
    pub terms: Vec<String>,
    pub number: Option<i32>,
    pub author: Option<String>,
    pub repo: Option<String>,
    pub org: Option<String>,
    pub is_draft: Option<bool>,
    pub is_unread: bool,
    pub is_conflict: bool,
}

pub fn parse_filter(text: &str) -> ParsedFilter {
    let mut filter = ParsedFilter::default();
    for token in text.split_whitespace().map(str::to_lowercase) {
        apply_token(&mut filter, token);
    }
    filter
}

fn apply_token(filter: &mut ParsedFilter, token: String) {
    if let Some(number) = token
        .strip_prefix(['!', '#'])
        .and_then(|n| n.parse::<i32>().ok())
    {
        filter.number = Some(number);
    } else if let Some(author) = token.strip_prefix('@').filter(|a| !a.is_empty()) {
        filter.author = Some(author.to_owned());
    } else if let Some(repo) = token.strip_prefix("repo:") {
        filter.repo = Some(repo.to_owned());
    } else if let Some(org) = token.strip_prefix("org:") {
        filter.org = Some(org.to_owned());
    } else if !apply_is_flag(filter, &token) {
        filter.terms.push(token);
    }
}

/// Handles `is:draft`, `is:!draft`, `is:unread` and `is:conflict`; false for anything else.
fn apply_is_flag(filter: &mut ParsedFilter, token: &str) -> bool {
    match token.strip_prefix("is:") {
        Some("draft") => filter.is_draft = Some(true),
        Some("!draft") => filter.is_draft = Some(false),
        Some("unread") => filter.is_unread = true,
        Some("conflict") => filter.is_conflict = true,
        Some(_) | None => return false,
    }
    true
}

pub fn row_matches(row: &PrRow, query: &ViewQuery, filter: &ParsedFilter) -> bool {
    query.status.matches(row.status)
        && query.org.as_ref().is_none_or(|org| *org == row.key.org)
        && (!query.unread_only || row.unread)
        && (!query.hide_drafts || !row.draft)
        && filter_matches(row, filter)
}

fn filter_matches(row: &PrRow, filter: &ParsedFilter) -> bool {
    let contains = |field: &str, wanted: &Option<String>| {
        wanted.as_ref().is_none_or(|w| field.contains(w.as_str()))
    };
    filter.number.is_none_or(|n| n == row.key.number.0)
        && contains(&row.author_key, &filter.author)
        && contains(&row.repo_key, &filter.repo)
        && contains(&row.org_label.to_lowercase(), &filter.org)
        && filter.is_draft.is_none_or(|d| d == row.draft)
        && (!filter.is_unread || row.unread)
        && (!filter.is_conflict || row.conflicts)
        && filter
            .terms
            .iter()
            .all(|t| row.haystack.contains(t.as_str()))
}
