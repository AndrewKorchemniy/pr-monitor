//! A people list with directory search: chips for the chosen people, a field that searches
//! after 3 letters, and results that resolve to a person when picked.

use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use eframe::egui::{self, RichText};
use prm_domain::{OrgId, Person, ids::RequestId, person::PersonCandidate};
use prm_service::{AppEvent, Command, SearchPhase};

use crate::{actions::UiAction, copy, frame::FrameCx, widgets};

/// Directory searches start at this many characters.
pub(crate) const MIN_QUERY: usize = 3;

static NEXT_REQUEST: AtomicU64 = AtomicU64::new(1);

pub(crate) fn next_request() -> RequestId {
    RequestId(NEXT_REQUEST.fetch_add(1, Ordering::Relaxed))
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Search {
    Idle,
    Searching(RequestId),
    Done,
}

/// One picker's transient state (its people live in the settings draft).
#[derive(Clone, Debug)]
pub(crate) struct PeoplePicker {
    pub id: &'static str,
    query: String,
    search: Search,
    results: Arc<[PersonCandidate]>,
    resolving: Option<RequestId>,
    error: Option<Arc<str>>,
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
        }
    }

    /// Takes search and resolve results meant for this picker; returns a resolved person.
    pub(crate) fn on_event(&mut self, event: &AppEvent) -> Option<Person> {
        if let AppEvent::PeopleResults {
            request,
            phase,
            candidates,
            ..
        } = event
        {
            self.on_results(*request, *phase, candidates);
            return None;
        }
        let AppEvent::PersonResolved {
            request, result, ..
        } = event
        else {
            return None;
        };
        if self.resolving != Some(*request) {
            return None;
        }
        self.resolving = None;
        result
            .as_ref()
            .map_err(|error| self.error = Some(error.title.clone()))
            .ok()
            .cloned()
    }

    /// Local results arrive first, then the directory's; stale requests are ignored.
    fn on_results(
        &mut self,
        request: RequestId,
        phase: SearchPhase,
        candidates: &Arc<[PersonCandidate]>,
    ) {
        if self.search != Search::Searching(request) {
            return;
        }
        self.results = candidates.clone();
        if phase == SearchPhase::Final {
            self.search = Search::Done;
        }
    }
}

/// Shows the chips and the search field; edits `people` in place.
pub(crate) fn people_picker(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    picker: &mut PeoplePicker,
    org: Option<&OrgId>,
    people: &mut Vec<Person>,
) {
    let theme = cx.theme.clone();
    chips(ui, &theme, people);
    let Some(org) = org else {
        widgets::muted(ui, &theme, copy::SAVE_ORG_FIRST);
        return;
    };
    let response = ui.add(
        egui::TextEdit::singleline(&mut picker.query)
            .id(egui::Id::unique(("people", picker.id)))
            .hint_text(copy::ADD_PEOPLE)
            .desired_width(320.0),
    );
    if response.changed() {
        start_search(cx, picker, org);
    }
    status_line(ui, &theme, picker);
    results(ui, cx, picker, org, people);
}

fn start_search(cx: &mut FrameCx<'_>, picker: &mut PeoplePicker, org: &OrgId) {
    picker.error = None;
    let query = picker.query.trim().to_owned();
    if query.chars().count() < MIN_QUERY {
        picker.search = Search::Idle;
        picker.results = Arc::from([]);
        return;
    }
    let request = next_request();
    picker.search = Search::Searching(request);
    cx.push(UiAction::Send(Command::SearchPeople {
        org: org.clone(),
        query,
        request,
    }));
}

fn status_line(ui: &mut egui::Ui, theme: &crate::theme::UiTheme, picker: &PeoplePicker) {
    let text = if let Some(error) = &picker.error {
        error.to_string()
    } else if picker.resolving.is_some() {
        copy::ADDING.to_owned()
    } else {
        match picker.search {
            Search::Idle if !picker.query.trim().is_empty() => copy::SEARCH_MIN.to_owned(),
            Search::Searching(_) if picker.results.is_empty() => copy::SEARCHING.to_owned(),
            Search::Done if picker.results.is_empty() => copy::no_people(picker.query.trim()),
            Search::Idle | Search::Searching(_) | Search::Done => return,
        }
    };
    widgets::muted(ui, theme, &text);
}

fn chips(ui: &mut egui::Ui, theme: &crate::theme::UiTheme, people: &mut Vec<Person>) {
    let mut remove = None;
    ui.horizontal_wrapped(|ui| {
        for (index, person) in people.iter().enumerate() {
            let mut text = person.display_name().to_string();
            if person.group {
                text.push_str(" · ");
                text.push_str(copy::GROUP_TAG);
            }
            let label = RichText::new(format!("{text}  ×")).font(theme.fonts.label.clone());
            let response = ui.add(
                egui::Button::new(label)
                    .corner_radius(egui::CornerRadius::same(theme.tokens.metrics.radius.pill)),
            );
            let response = match &person.unique_name {
                Some(unique) => response.on_hover_text(unique.as_ref()),
                None => response,
            };
            if response.clicked() {
                remove = Some(index);
            }
        }
    });
    if let Some(index) = remove {
        people.remove(index);
    }
}

fn results(
    ui: &mut egui::Ui,
    cx: &mut FrameCx<'_>,
    picker: &mut PeoplePicker,
    org: &OrgId,
    people: &mut Vec<Person>,
) {
    let mut chosen = None;
    for candidate in picker.results.iter().take(8) {
        let already = candidate
            .id
            .is_some_and(|id| people.iter().any(|p| p.id == id));
        if already {
            continue;
        }
        let mut text = candidate.name.to_string();
        if let Some(unique) = &candidate.unique_name {
            text.push_str("  ");
            text.push_str(unique);
        }
        if ui.selectable_label(false, text).clicked() {
            chosen = Some(candidate.clone());
        }
    }
    let Some(candidate) = chosen else {
        return;
    };
    picker.query.clear();
    picker.results = Arc::from([]);
    picker.search = Search::Idle;
    if let Some(person) = candidate.to_person() {
        people.push(person);
    } else {
        let request = next_request();
        picker.resolving = Some(request);
        cx.push(UiAction::Send(Command::ResolvePerson {
            org: org.clone(),
            candidate,
            request,
        }));
    }
}
