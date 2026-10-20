//! A token source: one way to get an Azure DevOps token without user interaction.

use prm_ado::TransportError;
use prm_domain::{
    OrgId,
    auth::{AccessToken, AuthError, AuthMethod, TenantInfo},
};

/// What a source needs to know about the org.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthContext {
    pub org: OrgId,
    /// `None` until discovered; `msa` orgs reject Entra tokens.
    pub tenant: Option<TenantInfo>,
}

#[async_trait::async_trait]
pub trait TokenSource: Send + Sync + 'static {
    fn method(&self) -> AuthMethod;

    /// Never interactive. Fails fast (`PatMissing`, `EntraNotConfigured`) when not set up.
    async fn acquire(&self, ctx: &AuthContext) -> Result<AccessToken, AuthError>;
}

/// The one place an HTTP transport failure (discovery, Entra token calls) becomes an
/// `AuthError`.
pub(crate) fn network(error: TransportError) -> AuthError {
    AuthError::Network(error.to_string().into())
}
