//! A people list with a directory search combobox. The field comes first and never moves:
//! typing opens a dropdown under it (a hint under 3 letters, then the matches as they arrive,
//! local ones first), Up and Down move the highlight, Enter or a click adds the person and
//! clears the field for the next name, and Esc closes the dropdown. The chosen people sit below
//! as compact pills, each with a flush remove button.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use eframe::egui;
use prm_domain::{
    OrgId, Person,
    ids::RequestId,
    person::{PersonCandidate, merge_candidates},
};
use prm_service::{AppEvent, SearchPhase, UiError};

use crate::{
    copy,
    frame::FrameCx,
    widgets::{self, controls},
};

mod combobox;
mod pills;

pub(crate) use pills::chips;

/// Directory searches start at this many characters.
const MIN_QUERY: usize = 3;
/// The most matches the dropdown lists.
const MAX_ROWS: usize = 8;

static NEXT_REQUEST: AtomicU64 = AtomicU64::new(1);

fn next_request() -> RequestId {
    RequestId(NEXT_REQUEST.fetch_add(1, Ordering::Relaxed))
}

/// Where the directory search for the current query stands.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Search {
    Idle,
    Searching(RequestId),
    /// Finished; the line to show when nothing is left to offer ("No people match …").
    Done(Arc<str>),
    /// The directory was not searched (the org is signed out); the matches are known people
    /// only, and this muted line says so.
    LocalOnly(Arc<str>),
    /// The directory failed; the matches are known people only, and this line says so.
    Failed(Arc<str>),
}

/// Which way an arrow key moves the highlight.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Up,
    Down,
}

/// What choosing a match does.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Pick {
    /// The match carries its id: add the person now.
    Add(Person),
    /// A directory entry: the service resolves it under this request first.
    Resolve(RequestId, PersonCandidate),
}

/// One picker's transient state (its people live in the settings draft).
#[derive(Clone, Debug)]
pub(crate) struct PeoplePicker {
    pub id: &'static str,
    query: String,
    search: Search,
    results: Arc<[PersonCandidate]>,
    /// Directory entries picked and not resolved yet; each is added when its result arrives.
    resolving: Vec<RequestId>,
    /// Why the last pick could not be added.
    error: Option<Arc<str>>,
    /// The dropdown is open.
    open: bool,
    /// The highlighted row, an index into the offered matches.
    highlight: usize,
}

impl PeoplePicker {
    pub(crate) fn new(id: &'static str) -> Self {
        Self {
            id,
            query: String::new(),
            search: Search::Idle,
            results: Arc::from([]),
            resolving: Vec::new(),
            error: None,
            open: false,
            highlight: 0,
        }
    }

    /// Takes search and resolve results meant for this picker; returns a resolved person.
    pub(crate) fn on_event(&mut self, event: &AppEvent) -> Option<Person> {
        if let AppEvent::PeopleResults {
            request,
            query,
            phase,
            candidates,
            ..
        } = event
        {
            self.on_results(*request, query, phase, candidates);
            return None;
        }
        let AppEvent::PersonResolved {
            request, result, ..
        } = event
        else {
            return None;
        };
        self.on_resolved(*request, result)
    }

    /// Local results arrive first, then the directory's; stale requests are ignored. Until the
    /// directory answers, the earlier matches that still fit stay after the local ones.
    fn on_results(
        &mut self,
        request: RequestId,
        query: &str,
        phase: &SearchPhase,
        candidates: &Arc<[PersonCandidate]>,
    ) {
        if self.search != Search::Searching(request) {
            return;
        }
        let finished = match phase {
            SearchPhase::Local => {
                let kept = self.results.to_vec();
                let limit = candidates.len() + kept.len();
                self.results = merge_candidates(candidates.to_vec(), kept, limit).into();
                return;
            }
            SearchPhase::Final => Search::Done(Arc::from(copy::no_people(query))),
            SearchPhase::LocalOnly => Search::LocalOnly(Arc::from(copy::SIGN_IN_TO_SEARCH)),
            SearchPhase::Failed(error) => Search::Failed(error.title.clone()),
        };
        self.search = finished;
        self.results = candidates.clone();
    }

    /// A person picked earlier is resolved (or could not be). Once nothing is left to show
    /// (no other pick pending, nothing typed) the dropdown closes; a failure opens it to say why.
    fn on_resolved(
        &mut self,
        request: RequestId,
        result: &Result<Person, UiError>,
    ) -> Option<Person> {
        let pending = self.resolving.iter().position(|r| *r == request)?;
        self.resolving.remove(pending);
        match result {
            Ok(person) => {
                self.open &= self.has_rows();
                Some(person.clone())
            }
            Err(error) => {
                self.error = Some(error.title.clone());
                self.open = true;
                None
            }
        }
    }

    /// The query changed: the dropdown opens (or closes once the field is empty) and shows the
    /// hint, or a new search starts, whose request is returned.
    fn typed(&mut self) -> Option<RequestId> {
        self.error = None;
        self.highlight = 0;
        let query = self.query.trim().to_lowercase();
        self.open = !query.is_empty() || !self.resolving.is_empty();
        if query.chars().count() < MIN_QUERY {
            self.search = Search::Idle;
            self.results = Arc::from([]);
            return None;
        }
        // why: the matches that still fit stay until the new results arrive, so the dropdown
        // does not shrink to "Searching…" and grow back with every letter.
        self.results = self
            .results
            .iter()
            .filter(|candidate| mentions(candidate, &query))
            .cloned()
            .collect();
        let request = next_request();
        self.search = Search::Searching(request);
        Some(request)
    }

    /// Moves the highlight among `count` offered matches.
    pub(crate) fn step(&mut self, step: Step, count: usize) {
        self.highlight = stepped(self.highlight, step, count);
    }

    /// Chooses `candidate`: clears the field for the next name and closes the dropdown, or
    /// keeps it open on "Adding…" while a directory entry resolves.
    fn pick(&mut self, candidate: &PersonCandidate) -> Pick {
        self.reset();
        if let Some(person) = candidate.to_person() {
            self.open = !self.resolving.is_empty();
            return Pick::Add(person);
        }
        let request = next_request();
        self.resolving.push(request);
        self.open = true;
        Pick::Resolve(request, candidate.clone())
    }

    /// Empties the field, forgets its search and closes the dropdown (a dialog closing). Picks
    /// still being resolved are kept, so those people are still added.
    pub(crate) fn reset(&mut self) {
        self.query.clear();
        self.search = Search::Idle;
        self.results = Arc::from([]);
        self.error = None;
        self.highlight = 0;
        self.open = false;
    }

    /// Closes the dropdown and keeps what was typed.
    pub(crate) const fn close(&mut self) {
        self.open = false;
    }

    /// The highlighted match among those offered next to `people`.
    fn highlighted<'a>(&'a self, people: &'a [Person]) -> Option<&'a PersonCandidate> {
        let count = offered(&self.results, people).count();
        let last = count.checked_sub(1)?;
        offered(&self.results, people).nth(self.highlight.min(last))
    }

    /// Something to show: progress on a pick, its failure, or a typed query.
    fn has_rows(&self) -> bool {
        !self.resolving.is_empty() || self.error.is_some() || !self.query.trim().is_empty()
    }
}

#[cfg(test)]
impl PeoplePicker {
    /// The state after typing `query` and receiving `found` in `phase` (screens and gallery).
    pub(crate) fn show_matches(
        &mut self,
        query: &str,
        phase: &SearchPhase,
        found: &[PersonCandidate],
    ) {
        query.clone_into(&mut self.query);
        if let Some(request) = self.typed() {
            self.on_results(request, query, phase, &Arc::from(found));
        }
    }
}

/// The highlight after one arrow key among `count` rows, wrapping from the last row to the
/// first and back; 0 when there are none.
const fn stepped(highlight: usize, step: Step, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    let current = if highlight < count {
        highlight
    } else {
        count - 1
    };
    match step {
        Step::Down => (current + 1) % count,
        Step::Up => (current + count - 1) % count,
    }
}

/// `candidate`'s name or sign-in name contains `query` (already lowercase).
fn mentions(candidate: &PersonCandidate, query: &str) -> bool {
    let contains = |text: &str| text.to_lowercase().contains(query);
    contains(&candidate.name) || candidate.unique_name.as_deref().is_some_and(contains)
}

/// The matches worth offering: not chosen already, and at most `MAX_ROWS`.
fn offered<'a>(
    results: &'a [PersonCandidate],
    people: &'a [Person],
) -> impl Iterator<Item = &'a PersonCandidate> + 'a {
    results
        .iter()
        .filter(|candidate| !people.iter().any(|p| candidate.is_person(p)))
        .take(MAX_ROWS)
}

/// The search field with its dropdown, then the chosen people as pills; edits `people` in
/// place. Without a saved org there is nothing to search yet.
pub(crate) fn people_picker(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    picker: &mut PeoplePicker,
    org: Option<&OrgId>,
    people: &mut Vec<Person>,
) {
    let theme = cx.theme.clone();
    let dropdown = if let Some(org) = org {
        combobox::search(ui, cx, picker, org, people)
    } else {
        widgets::muted(ui, &theme, copy::SAVE_ORG_FIRST);
        None
    };
    if !people.is_empty() {
        ui.add_space(theme.tokens.metrics.space.sm);
        chips(ui, &theme, people);
    }
    // why: the open dropdown floats over what follows; room under it keeps a dialog's action
    // bar below it rather than under it.
    if let Some(bottom) = dropdown {
        let short = bottom + controls::MENU_GAP - ui.cursor().top();
        ui.add_space(short.max(0.0));
    }
}

#[cfg(test)]
mod tests;
