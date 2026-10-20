//! One org's authentication: tenant discovery, the configured method (or the Auto chain), a
//! single-flight cache, interactive sign-in, PAT storage and sign-out.

use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicBool, Ordering},
};

use prm_ado::{Credential, OrgEndpoint};
use prm_domain::{
    OrgId, Secret, Timestamp,
    auth::{AccessToken, AuthError, AuthMethod, AuthMode, SignInMethod, TenantInfo},
    ports::Clock,
    secret::TokenId,
};
use tokio_util::sync::CancellationToken;

use crate::{
    azcli::AzureCliSource,
    cache::TokenCache,
    chain::AutoChain,
    consts::TENANT_TTL,
    discovery::TenantDiscovery,
    entra::{EntraConfig, EntraSource},
    pat::PatSource,
    source::{AuthContext, TokenSource},
};

/// Which methods are set up, for the sign-in screen.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct AuthAvailability {
    pub az_installed: bool,
    pub pat_stored: bool,
    pub entra_configured: bool,
    pub entra_signed_in: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub struct AuthSnapshot {
    /// The method that produced the last token.
    pub method: Option<AuthMethod>,
    pub tenant: Option<TenantInfo>,
    pub signed_out: bool,
    pub expires_at: Option<Timestamp>,
    pub availability: AuthAvailability,
}

/// A credential that can also be signed in and out. Upcasts to `Arc<dyn Credential>`.
#[async_trait::async_trait]
pub trait OrgAuthenticator: Credential {
    async fn sign_in(
        &self,
        method: SignInMethod,
        cancel: CancellationToken,
    ) -> Result<AuthMethod, AuthError>;
    /// Stores an already-validated PAT and prefers it.
    async fn store_pat(&self, pat: Secret) -> Result<(), AuthError>;
    /// Deletes stored secrets; no silent sign-in until the user signs in again.
    async fn sign_out(&self) -> Result<(), AuthError>;
    fn snapshot(&self) -> AuthSnapshot;
}

/// How to authenticate one org (from config and persisted state).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthSpec {
    pub org: OrgId,
    pub endpoint: OrgEndpoint,
    pub mode: AuthMode,
    pub tenant_override: Option<uuid::Uuid>,
    /// A previously discovered tenant and when.
    pub tenant_cached: Option<(TenantInfo, Timestamp)>,
    pub entra: Option<EntraConfig>,
    pub preferred: Option<AuthMethod>,
    pub signed_out: bool,
}

/// The sources an `OrgAuth` chooses between.
pub struct Sources {
    pub az: Arc<AzureCliSource>,
    pub pat: Arc<PatSource>,
    pub entra: Arc<EntraSource>,
}

pub struct OrgAuth {
    spec: AuthSpec,
    sources: Sources,
    chain: AutoChain,
    cache: TokenCache,
    discovery: TenantDiscovery,
    clock: Arc<dyn Clock>,
    tenant: Mutex<Option<TenantInfo>>,
    signed_out: AtomicBool,
    last: Mutex<Option<(AuthMethod, Option<Timestamp>)>>,
}

impl OrgAuth {
    pub fn new(
        spec: AuthSpec,
        sources: Sources,
        cache: TokenCache,
        discovery: TenantDiscovery,
        clock: Arc<dyn Clock>,
    ) -> Self {
        let chain_sources: Vec<Arc<dyn TokenSource>> = vec![
            sources.az.clone(),
            sources.entra.clone(),
            sources.pat.clone(),
        ];
        Self {
            chain: AutoChain::new(chain_sources, spec.preferred),
            signed_out: AtomicBool::new(spec.signed_out),
            tenant: Mutex::new(None),
            last: Mutex::new(None),
            spec,
            sources,
            cache,
            discovery,
            clock,
        }
    }

    /// The tenant: override, else a cached one under a week old, else discovered now.
    /// Discovery failure is not fatal: sources work without a tenant, just less precisely.
    async fn context(&self) -> AuthContext {
        let known = *self.tenant.lock().unwrap_or_else(PoisonError::into_inner);
        let tenant = match known.or_else(|| self.preset_tenant()) {
            Some(tenant) => Some(tenant),
            None => match self.discovery.discover(&self.spec.endpoint).await {
                Ok(tenant) => Some(tenant),
                Err(e) => {
                    tracing::warn!(org = %self.spec.org, error = %e, "tenant discovery failed");
                    None
                }
            },
        };
        *self.tenant.lock().unwrap_or_else(PoisonError::into_inner) = tenant;
        AuthContext {
            org: self.spec.org.clone(),
            tenant,
        }
    }

    fn preset_tenant(&self) -> Option<TenantInfo> {
        if let Some(id) = self.spec.tenant_override {
            return Some(TenantInfo::from_id(id));
        }
        let (tenant, at) = self.spec.tenant_cached?;
        (self.clock.now().since(at) < TENANT_TTL).then_some(tenant)
    }

    async fn acquire(&self, ctx: &AuthContext) -> Result<AccessToken, AuthError> {
        match self.spec.mode {
            AuthMode::Auto => self.chain.acquire(ctx).await,
            AuthMode::AzureCli => self.sources.az.acquire(ctx).await,
            AuthMode::Pat => self.sources.pat.acquire(ctx).await,
            AuthMode::Entra => self.sources.entra.acquire(ctx).await,
        }
    }

    fn remember(&self, token: &AccessToken) {
        *self.last.lock().unwrap_or_else(PoisonError::into_inner) =
            Some((token.method, token.expires_at));
    }

    /// Clears signed-out, prefers `method` and drops any cached token or failure.
    async fn signed_in_with(&self, method: AuthMethod) {
        self.signed_out.store(false, Ordering::SeqCst);
        self.chain.set_preferred(Some(method));
        self.cache.invalidate().await;
    }
}

#[async_trait::async_trait]
impl Credential for OrgAuth {
    async fn access_token(&self, rejected: Option<TokenId>) -> Result<AccessToken, AuthError> {
        if self.signed_out.load(Ordering::SeqCst) {
            return Err(AuthError::SignedOut);
        }
        let ctx = self.context().await;
        let token = self.cache.get(rejected, || self.acquire(&ctx)).await?;
        self.remember(&token);
        Ok(token)
    }
}

#[async_trait::async_trait]
impl OrgAuthenticator for OrgAuth {
    async fn sign_in(
        &self,
        method: SignInMethod,
        cancel: CancellationToken,
    ) -> Result<AuthMethod, AuthError> {
        match method {
            SignInMethod::Silent => {
                self.cache.invalidate().await;
            }
            SignInMethod::AzLogin { broker } => {
                let ctx = self.context().await;
                self.sources.az.login(&ctx, broker, cancel).await?;
                self.signed_in_with(AuthMethod::AzureCli).await;
            }
            SignInMethod::Entra => {
                let ctx = self.context().await;
                let token = self.sources.entra.sign_in_interactive(&ctx, cancel).await?;
                self.signed_in_with(AuthMethod::Entra).await;
                self.remember(&token);
                self.cache.prime(token).await;
                return Ok(AuthMethod::Entra);
            }
        }
        Ok(self.access_token(None).await?.method)
    }

    async fn store_pat(&self, pat: Secret) -> Result<(), AuthError> {
        self.sources.pat.store(&pat)?;
        self.signed_in_with(AuthMethod::Pat).await;
        Ok(())
    }

    async fn sign_out(&self) -> Result<(), AuthError> {
        self.signed_out.store(true, Ordering::SeqCst);
        self.cache.invalidate().await;
        *self.last.lock().unwrap_or_else(PoisonError::into_inner) = None;
        self.sources.pat.delete()?;
        self.sources.entra.forget()
    }

    fn snapshot(&self) -> AuthSnapshot {
        let last = *self.last.lock().unwrap_or_else(PoisonError::into_inner);
        AuthSnapshot {
            method: last.map(|(m, _)| m),
            tenant: *self.tenant.lock().unwrap_or_else(PoisonError::into_inner),
            signed_out: self.signed_out.load(Ordering::SeqCst),
            expires_at: last.and_then(|(_, e)| e),
            availability: AuthAvailability {
                az_installed: self.sources.az.installed(),
                pat_stored: self.sources.pat.is_stored(),
                entra_configured: self.sources.entra.is_configured(),
                entra_signed_in: self.sources.entra.has_refresh_token(),
            },
        }
    }
}

#[cfg(test)]
mod tests;
