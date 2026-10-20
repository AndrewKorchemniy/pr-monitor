//! Paging state machines, kept pure so their edge cases are unit-tested.
//!
//! PR lists page with `$top`/`$skip`, and the server may return fewer than `$top` items even
//! when more exist, so a short page is not the end: only an empty page is.

use std::{collections::HashSet, hash::Hash};

use http::HeaderMap;

use crate::consts::{H_CONTINUATION, MAX_CONTINUATION_PAGES};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PageStep {
    Continue {
        skip: u32,
    },
    Done,
    /// Stopped at the page limit; results may be incomplete.
    Truncated,
}

/// `$skip` paging with de-duplication across pages.
#[derive(Debug)]
pub struct SkipPager<K: Eq + Hash> {
    skip: u32,
    pages: u32,
    max_pages: u32,
    seen: HashSet<K>,
}

impl<K: Eq + Hash> SkipPager<K> {
    pub fn new(max_pages: u32) -> Self {
        Self {
            skip: 0,
            pages: 0,
            max_pages,
            seen: HashSet::new(),
        }
    }

    pub const fn skip(&self) -> u32 {
        self.skip
    }

    /// Returns the page's new unique items and what to do next:
    /// - an empty page ends paging;
    /// - `skip` advances by the number of items *returned*, not by `$top`;
    /// - a non-empty page with no new items ends paging (the server ignored `$skip`);
    /// - reaching `max_pages` truncates.
    pub fn accept<T>(&mut self, page: Vec<T>, key: impl Fn(&T) -> K) -> (Vec<T>, PageStep) {
        if page.is_empty() {
            return (page, PageStep::Done);
        }
        self.pages += 1;
        self.skip = self
            .skip
            .saturating_add(u32::try_from(page.len()).unwrap_or(u32::MAX));
        let fresh: Vec<T> = page
            .into_iter()
            .filter(|item| self.seen.insert(key(item)))
            .collect();
        let step = if fresh.is_empty() {
            PageStep::Done
        } else if self.pages >= self.max_pages {
            PageStep::Truncated
        } else {
            PageStep::Continue { skip: self.skip }
        };
        (fresh, step)
    }
}

/// Continuation-token paging: stops on a missing, empty or repeated token, or at the page limit.
#[derive(Debug, Default)]
pub struct ContinuationGuard {
    seen: HashSet<String>,
    pages: u32,
}

impl ContinuationGuard {
    /// The token for the next page, if paging should continue.
    pub fn next(&mut self, headers: &HeaderMap) -> Option<String> {
        self.pages += 1;
        if self.pages >= MAX_CONTINUATION_PAGES {
            return None;
        }
        let token = headers.get(H_CONTINUATION)?.to_str().ok()?.trim();
        if token.is_empty() || !self.seen.insert(token.to_owned()) {
            return None;
        }
        Some(token.to_owned())
    }
}

#[cfg(test)]
mod tests;
