//! Background jobs: what they return to the actor, how they are tagged (so a panic can be
//! attributed), and the small one-shot jobs. Jobs never touch service state.

pub(crate) mod people;
pub(crate) mod refresh;

use std::sync::Arc;

use prm_ado::{AdoApi, AdoError};
use prm_auth::{OrgAuthenticator, pat::validate_pat_format};
use prm_domain::{
    OrgId, Person, Secret, Vsid,
    auth::{AuthError, AuthMethod},
    ids::RequestId,
    model::Catalog,
    notify::NotifierOp,
    person::PersonCandidate,
    ports::PortError,
};

use crate::{
    errors::UiError,
    ports::{AutostartState, Notifier, NotifierCapability},
};

pub(crate) use refresh::{RefreshFailure, RefreshOutput};

/// A finished job, as the actor receives it.
pub(crate) enum JobOutput {
    Refresh {
        org: OrgId,
        generation: u64,
        result: Result<Box<RefreshOutput>, RefreshFailure>,
    },
    Probe {
        org: OrgId,
        generation: u64,
        result: Result<AuthMethod, AuthError>,
    },
    SignIn {
        org: OrgId,
        result: Result<AuthMethod, AuthError>,
    },
    SignedOut {
        org: OrgId,
        result: Result<(), AuthError>,
    },
    SetPat {
        org: OrgId,
        result: Result<Person, AdoError>,
    },
    Catalog {
        org: OrgId,
        generation: u64,
        result: Result<Catalog, AdoError>,
    },
    Search {
        org: OrgId,
        request: RequestId,
        query: Arc<str>,
        result: Result<Vec<PersonCandidate>, AdoError>,
    },
    Resolve {
        org: OrgId,
        request: RequestId,
        candidate: PersonCandidate,
        result: Result<Vsid, AdoError>,
    },
    Saved {
        what: Saved,
        result: Result<(), UiError>,
    },
    /// State saved, then the cycle's toasts shown (in that order).
    Flushed {
        saved: Result<(), UiError>,
        notify_failures: usize,
    },
    Capability(NotifierCapability),
    /// `seq` orders concurrent reads and writes; only the newest result is applied.
    Autostart {
        seq: u64,
        state: Result<AutostartState, PortError>,
        set_error: Option<PortError>,
    },
    Opened(Result<(), PortError>),
    TestNotification(Result<(), PortError>),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Saved {
    State,
    Config,
    Catalog,
    OrgDataDeleted,
}

/// What a job was, kept by task id so a panic can be attributed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct JobTag {
    pub org: Option<OrgId>,
    pub kind: JobKind,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum JobKind {
    Refresh,
    Probe,
    SignIn,
    SignOut,
    SetPat,
    Catalog,
    Search,
    Resolve,
    Save,
    Flush,
    Port,
}

impl JobTag {
    pub(crate) fn org(org: &OrgId, kind: JobKind) -> Self {
        Self {
            org: Some(org.clone()),
            kind,
        }
    }

    pub(crate) const fn global(kind: JobKind) -> Self {
        Self { org: None, kind }
    }
}

/// Runs a blocking port call on the blocking pool. `None` if it panicked.
pub(crate) async fn blocking<T, F>(f: F) -> Option<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    match tokio::task::spawn_blocking(f).await {
        Ok(value) => Some(value),
        Err(error) => {
            tracing::error!(%error, "blocking task failed");
            None
        }
    }
}

/// Checks the PAT against the org with a one-off client, and stores it only if it works.
pub(crate) async fn set_pat(
    check: Arc<dyn AdoApi>,
    auth: Arc<dyn OrgAuthenticator>,
    pat: Secret,
) -> Result<Person, AdoError> {
    validate_pat_format(pat.expose())?;
    let me = check.connection_data().await.map_err(pat_error)?;
    auth.store_pat(pat).await?;
    Ok(me)
}

/// A rejected PAT is reported as invalid, not as a generic sign-in problem.
fn pat_error(error: AdoError) -> AdoError {
    let rejected = matches!(
        error,
        AdoError::Auth(AuthError::Unauthorized { .. } | AuthError::SignInPageReturned)
    );
    if rejected {
        return AdoError::Auth(AuthError::PatInvalid);
    }
    if matches!(error, AdoError::Forbidden { .. }) {
        return AdoError::Auth(AuthError::PatInsufficientScope);
    }
    error
}

/// Shows and retracts toasts in order; returns how many calls failed.
pub(crate) fn apply_notifier_ops(notifier: &dyn Notifier, ops: &[NotifierOp]) -> usize {
    let mut failures = 0;
    for op in ops {
        let result = match op {
            NotifierOp::Show(notification) => notifier.show(notification),
            NotifierOp::Retract { tag, group } => notifier.retract(tag, group),
        };
        if let Err(error) = result {
            tracing::warn!(error = %error.message, "notification call failed");
            failures += 1;
        }
    }
    failures
}

#[cfg(test)]
mod tests;
