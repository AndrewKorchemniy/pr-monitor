//! `auth = "auto"`: try each configured method, preferring the last one that worked.

use std::sync::{Arc, Mutex, PoisonError};

use prm_domain::auth::{AccessToken, AuthError, AuthMethod};

use crate::source::{AuthContext, TokenSource};

pub struct AutoChain {
    sources: Vec<Arc<dyn TokenSource>>,
    preferred: Mutex<Option<AuthMethod>>,
}

impl AutoChain {
    /// `sources` in fallback order (Azure CLI, Entra, PAT).
    pub fn new(sources: Vec<Arc<dyn TokenSource>>, preferred: Option<AuthMethod>) -> Self {
        Self {
            sources,
            preferred: Mutex::new(preferred),
        }
    }

    pub fn preferred(&self) -> Option<AuthMethod> {
        *self
            .preferred
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub fn set_preferred(&self, method: Option<AuthMethod>) {
        *self
            .preferred
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = method;
    }

    /// Preferred method first, then the rest. Stops early on a policy block or cancel.
    /// Success remembers the method; total failure returns the most actionable error.
    pub async fn acquire(&self, ctx: &AuthContext) -> Result<AccessToken, AuthError> {
        let mut errors = Vec::new();
        for source in self.ordered() {
            match source.acquire(ctx).await {
                Ok(token) => {
                    self.set_preferred(Some(source.method()));
                    return Ok(token);
                }
                Err(e @ (AuthError::ConditionalAccessBlocked(_) | AuthError::Cancelled)) => {
                    return Err(e);
                }
                Err(e) => errors.push((source.method(), e)),
            }
        }
        Err(most_actionable(errors))
    }

    fn ordered(&self) -> Vec<Arc<dyn TokenSource>> {
        let preferred = self.preferred();
        let (first, rest): (Vec<_>, Vec<_>) = self
            .sources
            .iter()
            .cloned()
            .partition(|s| Some(s.method()) == preferred);
        first.into_iter().chain(rest).collect()
    }
}

/// The error the user can most usefully act on. If no method is set up at all, `NoMethod`.
pub fn most_actionable(errors: Vec<(AuthMethod, AuthError)>) -> AuthError {
    let unconfigured = |e: &AuthError| {
        matches!(
            e,
            AuthError::PatMissing | AuthError::EntraNotConfigured | AuthError::AzNotInstalled
        )
    };
    if errors.iter().all(|(_, e)| unconfigured(e)) {
        let tried = errors
            .iter()
            .map(|(m, e)| (*m, e.to_string().into()))
            .collect::<Vec<_>>();
        return AuthError::NoMethod {
            tried: tried.into(),
        };
    }
    errors
        .into_iter()
        .map(|(_, e)| e)
        .min_by_key(AuthError::rank)
        .unwrap_or(AuthError::NoMethod {
            tried: Arc::new([]),
        })
}

#[cfg(test)]
mod tests;
