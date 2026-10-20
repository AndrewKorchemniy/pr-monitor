//! The HTTP transport, built on first use once the proxy is known. PAC/WPAD resolution can take
//! seconds, so it runs on a thread while the app starts; the first request waits for it (8 s at
//! most, then the system proxy).

use std::{
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use prm_ado::{
    HttpTransport, ProxySetting, ReqwestTransport, TransportConfig, TransportError,
    http::{HttpRequest, HttpResponse},
};
use prm_config::{schema::NetworkConfig, schema::ProxyConfig};
use prm_domain::ports::PortError;
use prm_platform::proxy::{ProxyDecision, ProxyMode, resolve_proxy};
use tokio::sync::{OnceCell, oneshot};

use crate::build_info::VERSION;

pub(crate) const PROXY_WAIT: Duration = Duration::from_secs(8);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProxyPlan {
    Fixed(ProxySetting),
    Resolve(ProxyMode),
}

/// Off-Windows there is no resolver: PAC falls back to the system settings.
pub(crate) fn proxy_plan(config: &ProxyConfig, windows: bool) -> ProxyPlan {
    match config {
        ProxyConfig::None => ProxyPlan::Fixed(ProxySetting::None),
        ProxyConfig::Manual(url) => ProxyPlan::Fixed(ProxySetting::Manual(url.clone())),
        ProxyConfig::System if windows => ProxyPlan::Resolve(ProxyMode::System),
        ProxyConfig::Pac if windows => ProxyPlan::Resolve(ProxyMode::PacOnly),
        ProxyConfig::System | ProxyConfig::Pac => ProxyPlan::Fixed(ProxySetting::System),
    }
}

pub(crate) fn decision_to_setting(decision: Result<ProxyDecision, PortError>) -> ProxySetting {
    match decision {
        Ok(ProxyDecision::Direct) => ProxySetting::None,
        Ok(ProxyDecision::Proxy(url)) => ProxySetting::Manual(url),
        Ok(ProxyDecision::SystemStatic) | Err(_) => ProxySetting::System,
    }
}

type Build = dyn Fn(ProxySetting) -> Result<Arc<dyn HttpTransport>, TransportError> + Send + Sync;

pub(crate) struct DeferredTransport {
    cell: OnceCell<Arc<dyn HttpTransport>>,
    pending: Mutex<Option<oneshot::Receiver<ProxySetting>>>,
    fixed: Option<ProxySetting>,
    build: Box<Build>,
}

impl DeferredTransport {
    pub(crate) fn new(
        fixed: Option<ProxySetting>,
        pending: Option<oneshot::Receiver<ProxySetting>>,
        build: Box<Build>,
    ) -> Self {
        Self {
            cell: OnceCell::new(),
            pending: Mutex::new(pending),
            fixed,
            build,
        }
    }

    async fn setting(&self) -> ProxySetting {
        if let Some(fixed) = &self.fixed {
            return fixed.clone();
        }
        let pending = self
            .pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        let Some(rx) = pending else {
            return ProxySetting::System;
        };
        if let Ok(Ok(setting)) = tokio::time::timeout(PROXY_WAIT, rx).await {
            return setting;
        }
        tracing::warn!("proxy resolution timed out; using the system proxy");
        ProxySetting::System
    }
}

#[async_trait::async_trait]
impl HttpTransport for DeferredTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let transport = self
            .cell
            .get_or_try_init(|| async { (self.build)(self.setting().await) })
            .await?;
        transport.send(request).await
    }
}

/// Logs the proxy without any credentials in its URL.
fn describe(setting: &ProxySetting) -> String {
    match setting {
        ProxySetting::System => "system".to_owned(),
        ProxySetting::None => "none".to_owned(),
        ProxySetting::Manual(url) => {
            let mut shown = url.clone();
            let _ = shown.set_username("");
            let _ = shown.set_password(None);
            shown.to_string()
        }
    }
}

pub(crate) fn deferred_transport(network: &NetworkConfig) -> Arc<dyn HttpTransport> {
    let connect = Duration::from_secs(u64::from(network.connect_timeout_secs));
    let request = Duration::from_secs(u64::from(network.request_timeout_secs));
    let build = Box::new(move |proxy: ProxySetting| {
        tracing::info!(proxy = describe(&proxy), "network ready");
        let config = TransportConfig {
            user_agent: prm_domain::app::user_agent(VERSION),
            connect_timeout: connect,
            request_timeout: request,
            proxy,
        };
        ReqwestTransport::new(&config).map(|t| Arc::new(t) as Arc<dyn HttpTransport>)
    });
    match proxy_plan(&network.proxy, cfg!(windows)) {
        ProxyPlan::Fixed(setting) => Arc::new(DeferredTransport::new(Some(setting), None, build)),
        ProxyPlan::Resolve(mode) => Arc::new(DeferredTransport::new(
            None,
            Some(spawn_resolver(mode)),
            build,
        )),
    }
}

fn spawn_resolver(mode: ProxyMode) -> oneshot::Receiver<ProxySetting> {
    let (tx, rx) = oneshot::channel();
    let spawned = std::thread::Builder::new()
        .name("proxy".into())
        .spawn(move || {
            let target = url::Url::parse("https://dev.azure.com/").map_err(|e| {
                PortError::new(prm_domain::ports::PortErrorKind::Failed, e.to_string())
            });
            let _ = tx.send(decision_to_setting(
                target.and_then(|t| resolve_proxy(&t, mode)),
            ));
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "couldn't start proxy resolution");
    }
    rx
}

#[cfg(test)]
mod tests;
