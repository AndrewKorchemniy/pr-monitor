//! Assembling the service from the platform adapters and the loaded files.

use std::{collections::HashMap, sync::Arc, time::Duration};

use prm_ado::{HttpAdoConnector, HttpTransport};
use prm_auth::{DefaultAuthFactory, SecretStore, TokioCommandRunner};
use prm_config::{AppPaths, Config, ConfigLoad, FileRepo, StateLoad};
use prm_domain::ports::UrlOpener;
use prm_domain::{OrgId, model::Catalog, ports::Clock};
use prm_service::ports::{Autostart, Notifier};
use prm_service::{EventSink, InitialData, ServiceDeps};

use crate::cli::Args;

/// Autostart (`--minimized`) waits at least this long before the first refresh, so login
/// isn't slowed down.
const MINIMIZED_DELAY_SECS: u32 = 10;

pub(crate) fn effective_startup_delay(minimized: bool, config_secs: u32) -> Option<Duration> {
    let secs = if minimized {
        config_secs.max(MINIMIZED_DELAY_SECS)
    } else {
        config_secs
    };
    (secs > 0).then(|| Duration::from_secs(u64::from(secs)))
}

/// Hidden at start unless something asked to show a PR.
pub(crate) fn start_hidden(args: &Args, config: &Config) -> bool {
    (args.minimized || config.ui.start_minimized) && args.activate.is_none()
}

/// The platform adapters the service uses (the secret store is wrapped separately).
pub(crate) struct Platform {
    pub notifier: Arc<dyn Notifier>,
    pub autostart: Arc<dyn Autostart>,
    pub opener: Arc<dyn UrlOpener>,
}

/// What the service runs on.
pub(crate) struct Ports {
    pub transport: Arc<dyn HttpTransport>,
    pub secrets: Arc<dyn SecretStore>,
    pub clock: Arc<dyn Clock>,
    pub events: Arc<dyn EventSink>,
}

pub(crate) fn deps(platform: Platform, ports: Ports, paths: &AppPaths) -> ServiceDeps {
    let auth = DefaultAuthFactory::new(
        Arc::new(TokioCommandRunner),
        Arc::clone(&ports.transport),
        ports.secrets,
        Arc::clone(&platform.opener),
        Arc::clone(&ports.clock),
    );
    ServiceDeps {
        ado: Arc::new(HttpAdoConnector::new(ports.transport)),
        auth: Arc::new(auth),
        notifier: platform.notifier,
        events: ports.events,
        clock: ports.clock,
        opener: platform.opener,
        autostart: platform.autostart,
        repo: Arc::new(FileRepo::new(paths.clone())),
    }
}

pub(crate) fn initial_data(
    config: ConfigLoad,
    state: StateLoad,
    catalogs: HashMap<OrgId, Catalog>,
    args: &Args,
) -> InitialData {
    let startup_delay =
        effective_startup_delay(args.minimized, config.config.refresh.startup_delay_secs);
    let mut config_issues = config.issues;
    config_issues.extend(state.issue);
    InitialData {
        config: config.config,
        config_issues,
        persisted: state.state,
        catalogs,
        startup_delay,
        rng_seed: None,
    }
}

#[cfg(test)]
mod tests;
