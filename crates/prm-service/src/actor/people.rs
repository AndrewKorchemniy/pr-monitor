//! The people picker: instant local results, then directory results (latest query wins), and
//! resolving directory entries to the VSIDs PR data uses.

use std::sync::Arc;

use prm_ado::AdoError;
use prm_config::Severity;
use prm_domain::{
    OrgId, Vsid,
    ids::RequestId,
    person::{PersonCandidate, merge_candidates},
};

use super::Service;
use crate::{
    consts::{NOTICE_TTL, PEOPLE_REMOTE_MIN_CHARS, SEARCH_DEBOUNCE, SEARCH_LIMIT},
    errors::UiError,
    event::{AppEvent, Notice, SearchPhase},
    jobs::{JobKind, JobOutput, JobTag, people},
    text,
};

impl Service {
    pub(super) fn search_people(&mut self, org: &OrgId, query: &str, request: RequestId) {
        let Some(idx) = self.index(org) else {
            return;
        };
        let query: Arc<str> = Arc::from(query.trim());
        let local = self.orgs[idx].data.people.search(&query, SEARCH_LIMIT);
        self.deps.events.emit(AppEvent::PeopleResults {
            request,
            org: org.clone(),
            query: query.clone(),
            phase: SearchPhase::Local,
            candidates: local.into(),
        });
        self.orgs[idx].link.abort_search();
        let signed_in = self.orgs[idx].live.auth_state.is_signed_in();
        if query.chars().count() < PEOPLE_REMOTE_MIN_CHARS || !signed_in {
            return;
        }
        let api = self.orgs[idx].link.api.clone();
        let id = org.clone();
        let handle = self.spawn(JobTag::org(org, JobKind::Search), async move {
            let result = people::search_directory(api, query.clone(), SEARCH_DEBOUNCE).await;
            JobOutput::Search {
                org: id,
                request,
                query,
                result,
            }
        });
        self.orgs[idx].link.search = Some((request, handle));
    }

    pub(super) fn on_search(
        &mut self,
        org: &OrgId,
        request: RequestId,
        query: &Arc<str>,
        result: Result<Vec<PersonCandidate>, AdoError>,
    ) {
        let Some(idx) = self.index(org) else {
            return;
        };
        let link = &mut self.orgs[idx].link;
        if link.search.as_ref().map(|(r, _)| *r) != Some(request) {
            return;
        }
        link.search = None;
        let local = self.orgs[idx].data.people.search(query, SEARCH_LIMIT);
        let candidates = match result {
            Ok(remote) => merge_candidates(local, remote, SEARCH_LIMIT),
            Err(error) => {
                tracing::warn!(org = %org, %error, "directory search failed");
                self.org_notice(org, text::SEARCH_FAILED);
                local
            }
        };
        self.deps.events.emit(AppEvent::PeopleResults {
            request,
            org: org.clone(),
            query: query.clone(),
            phase: SearchPhase::Final,
            candidates: candidates.into(),
        });
    }

    pub(super) fn resolve_person(
        &mut self,
        org: &OrgId,
        candidate: PersonCandidate,
        request: RequestId,
    ) {
        let Some(idx) = self.index(org) else {
            return;
        };
        if let Some(person) = candidate.to_person() {
            let now = self.deps.clock.now();
            self.orgs[idx].data.people.harvest([person.clone()], now);
            self.emit_resolved(org, request, Ok(person));
            return;
        }
        let Some(descriptor) = candidate.descriptor.clone() else {
            let error = UiError::new(text::PERSON_UNRESOLVED);
            self.emit_resolved(org, request, Err(error));
            return;
        };
        let api = self.orgs[idx].link.api.clone();
        let id = org.clone();
        self.spawn(JobTag::org(org, JobKind::Resolve), async move {
            let result = api.storage_key(&descriptor).await;
            JobOutput::Resolve {
                org: id,
                request,
                candidate,
                result,
            }
        });
    }

    pub(super) fn on_resolve(
        &mut self,
        org: &OrgId,
        request: RequestId,
        candidate: &PersonCandidate,
        result: Result<Vsid, AdoError>,
    ) {
        let Some(idx) = self.index(org) else {
            return;
        };
        let person = result.map_err(|e| UiError::from(&e)).and_then(|id| {
            people::resolved(candidate, id).ok_or_else(|| UiError::new(text::PERSON_UNRESOLVED))
        });
        if let Ok(person) = &person {
            let now = self.deps.clock.now();
            self.orgs[idx].data.people.harvest([person.clone()], now);
            self.mark_persist();
        }
        self.emit_resolved(org, request, person);
    }

    fn emit_resolved(
        &self,
        org: &OrgId,
        request: RequestId,
        result: Result<prm_domain::Person, UiError>,
    ) {
        self.deps.events.emit(AppEvent::PersonResolved {
            request,
            org: org.clone(),
            result,
        });
    }

    pub(super) fn org_notice(&mut self, org: &OrgId, text: &str) {
        self.notice(Notice {
            severity: Severity::Warning,
            text: Arc::from(text),
            org: Some(org.clone()),
            action: None,
            ttl: NOTICE_TTL,
        });
    }
}
