//! The HTTP seam. Everything above it is testable with a fake transport; `ReqwestTransport` is
//! the production implementation (SChannel on Windows, rustls elsewhere).

use std::{sync::Arc, time::Duration};

use bytes::Bytes;
use http::{HeaderMap, Method, StatusCode};
use url::Url;

/// One request. The body may hold secrets (refresh tokens, auth codes), so `Debug` shows only
/// its length.
#[derive(Clone)]
pub struct HttpRequest {
    pub method: Method,
    pub url: Url,
    pub headers: HeaderMap,
    pub body: Option<Bytes>,
}

impl std::fmt::Debug for HttpRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("url", &self.url.as_str())
            .field("headers", &self.headers)
            .field("body_len", &self.body.as_ref().map(Bytes::len))
            .finish()
    }
}

/// One response. The body may hold tokens, so `Debug` shows only its length.
#[derive(Clone)]
pub struct HttpResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

impl std::fmt::Debug for HttpResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpResponse")
            .field("status", &self.status)
            .field("headers", &self.headers)
            .field("body_len", &self.body.len())
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TransportError {
    #[error("timed out")]
    Timeout,
    #[error("connect: {0}")]
    Connect(Arc<str>),
    #[error("tls: {0}")]
    Tls(Arc<str>),
    #[error("proxy: {0}")]
    Proxy(Arc<str>),
    #[error("io: {0}")]
    Io(Arc<str>),
    #[error("bad request: {0}")]
    Invalid(Arc<str>),
}

#[async_trait::async_trait]
pub trait HttpTransport: Send + Sync + 'static {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError>;
}

/// PAC is resolved by the platform layer into `Manual` before the transport is built.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum ProxySetting {
    #[default]
    System,
    None,
    Manual(Url),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportConfig {
    pub user_agent: String,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub proxy: ProxySetting,
}

/// Connections idle longer than this are dropped.
const POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Clone, Debug)]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    pub fn new(cfg: &TransportConfig) -> Result<Self, TransportError> {
        let builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(cfg.connect_timeout)
            .timeout(cfg.request_timeout)
            .pool_idle_timeout(POOL_IDLE_TIMEOUT)
            .user_agent(cfg.user_agent.clone())
            .gzip(true);
        let builder = match &cfg.proxy {
            ProxySetting::System => builder,
            ProxySetting::None => builder.no_proxy(),
            ProxySetting::Manual(url) => {
                let proxy = reqwest::Proxy::all(url.as_str())
                    .map_err(|e| TransportError::Proxy(e.to_string().into()))?;
                builder.proxy(proxy)
            }
        };
        let client = builder
            .build()
            .map_err(|e| TransportError::Invalid(e.to_string().into()))?;
        Ok(Self { client })
    }
}

#[async_trait::async_trait]
impl HttpTransport for ReqwestTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let mut builder = self
            .client
            .request(request.method, request.url.as_str())
            .headers(request.headers);
        if let Some(body) = request.body {
            builder = builder.body(body);
        }
        let response = builder.send().await.map_err(map_error)?;
        let status = response.status();
        let headers = response.headers().clone();
        let body = response.bytes().await.map_err(map_error)?;
        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    }
}

/// Never includes headers (they may carry tokens).
fn map_error(e: reqwest::Error) -> TransportError {
    if e.is_timeout() {
        TransportError::Timeout
    } else if e.is_connect() {
        TransportError::Connect(e.to_string().into())
    } else {
        TransportError::Io(e.to_string().into())
    }
}
