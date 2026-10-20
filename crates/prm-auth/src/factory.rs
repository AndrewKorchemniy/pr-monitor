//! Builds an `OrgAuthenticator` per org from shared infrastructure.

use std::sync::Arc;

use prm_ado::HttpTransport;
use prm_domain::ports::{Clock, UrlOpener};

use crate::{
    azcli::{AzureCliSource, locate::AzLocator},
    cache::TokenCache,
    consts::{NEG_CACHE_TTL, TOKEN_SKEW},
    discovery::TenantDiscovery,
    entra::{EntraDeps, EntraSource},
    org::{AuthSpec, OrgAuth, OrgAuthenticator, Sources},
    pat::PatSource,
    process::CommandRunner,
    secrets::{SecretKey, SecretStore},
};

pub trait AuthFactory: Send + Sync + 'static {
    fn create(&self, spec: AuthSpec) -> Arc<dyn OrgAuthenticator>;
}

pub struct DefaultAuthFactory {
    runner: Arc<dyn CommandRunner>,
    transport: Arc<dyn HttpTransport>,
    /// Already wrapped in `ChunkedSecretStore`.
    secrets: Arc<dyn SecretStore>,
    opener: Arc<dyn UrlOpener>,
    clock: Arc<dyn Clock>,
}

impl DefaultAuthFactory {
    pub fn new(
        runner: Arc<dyn CommandRunner>,
        transport: Arc<dyn HttpTransport>,
        secrets: Arc<dyn SecretStore>,
        opener: Arc<dyn UrlOpener>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            runner,
            transport,
            secrets,
            opener,
            clock,
        }
    }
}

impl AuthFactory for DefaultAuthFactory {
    fn create(&self, spec: AuthSpec) -> Arc<dyn OrgAuthenticator> {
        let entra_deps = EntraDeps {
            transport: self.transport.clone(),
            secrets: self.secrets.clone(),
            opener: self.opener.clone(),
            clock: self.clock.clone(),
        };
        let sources = Sources {
            az: Arc::new(AzureCliSource::new(
                self.runner.clone(),
                AzLocator::default(),
                self.clock.clone(),
            )),
            pat: Arc::new(PatSource::new(
                self.secrets.clone(),
                SecretKey::pat(&spec.org),
            )),
            entra: Arc::new(EntraSource::new(
                spec.entra.clone(),
                entra_deps,
                SecretKey::entra_refresh(&spec.org),
            )),
        };
        let cache = TokenCache::new(self.clock.clone(), TOKEN_SKEW, NEG_CACHE_TTL);
        let discovery = TenantDiscovery::new(self.transport.clone());
        Arc::new(OrgAuth::new(
            spec,
            sources,
            cache,
            discovery,
            self.clock.clone(),
        ))
    }
}
