//! One org's HTTP client: headers, concurrency cap, rate gate, retries, a single token refresh
//! on 401, and sign-in-page detection. `execute` is the only request loop in the crate.

use std::sync::{
    Arc, Mutex, PoisonError,
    atomic::{AtomicU64, Ordering},
};

use bytes::Bytes;
use http::{HeaderMap, HeaderValue, Method, header};
use prm_domain::auth::{AccessToken, AuthError, extract_code};
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::Semaphore;
use url::Url;

use crate::{
    consts::{
        H_ACTIVITY, H_E2EID, H_FEDAUTH, H_MSA_PASSTHROUGH, H_SESSION, MAX_CONCURRENCY, MAX_PAGES,
        PAGE_SIZE, V_FEDAUTH, V_MSA_PASSTHROUGH,
    },
    credential::{Credential, auth_header_value},
    dto::error::VssErrorDto,
    error::AdoError,
    http::{HttpRequest, HttpResponse, HttpTransport, TransportError},
    rate::{RateGate, RateSnapshot, parse_retry_after},
    retry::{RetryPolicy, backoff_delay},
    urls::OrgEndpoint,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientPolicy {
    pub max_concurrency: usize,
    pub retry: RetryPolicy,
    pub page_size: u32,
    pub max_pages: u32,
}

impl Default for ClientPolicy {
    fn default() -> Self {
        Self {
            max_concurrency: MAX_CONCURRENCY,
            retry: RetryPolicy::default(),
            page_size: PAGE_SIZE,
            max_pages: MAX_PAGES,
        }
    }
}

/// A decoded body plus the response headers (for continuation tokens).
pub(crate) struct Json<T> {
    pub(crate) body: T,
    pub(crate) headers: HeaderMap,
}

pub struct AdoClient {
    endpoint: OrgEndpoint,
    transport: Arc<dyn HttpTransport>,
    credential: Arc<dyn Credential>,
    semaphore: Semaphore,
    gate: RateGate,
    session: uuid::Uuid,
    policy: ClientPolicy,
    requests: AtomicU64,
    rng: Mutex<fastrand::Rng>,
}

/// What to do with one HTTP attempt's result.
#[derive(Debug)]
enum Outcome {
    Done(HttpResponse),
    Fail(AdoError),
    /// 401: ask the credential for a fresh token once, then retry.
    RefreshAuth,
    RetryWithBackoff,
    /// 429: wait in the rate gate. `no_retry_after`: the server gave no wait, so use the default.
    RetryAfterGate {
        no_retry_after: bool,
    },
}

impl AdoClient {
    pub fn new(
        endpoint: OrgEndpoint,
        transport: Arc<dyn HttpTransport>,
        credential: Arc<dyn Credential>,
        policy: ClientPolicy,
        session: uuid::Uuid,
    ) -> Self {
        Self {
            semaphore: Semaphore::new(policy.max_concurrency.max(1)),
            gate: RateGate::default(),
            requests: AtomicU64::new(0),
            rng: Mutex::new(fastrand::Rng::new()),
            endpoint,
            transport,
            credential,
            session,
            policy,
        }
    }

    pub fn endpoint(&self) -> &OrgEndpoint {
        &self.endpoint
    }

    pub fn policy(&self) -> &ClientPolicy {
        &self.policy
    }

    pub fn rate(&self) -> RateSnapshot {
        self.gate.snapshot()
    }

    pub fn request_count(&self) -> u64 {
        self.requests.load(Ordering::Relaxed)
    }

    pub(crate) async fn get_json<T: DeserializeOwned>(
        &self,
        url: Url,
    ) -> Result<Json<T>, AdoError> {
        let response = self.execute(Method::GET, &url, None).await?;
        decode(response, &url)
    }

    pub(crate) async fn post_json<B, T>(&self, url: Url, body: &B) -> Result<Json<T>, AdoError>
    where
        B: Serialize + ?Sized,
        T: DeserializeOwned,
    {
        let bytes = serde_json::to_vec(body).map_err(|e| AdoError::Decode(e.to_string().into()))?;
        let response = self
            .execute(Method::POST, &url, Some(Bytes::from(bytes)))
            .await?;
        decode(response, &url)
    }

    async fn execute(
        &self,
        method: Method,
        url: &Url,
        body: Option<Bytes>,
    ) -> Result<HttpResponse, AdoError> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| AdoError::Cancelled)?;
        let mut attempt = 0;
        let mut rejected = None;
        let mut refreshed = false;
        loop {
            self.gate.wait().await;
            let token = self.credential.access_token(rejected.take()).await?;
            let request = HttpRequest {
                method: method.clone(),
                url: url.clone(),
                headers: self.headers(&token, body.is_some())?,
                body: body.clone(),
                timeout: None,
            };
            self.requests.fetch_add(1, Ordering::Relaxed);
            let can_retry = attempt + 1 < self.policy.retry.max_attempts;
            let outcome = match self.transport.send(request).await {
                Ok(response) => {
                    self.gate.observe(&response.headers);
                    classify(&method, response, can_retry, refreshed)
                }
                Err(e) => transport_outcome(e, can_retry),
            };
            match outcome {
                Outcome::Done(response) => return Ok(response),
                Outcome::Fail(error) => return Err(error),
                Outcome::RefreshAuth => {
                    refreshed = true;
                    rejected = Some(token.id);
                }
                Outcome::RetryWithBackoff => {
                    tokio::time::sleep(self.backoff(attempt)).await;
                    attempt += 1;
                }
                Outcome::RetryAfterGate { no_retry_after } => {
                    self.hold_for_rate_limit(no_retry_after);
                    attempt += 1;
                }
            }
        }
    }

    fn headers(&self, token: &AccessToken, has_body: bool) -> Result<HeaderMap, AdoError> {
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(H_FEDAUTH, HeaderValue::from_static(V_FEDAUTH));
        headers.insert(
            H_MSA_PASSTHROUGH,
            HeaderValue::from_static(V_MSA_PASSTHROUGH),
        );
        if let Ok(session) = HeaderValue::from_str(&self.session.to_string()) {
            headers.insert(H_SESSION, session);
        }
        headers.insert(header::AUTHORIZATION, auth_header_value(&token.header)?);
        if has_body {
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
        }
        Ok(headers)
    }

    /// A 429 without `Retry-After` still waits, for the policy's default time.
    fn hold_for_rate_limit(&self, no_retry_after: bool) {
        if no_retry_after {
            self.gate.push(self.policy.retry.default_429_wait);
        }
    }

    fn backoff(&self, attempt: u32) -> std::time::Duration {
        let mut rng = self.rng.lock().unwrap_or_else(PoisonError::into_inner);
        backoff_delay(&self.policy.retry, attempt, &mut rng)
    }
}

fn transport_outcome(error: TransportError, can_retry: bool) -> Outcome {
    if can_retry {
        return Outcome::RetryWithBackoff;
    }
    match error {
        TransportError::Timeout => Outcome::Fail(AdoError::Timeout),
        TransportError::Connect(_)
        | TransportError::Tls(_)
        | TransportError::Proxy(_)
        | TransportError::Io(_)
        | TransportError::Invalid(_) => Outcome::Fail(AdoError::Network(error.to_string().into())),
    }
}

/// Maps one response to the next step. Pure apart from reading the response.
fn classify(method: &Method, response: HttpResponse, can_retry: bool, refreshed: bool) -> Outcome {
    let status = response.status.as_u16();
    match status {
        200..=299 => {
            let expects_body = *method != Method::HEAD && status != 204;
            if expects_body && !is_json(&response.headers) {
                // A 203 with an HTML sign-in page, or any non-JSON success: the token was not accepted.
                Outcome::Fail(AuthError::SignInPageReturned.into())
            } else {
                Outcome::Done(response)
            }
        }
        // Redirects are disabled; one means a sign-in redirect.
        300..=399 => Outcome::Fail(AuthError::SignInPageReturned.into()),
        401 if !refreshed => Outcome::RefreshAuth,
        401 => Outcome::Fail(
            AuthError::Unauthorized {
                code: vss_code(&response),
            }
            .into(),
        ),
        403 => Outcome::Fail(AdoError::Forbidden {
            message: vss_message(&response).unwrap_or_else(|| "Forbidden".into()),
            code: vss_code(&response),
        }),
        404 => Outcome::Fail(AdoError::NotFound {
            message: vss_message(&response),
        }),
        407 => Outcome::Fail(AdoError::ProxyAuthRequired),
        429 if can_retry => Outcome::RetryAfterGate {
            no_retry_after: parse_retry_after(&response.headers).is_none(),
        },
        429 => Outcome::Fail(AdoError::RateLimited {
            retry_after: parse_retry_after(&response.headers),
        }),
        500 | 502 | 503 | 504 if can_retry => Outcome::RetryWithBackoff,
        500..=599 => Outcome::Fail(AdoError::Server {
            status,
            message: vss_message(&response),
            activity: activity_id(&response.headers),
        }),
        _ => Outcome::Fail(AdoError::Http {
            status,
            message: vss_message(&response).unwrap_or_else(|| "Unexpected status".into()),
            code: vss_code(&response),
            activity: activity_id(&response.headers),
        }),
    }
}

fn is_json(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_ascii_lowercase)
        .is_some_and(|ct| ct.starts_with("application/json") || ct.contains("+json"))
}

fn vss_message(response: &HttpResponse) -> Option<Arc<str>> {
    serde_json::from_slice::<VssErrorDto>(&response.body)
        .ok()
        .map(|e| e.message)
        .filter(|m| !m.is_empty())
        .map(Arc::from)
}

fn vss_code(response: &HttpResponse) -> Option<Arc<str>> {
    vss_message(response).and_then(|m| extract_code(&m).map(Arc::from))
}

fn activity_id(headers: &HeaderMap) -> Option<Arc<str>> {
    [H_ACTIVITY, H_E2EID]
        .into_iter()
        .find_map(|name| headers.get(name)?.to_str().ok().map(Arc::from))
}

fn decode<T: DeserializeOwned>(response: HttpResponse, url: &Url) -> Result<Json<T>, AdoError> {
    let body = serde_json::from_slice(&response.body)
        .map_err(|e| AdoError::Decode(format!("{}: {e}", url.path()).into()))?;
    Ok(Json {
        body,
        headers: response.headers,
    })
}

#[cfg(test)]
mod tests;
