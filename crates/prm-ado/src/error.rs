//! Errors from Azure DevOps calls. Auth problems keep their `AuthError` so the UI can offer the
//! right sign-in action.

use std::{sync::Arc, time::Duration};

use prm_domain::auth::AuthError;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AdoError {
    #[error(transparent)]
    Auth(#[from] AuthError),
    #[error("Access denied: {message}")]
    Forbidden {
        message: Arc<str>,
        code: Option<Arc<str>>,
    },
    #[error("Not found.")]
    NotFound { message: Option<Arc<str>> },
    #[error("Azure DevOps is throttling requests.")]
    RateLimited { retry_after: Option<Duration> },
    #[error("Azure DevOps had a server error ({status}).")]
    Server {
        status: u16,
        message: Option<Arc<str>>,
        activity: Option<Arc<str>>,
    },
    #[error("Azure DevOps returned HTTP {status}: {message}")]
    Http {
        status: u16,
        message: Arc<str>,
        code: Option<Arc<str>>,
        activity: Option<Arc<str>>,
    },
    /// HTTP 407: the proxy wants NTLM/Kerberos sign-in, which reqwest can't do.
    #[error("Your proxy requires a sign-in PR Monitor can't do yet.")]
    ProxyAuthRequired,
    #[error("Can't reach Azure DevOps.")]
    Network(Arc<str>),
    #[error("Azure DevOps didn't respond in time.")]
    Timeout,
    #[error("Unexpected response from Azure DevOps.")]
    Decode(Arc<str>),
    #[error("Cancelled.")]
    Cancelled,
}

impl AdoError {
    pub fn is_auth(&self) -> bool {
        matches!(self, Self::Auth(_))
    }

    /// Worth retrying later without user action.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::Network(_) | Self::Timeout | Self::Server { .. } | Self::RateLimited { .. }
        )
    }

    /// A `TF…`/`VS…`/`AADSTS…` code when the server sent one.
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Auth(auth) => auth.code(),
            Self::Forbidden { code, .. } | Self::Http { code, .. } => code.as_deref(),
            Self::NotFound { .. }
            | Self::RateLimited { .. }
            | Self::Server { .. }
            | Self::ProxyAuthRequired
            | Self::Network(_)
            | Self::Timeout
            | Self::Decode(_)
            | Self::Cancelled => None,
        }
    }
}
