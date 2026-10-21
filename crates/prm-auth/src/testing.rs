//! Fakes for tests here and in prm-service (enabled by `test-support`).
#![allow(
    clippy::panic,
    reason = "test fixtures fail loudly on bad literals; the lint is silent under cfg(test), so expect would be unfulfilled there"
)]

use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use prm_ado::Credential;
use prm_domain::{
    Secret,
    auth::{AccessToken, AuthError, AuthMethod, SignInMethod},
    secret::TokenId,
};
use tokio_util::sync::CancellationToken;

use crate::{
    factory::AuthFactory,
    org::{AuthSnapshot, AuthSpec, OrgAuthenticator},
    process::{CommandRunner, ProcessOutput, ProcessSpec, RunError},
    source::{AuthContext, TokenSource},
};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A token source with scripted results (the last one repeats) and an optional delay.
#[derive(Debug)]
pub struct FakeTokenSource {
    method: AuthMethod,
    results: Mutex<VecDeque<Result<AccessToken, AuthError>>>,
    acquired: AtomicUsize,
    delay: Option<Duration>,
}

impl FakeTokenSource {
    pub fn new(
        method: AuthMethod,
        results: impl IntoIterator<Item = Result<AccessToken, AuthError>>,
    ) -> Self {
        Self {
            method,
            results: Mutex::new(results.into_iter().collect()),
            acquired: AtomicUsize::new(0),
            delay: None,
        }
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    pub fn acquired(&self) -> usize {
        self.acquired.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl TokenSource for FakeTokenSource {
    fn method(&self) -> AuthMethod {
        self.method
    }

    async fn acquire(&self, _ctx: &AuthContext) -> Result<AccessToken, AuthError> {
        self.acquired.fetch_add(1, Ordering::SeqCst);
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
        let mut results = lock(&self.results);
        let next = if results.len() > 1 {
            results.pop_front()
        } else {
            results.front().cloned()
        };
        next.unwrap_or(Err(AuthError::PatMissing))
    }
}

/// Runs nothing: answers by the first argument (`account`, `login`), records every spec.
#[derive(Debug, Default)]
pub struct FakeCommandRunner {
    answers: Mutex<Vec<(String, Result<ProcessOutput, RunError>)>>,
    specs: Mutex<Vec<ProcessSpec>>,
    delay: Option<Duration>,
}

impl FakeCommandRunner {
    pub fn answer(self, first_arg: &str, result: Result<ProcessOutput, RunError>) -> Self {
        lock(&self.answers).push((first_arg.to_owned(), result));
        self
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = Some(delay);
        self
    }

    pub fn specs(&self) -> Vec<ProcessSpec> {
        lock(&self.specs).clone()
    }
}

/// A successful process with this stdout.
#[expect(
    clippy::unnecessary_wraps,
    reason = "shaped to plug straight into FakeCommandRunner::answer"
)]
pub fn success(stdout: &str) -> Result<ProcessOutput, RunError> {
    Ok(ProcessOutput {
        status: Some(0),
        stdout: stdout.as_bytes().to_vec(),
        stderr: String::new(),
    })
}

/// A failed process with this stderr.
#[expect(
    clippy::unnecessary_wraps,
    reason = "shaped to plug straight into FakeCommandRunner::answer"
)]
pub fn failure(code: i32, stderr: &str) -> Result<ProcessOutput, RunError> {
    Ok(ProcessOutput {
        status: Some(code),
        stdout: Vec::new(),
        stderr: stderr.to_owned(),
    })
}

#[async_trait::async_trait]
impl CommandRunner for FakeCommandRunner {
    async fn run(
        &self,
        spec: ProcessSpec,
        cancel: CancellationToken,
    ) -> Result<ProcessOutput, RunError> {
        let first = spec
            .args
            .first()
            .map(|a| a.to_string_lossy().into_owned())
            .unwrap_or_default();
        lock(&self.specs).push(spec);
        if let Some(delay) = self.delay {
            tokio::select! {
                () = tokio::time::sleep(delay) => {}
                () = cancel.cancelled() => return Err(RunError::Cancelled),
            }
        }
        lock(&self.answers)
            .iter()
            .find(|(arg, _)| *arg == first)
            .map_or(Err(RunError::NotFound), |(_, result)| result.clone())
    }
}

/// An authenticator with scripted results. Tokens default to a bearer "fake".
#[derive(Debug)]
pub struct FakeOrgAuth {
    token: Mutex<Result<AccessToken, AuthError>>,
    sign_in: Mutex<Result<AuthMethod, AuthError>>,
    snapshot: Mutex<AuthSnapshot>,
    stored_pats: Mutex<Vec<String>>,
    sign_in_delay: Mutex<Option<Duration>>,
    sign_ins: AtomicUsize,
}

impl Default for FakeOrgAuth {
    fn default() -> Self {
        Self {
            token: Mutex::new(Ok(prm_ado::testing::token("fake"))),
            sign_in: Mutex::new(Ok(AuthMethod::AzureCli)),
            snapshot: Mutex::new(AuthSnapshot {
                method: Some(AuthMethod::AzureCli),
                ..AuthSnapshot::default()
            }),
            stored_pats: Mutex::new(Vec::new()),
            sign_in_delay: Mutex::new(None),
            sign_ins: AtomicUsize::new(0),
        }
    }
}

impl FakeOrgAuth {
    pub fn set_token(&self, result: Result<AccessToken, AuthError>) {
        *lock(&self.token) = result;
    }

    pub fn set_sign_in(&self, result: Result<AuthMethod, AuthError>) {
        *lock(&self.sign_in) = result;
    }

    pub fn with_sign_in_delay(self, delay: Duration) -> Self {
        self.set_sign_in_delay(Some(delay));
        self
    }

    /// Later sign-ins wait this long (or until cancelled) before answering.
    pub fn set_sign_in_delay(&self, delay: Option<Duration>) {
        *lock(&self.sign_in_delay) = delay;
    }

    /// How many times `sign_in` was called (any method).
    pub fn sign_ins(&self) -> usize {
        self.sign_ins.load(Ordering::SeqCst)
    }

    pub fn set_snapshot(&self, snapshot: AuthSnapshot) {
        *lock(&self.snapshot) = snapshot;
    }

    pub fn stored_pats(&self) -> Vec<String> {
        lock(&self.stored_pats).clone()
    }
}

#[async_trait::async_trait]
impl Credential for FakeOrgAuth {
    async fn access_token(&self, _rejected: Option<TokenId>) -> Result<AccessToken, AuthError> {
        lock(&self.token).clone()
    }
}

#[async_trait::async_trait]
impl OrgAuthenticator for FakeOrgAuth {
    async fn sign_in(
        &self,
        _method: SignInMethod,
        cancel: CancellationToken,
    ) -> Result<AuthMethod, AuthError> {
        self.sign_ins.fetch_add(1, Ordering::SeqCst);
        let delay = *lock(&self.sign_in_delay);
        if let Some(delay) = delay {
            tokio::select! {
                () = tokio::time::sleep(delay) => {}
                () = cancel.cancelled() => return Err(AuthError::Cancelled),
            }
        }
        lock(&self.sign_in).clone()
    }

    async fn store_pat(&self, pat: Secret) -> Result<(), AuthError> {
        lock(&self.stored_pats).push(pat.expose().to_owned());
        Ok(())
    }

    async fn sign_out(&self) -> Result<(), AuthError> {
        lock(&self.snapshot).signed_out = true;
        Ok(())
    }

    fn snapshot(&self) -> AuthSnapshot {
        *lock(&self.snapshot)
    }
}

/// Hands out one shared `FakeOrgAuth` per org (created on demand); records specs.
#[derive(Debug, Default)]
pub struct FakeAuthFactory {
    auths: Mutex<Vec<(prm_domain::OrgId, Arc<FakeOrgAuth>)>>,
    specs: Mutex<Vec<AuthSpec>>,
}

impl FakeAuthFactory {
    pub fn auth(&self, org: &prm_domain::OrgId) -> Arc<FakeOrgAuth> {
        let mut auths = lock(&self.auths);
        if let Some((_, auth)) = auths.iter().find(|(o, _)| o == org) {
            return auth.clone();
        }
        let auth = Arc::new(FakeOrgAuth::default());
        auths.push((org.clone(), auth.clone()));
        auth
    }

    pub fn specs(&self) -> Vec<AuthSpec> {
        lock(&self.specs).clone()
    }
}

impl AuthFactory for FakeAuthFactory {
    fn create(&self, spec: AuthSpec) -> Arc<dyn OrgAuthenticator> {
        let auth = self.auth(&spec.org);
        lock(&self.specs).push(spec);
        auth
    }
}
