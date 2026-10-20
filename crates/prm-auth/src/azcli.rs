//! Tokens from the user's Azure CLI session, and "Sign in with Microsoft" via `az login`.
//! Using the user's own `az` keeps sign-in on Microsoft's first-party client: no app
//! registration and no admin consent.

pub mod locate;
pub mod output;
pub mod spec;
pub mod stderr;

use std::sync::Arc;

use prm_domain::{
    auth::{AccessToken, AuthError, AuthHeader, AuthMethod, AzBroker},
    ports::Clock,
};
use tokio_util::sync::CancellationToken;

use crate::{
    azcli::{
        locate::AzLocator,
        output::parse_token_output,
        spec::{login_spec, token_spec},
        stderr::{map_failure, map_run_error},
    },
    process::{CommandRunner, RunError},
    source::{AuthContext, TokenSource},
};

pub struct AzureCliSource {
    runner: Arc<dyn CommandRunner>,
    locator: AzLocator,
    clock: Arc<dyn Clock>,
}

impl AzureCliSource {
    pub fn new(runner: Arc<dyn CommandRunner>, locator: AzLocator, clock: Arc<dyn Clock>) -> Self {
        Self {
            runner,
            locator,
            clock,
        }
    }

    /// Runs `az login` for the org's tenant. Opens the browser (or Windows' account picker).
    pub async fn login(
        &self,
        ctx: &AuthContext,
        broker: AzBroker,
        cancel: CancellationToken,
    ) -> Result<(), AuthError> {
        let tenant = usable_tenant(ctx)?;
        let az = self.locator.resolve()?;
        let output = self
            .runner
            .run(login_spec(&az, tenant, broker), cancel)
            .await
            .map_err(|e| self.run_failed(&e))?;
        if output.status == Some(0) {
            Ok(())
        } else {
            Err(map_failure(&output, tenant))
        }
    }

    /// Whether `az` was found (resolves on first call).
    pub fn installed(&self) -> bool {
        self.locator.resolve().is_ok()
    }

    fn run_failed(&self, error: &RunError) -> AuthError {
        if *error == RunError::NotFound {
            self.locator.forget();
        }
        map_run_error(error)
    }
}

/// The tenant to pass to `az`; Microsoft-account orgs can't use Azure CLI tokens.
fn usable_tenant(ctx: &AuthContext) -> Result<Option<uuid::Uuid>, AuthError> {
    match ctx.tenant {
        Some(tenant) if tenant.msa => Err(AuthError::MsaOrgUnsupported),
        Some(tenant) => Ok(Some(tenant.id)),
        None => Ok(None),
    }
}

#[async_trait::async_trait]
impl TokenSource for AzureCliSource {
    fn method(&self) -> AuthMethod {
        AuthMethod::AzureCli
    }

    async fn acquire(&self, ctx: &AuthContext) -> Result<AccessToken, AuthError> {
        let tenant = usable_tenant(ctx)?;
        let az = self.locator.resolve()?;
        let output = self
            .runner
            .run(token_spec(&az, tenant), CancellationToken::new())
            .await
            .map_err(|e| self.run_failed(&e))?;
        if output.status != Some(0) {
            return Err(map_failure(&output, tenant));
        }
        let (secret, expires_at) = parse_token_output(&output.stdout, self.clock.now())?;
        Ok(AccessToken {
            id: secret.id(),
            header: AuthHeader::Bearer(secret),
            expires_at: Some(expires_at),
            method: AuthMethod::AzureCli,
        })
    }
}

#[cfg(test)]
mod tests;
