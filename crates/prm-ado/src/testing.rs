//! Fakes for tests here and in downstream crates (enabled by `test-support`).
#![allow(
    clippy::panic,
    reason = "test fixtures fail loudly on bad literals; the lint is silent under cfg(test), so expect would be unfulfilled there"
)]

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use bytes::Bytes;
use http::{HeaderMap, HeaderValue, StatusCode, header};
use prm_domain::{
    OrgId, Person, PrKey, Vsid,
    auth::{AccessToken, AuthError, AuthHeader, AuthMethod},
    ids::{PrNumber, ProjectId, RepoId},
    model::{PrState, Project, PullRequest, Repository, ThreadDigest},
    person::PersonCandidate,
    plan::PrQuery,
    secret::{Secret, TokenId},
};

use crate::{
    api::{AdoApi, OrgWideProbe},
    client::ClientPolicy,
    connector::AdoConnector,
    credential::Credential,
    error::AdoError,
    http::{HttpRequest, HttpResponse, HttpTransport, TransportError},
    rate::RateSnapshot,
    urls::OrgEndpoint,
};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A fixture file from `crates/prm-ado/tests/fixtures`.
#[macro_export]
macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/",
            $name
        ))
    };
}

/// A bearer token with a predictable id.
pub fn token(value: &str) -> AccessToken {
    let secret = Secret::new(value);
    AccessToken {
        id: secret.id(),
        header: AuthHeader::Bearer(secret),
        expires_at: None,
        method: AuthMethod::AzureCli,
    }
}

/// A scripted HTTP response.
#[derive(Clone, Debug)]
pub struct Scripted {
    pub result: Result<HttpResponse, TransportError>,
    pub delay: Option<Duration>,
}

impl Scripted {
    pub fn json(status: u16, body: &str) -> Self {
        Self::with_type(status, body, "application/json; charset=utf-8")
    }

    pub fn with_type(status: u16, body: &str, content_type: &'static str) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
        Self {
            result: Ok(HttpResponse {
                status: StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                headers,
                body: Bytes::copy_from_slice(body.as_bytes()),
            }),
            delay: None,
        }
    }

    pub fn error(error: TransportError) -> Self {
        Self {
            result: Err(error),
            delay: None,
        }
    }

    pub fn header(mut self, name: &'static str, value: &'static str) -> Self {
        if let Ok(response) = &mut self.result {
            response
                .headers
                .insert(name, HeaderValue::from_static(value));
        }
        self
    }

    pub fn delayed(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }
}

/// Replays scripted responses in order and records requests. Counts peak concurrency.
#[derive(Debug, Default)]
pub struct FakeTransport {
    script: Mutex<VecDeque<Scripted>>,
    requests: Mutex<Vec<HttpRequest>>,
    in_flight: AtomicUsize,
    peak_in_flight: AtomicUsize,
}

impl FakeTransport {
    pub fn new(script: impl IntoIterator<Item = Scripted>) -> Self {
        Self {
            script: Mutex::new(script.into_iter().collect()),
            ..Self::default()
        }
    }

    pub fn push(&self, scripted: Scripted) {
        lock(&self.script).push_back(scripted);
    }

    pub fn requests(&self) -> Vec<HttpRequest> {
        lock(&self.requests).clone()
    }

    pub fn peak_in_flight(&self) -> usize {
        self.peak_in_flight.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl HttpTransport for FakeTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        lock(&self.requests).push(request);
        let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak_in_flight.fetch_max(now, Ordering::SeqCst);
        let next = lock(&self.script).pop_front();
        if let Some(delay) = next.as_ref().and_then(|s| s.delay) {
            tokio::time::sleep(delay).await;
        }
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
        next.map_or_else(
            || Err(TransportError::Invalid("script exhausted".into())),
            |scripted| scripted.result,
        )
    }
}

/// Hands out scripted tokens; records every `rejected` argument.
#[derive(Debug)]
pub struct FakeCredential {
    tokens: Mutex<VecDeque<AccessToken>>,
    current: Mutex<AccessToken>,
    rejected: Mutex<Vec<Option<TokenId>>>,
}

impl FakeCredential {
    /// The first token is current; later ones are handed out after each rejection.
    pub fn new(first: AccessToken, refreshed: impl IntoIterator<Item = AccessToken>) -> Self {
        Self {
            tokens: Mutex::new(refreshed.into_iter().collect()),
            current: Mutex::new(first),
            rejected: Mutex::new(Vec::new()),
        }
    }

    pub fn rejected(&self) -> Vec<Option<TokenId>> {
        lock(&self.rejected).clone()
    }
}

#[async_trait::async_trait]
impl Credential for FakeCredential {
    async fn access_token(&self, rejected: Option<TokenId>) -> Result<AccessToken, AuthError> {
        lock(&self.rejected).push(rejected);
        let mut current = lock(&self.current);
        if rejected == Some(current.id) {
            let next = lock(&self.tokens)
                .pop_front()
                .ok_or(AuthError::Unauthorized { code: None })?;
            *current = next;
        }
        Ok(current.clone())
    }
}

/// Which `AdoApi` method a fake failure or counter refers to.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ApiCall {
    ConnectionData,
    Projects,
    Repositories,
    ActivePullRequests,
    PullRequest,
    Threads,
    SearchSubjects,
    StorageKey,
    IdentitiesByQuery,
    IdentitiesByIds,
    ProbeOrgWide,
}

/// The contents of a fake org.
#[derive(Clone, Debug, Default)]
pub struct FakeOrg {
    pub me: Option<Person>,
    pub projects: Vec<Project>,
    pub repos: Vec<Repository>,
    /// All PRs; `active_pull_requests` returns the active ones matching the query.
    pub prs: Vec<PullRequest>,
    pub threads: HashMap<PrKey, Vec<ThreadDigest>>,
    pub subjects: Vec<PersonCandidate>,
    pub storage_keys: HashMap<String, Vsid>,
    pub identities: Vec<Person>,
    pub org_wide: Option<OrgWideProbe>,
    pub rate: RateSnapshot,
}

/// An in-memory org with per-call counters, injectable failures, delays and panics.
#[derive(Debug)]
pub struct FakeAdoApi {
    endpoint: OrgEndpoint,
    org: Mutex<FakeOrg>,
    calls: Mutex<HashMap<ApiCall, u32>>,
    failures: Mutex<HashMap<ApiCall, AdoError>>,
    project_failures: Mutex<HashMap<ProjectId, AdoError>>,
    delay: Mutex<Option<Duration>>,
    panic_on: Mutex<Option<ApiCall>>,
}

impl FakeAdoApi {
    pub fn new(org: &OrgId, contents: FakeOrg) -> Self {
        Self {
            endpoint: OrgEndpoint::new(org.clone()),
            org: Mutex::new(contents),
            calls: Mutex::new(HashMap::new()),
            failures: Mutex::new(HashMap::new()),
            project_failures: Mutex::new(HashMap::new()),
            delay: Mutex::new(None),
            panic_on: Mutex::new(None),
        }
    }

    /// Edits the org's contents in place.
    pub fn update(&self, edit: impl FnOnce(&mut FakeOrg)) {
        edit(&mut lock(&self.org));
    }

    /// Fails every later call of this kind until cleared.
    pub fn fail(&self, call: ApiCall, error: AdoError) {
        lock(&self.failures).insert(call, error);
    }

    pub fn fail_project(&self, project: ProjectId, error: AdoError) {
        lock(&self.project_failures).insert(project, error);
    }

    pub fn clear_failures(&self) {
        lock(&self.failures).clear();
        lock(&self.project_failures).clear();
    }

    pub fn set_delay(&self, delay: Option<Duration>) {
        *lock(&self.delay) = delay;
    }

    pub fn panic_on(&self, call: Option<ApiCall>) {
        *lock(&self.panic_on) = call;
    }

    pub fn calls(&self, call: ApiCall) -> u32 {
        lock(&self.calls).get(&call).copied().unwrap_or(0)
    }

    /// Counts the call, then applies delay, panic and failure injection.
    async fn enter(&self, call: ApiCall) -> Result<(), AdoError> {
        *lock(&self.calls).entry(call).or_default() += 1;
        let delay = *lock(&self.delay);
        if let Some(delay) = delay {
            tokio::time::sleep(delay).await;
        }
        assert!(
            *lock(&self.panic_on) != Some(call),
            "scripted panic in {call:?}"
        );
        lock(&self.failures).get(&call).cloned().map_or(Ok(()), Err)
    }

    fn snapshot(&self) -> FakeOrg {
        lock(&self.org).clone()
    }
}

fn matches_query(pr: &PullRequest, query: &PrQuery) -> bool {
    pr.state == PrState::Active && query.covers(pr.project, pr.key.repo)
}

#[async_trait::async_trait]
impl AdoApi for FakeAdoApi {
    fn endpoint(&self) -> &OrgEndpoint {
        &self.endpoint
    }

    async fn connection_data(&self) -> Result<Person, AdoError> {
        self.enter(ApiCall::ConnectionData).await?;
        self.snapshot()
            .me
            .ok_or(AdoError::Auth(AuthError::Unauthorized { code: None }))
    }

    async fn projects(&self) -> Result<Vec<Project>, AdoError> {
        self.enter(ApiCall::Projects).await?;
        Ok(self.snapshot().projects)
    }

    async fn repositories(&self) -> Result<Vec<Repository>, AdoError> {
        self.enter(ApiCall::Repositories).await?;
        Ok(self.snapshot().repos)
    }

    async fn active_pull_requests(&self, query: &PrQuery) -> Result<Vec<PullRequest>, AdoError> {
        self.enter(ApiCall::ActivePullRequests).await?;
        if let PrQuery::Project { project } | PrQuery::Repo { project, .. } = query
            && let Some(error) = lock(&self.project_failures).get(project).cloned()
        {
            return Err(error);
        }
        let prs = self.snapshot().prs;
        Ok(prs
            .into_iter()
            .filter(|pr| matches_query(pr, query))
            .collect())
    }

    async fn pull_request(
        &self,
        project: ProjectId,
        repo: RepoId,
        id: PrNumber,
    ) -> Result<PullRequest, AdoError> {
        self.enter(ApiCall::PullRequest).await?;
        self.snapshot()
            .prs
            .into_iter()
            .find(|pr| pr.project == project && pr.key.repo == repo && pr.key.number == id)
            .ok_or(AdoError::NotFound { message: None })
    }

    async fn threads(
        &self,
        _project: ProjectId,
        repo: RepoId,
        id: PrNumber,
    ) -> Result<Vec<ThreadDigest>, AdoError> {
        self.enter(ApiCall::Threads).await?;
        let key = PrKey::new(self.endpoint.org().clone(), repo, id);
        Ok(self
            .snapshot()
            .threads
            .get(&key)
            .cloned()
            .unwrap_or_default())
    }

    async fn search_subjects(&self, query: &str) -> Result<Vec<PersonCandidate>, AdoError> {
        self.enter(ApiCall::SearchSubjects).await?;
        let q = query.to_lowercase();
        let subjects = self.snapshot().subjects;
        Ok(subjects
            .into_iter()
            .filter(|s| s.name.to_lowercase().contains(&q))
            .collect())
    }

    async fn storage_key(&self, descriptor: &str) -> Result<Vsid, AdoError> {
        self.enter(ApiCall::StorageKey).await?;
        let keys = self.snapshot().storage_keys;
        keys.get(descriptor)
            .copied()
            .ok_or(AdoError::NotFound { message: None })
    }

    async fn identities_by_query(&self, query: &str) -> Result<Vec<Person>, AdoError> {
        self.enter(ApiCall::IdentitiesByQuery).await?;
        let q = query.to_lowercase();
        let people = self.snapshot().identities;
        Ok(people
            .into_iter()
            .filter(|p| {
                p.unique_name
                    .as_deref()
                    .is_some_and(|u| u.to_lowercase() == q)
            })
            .collect())
    }

    async fn identities_by_ids(&self, ids: &[Vsid]) -> Result<Vec<Person>, AdoError> {
        self.enter(ApiCall::IdentitiesByIds).await?;
        let people = self.snapshot().identities;
        Ok(people.into_iter().filter(|p| ids.contains(&p.id)).collect())
    }

    async fn probe_org_wide(&self, _project_count: usize) -> Result<OrgWideProbe, AdoError> {
        self.enter(ApiCall::ProbeOrgWide).await?;
        Ok(self
            .snapshot()
            .org_wide
            .unwrap_or(OrgWideProbe::Unsupported))
    }

    fn rate(&self) -> RateSnapshot {
        self.snapshot().rate
    }

    fn request_count(&self) -> u64 {
        lock(&self.calls).values().map(|n| u64::from(*n)).sum()
    }
}

/// Connects each org to a pre-built `FakeAdoApi`; unknown orgs get an empty one.
#[derive(Debug, Default)]
pub struct FakeConnector {
    apis: Mutex<HashMap<OrgId, Arc<FakeAdoApi>>>,
}

impl FakeConnector {
    pub fn insert(&self, api: Arc<FakeAdoApi>) {
        lock(&self.apis).insert(api.endpoint.org().clone(), api);
    }

    pub fn api(&self, org: &OrgId) -> Option<Arc<FakeAdoApi>> {
        lock(&self.apis).get(org).cloned()
    }
}

impl AdoConnector for FakeConnector {
    fn connect(
        &self,
        endpoint: OrgEndpoint,
        _credential: Arc<dyn Credential>,
        _policy: ClientPolicy,
    ) -> Arc<dyn AdoApi> {
        let mut apis = lock(&self.apis);
        let api = apis
            .entry(endpoint.org().clone())
            .or_insert_with(|| Arc::new(FakeAdoApi::new(endpoint.org(), FakeOrg::default())));
        api.clone()
    }
}

/// A header of the most recent request, for tests that inspect what was sent.
pub fn last_request_header(transport: &FakeTransport, name: &str) -> Option<String> {
    transport.requests().last().and_then(|r| {
        r.headers
            .get(name)
            .map(|v| v.to_str().unwrap_or_default().to_owned())
    })
}
