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
use prm_domain::{OrgId, Person, ids::RequestId, person::PersonCandidate};
use prm_service::{AppEvent, SearchPhase, UiError};

use crate::{copy, frame::FrameCx, widgets};

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
    resolving: Option<RequestId>,
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
            resolving: None,
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

    /// Local results arrive first, then the directory's; stale requests are ignored.
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
        self.results = candidates.clone();
        self.search = match phase {
            SearchPhase::Local => Search::Searching(request),
            SearchPhase::Final => Search::Done(Arc::from(copy::no_people(query))),
            SearchPhase::Failed(error) => Search::Failed(error.title.clone()),
        };
    }

    /// The person picked earlier is resolved (or could not be): the dropdown stops saying
    /// "Adding…" and closes unless the user is typing the next name or it failed.
    fn on_resolved(
        &mut self,
        request: RequestId,
        result: &Result<Person, UiError>,
    ) -> Option<Person> {
        if self.resolving != Some(request) {
            return None;
        }
        self.resolving = None;
        match result {
            Ok(person) => {
                self.open = !self.query.trim().is_empty();
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
        self.results = Arc::from([]);
        let query = self.query.trim();
        self.open = !query.is_empty() || self.resolving.is_some();
        if query.chars().count() < MIN_QUERY {
            self.search = Search::Idle;
            return None;
        }
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
        self.query.clear();
        self.search = Search::Idle;
        self.results = Arc::from([]);
        self.error = None;
        self.highlight = 0;
        if let Some(person) = candidate.to_person() {
            self.open = self.resolving.is_some();
            return Pick::Add(person);
        }
        let request = next_request();
        self.resolving = Some(request);
        self.open = true;
        Pick::Resolve(request, candidate.clone())
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
        self.resolving.is_some() || self.error.is_some() || !self.query.trim().is_empty()
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
    match org {
        Some(org) => combobox::search(ui, cx, picker, org, people),
        None => {
            widgets::muted(ui, &theme, copy::SAVE_ORG_FIRST);
        }
    }
    if !people.is_empty() {
        ui.add_space(theme.tokens.metrics.space.sm);
        chips(ui, &theme, people);
    }
}

#[cfg(test)]
mod tests;
