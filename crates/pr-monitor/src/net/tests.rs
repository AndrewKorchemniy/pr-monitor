use std::sync::atomic::{AtomicUsize, Ordering};

use prm_ado::http::{HttpRequest, HttpResponse};

use super::*;

#[test]
fn proxy_plans_resolve_only_on_windows() {
    let manual = url::Url::parse("http://proxy:8080").expect("url");
    assert_eq!(
        proxy_plan(&ProxyConfig::None, true),
        ProxyPlan::Fixed(ProxySetting::None)
    );
    assert_eq!(
        proxy_plan(&ProxyConfig::Manual(manual.clone()), true),
        ProxyPlan::Fixed(ProxySetting::Manual(manual))
    );
    assert_eq!(
        proxy_plan(&ProxyConfig::System, true),
        ProxyPlan::Resolve(ProxyMode::System)
    );
    assert_eq!(
        proxy_plan(&ProxyConfig::Pac, true),
        ProxyPlan::Resolve(ProxyMode::PacOnly)
    );
    assert_eq!(
        proxy_plan(&ProxyConfig::Pac, false),
        ProxyPlan::Fixed(ProxySetting::System)
    );
}

#[test]
fn proxy_decisions_map_to_settings() {
    let url = url::Url::parse("http://p:1").expect("url");
    assert_eq!(
        decision_to_setting(Ok(ProxyDecision::Direct)),
        ProxySetting::None
    );
    assert_eq!(
        decision_to_setting(Ok(ProxyDecision::Proxy(url.clone()))),
        ProxySetting::Manual(url)
    );
    assert_eq!(
        decision_to_setting(Ok(ProxyDecision::SystemStatic)),
        ProxySetting::System
    );
    let failed = Err(PortError::new(
        prm_domain::ports::PortErrorKind::Failed,
        "x",
    ));
    assert_eq!(decision_to_setting(failed), ProxySetting::System);
}

#[test]
fn logged_proxies_never_show_credentials() {
    let url = url::Url::parse("http://user:secret@proxy:8080/").expect("url");
    assert_eq!(describe(&ProxySetting::Manual(url)), "http://proxy:8080/");
}

/// A transport that answers 204 and records which proxy it was built with.
struct Answer;

#[async_trait::async_trait]
impl HttpTransport for Answer {
    async fn send(&self, _request: HttpRequest) -> Result<HttpResponse, TransportError> {
        Ok(HttpResponse {
            status: http::StatusCode::NO_CONTENT,
            headers: http::HeaderMap::new(),
            body: bytes::Bytes::new(),
        })
    }
}

fn request() -> HttpRequest {
    HttpRequest {
        method: http::Method::GET,
        url: url::Url::parse("https://dev.azure.com/contoso").expect("url"),
        headers: http::HeaderMap::new(),
        body: None,
        timeout: None,
    }
}

fn recording_build(
    builds: &Arc<AtomicUsize>,
    seen: &Arc<Mutex<Option<ProxySetting>>>,
) -> Box<Build> {
    let (builds, seen) = (Arc::clone(builds), Arc::clone(seen));
    Box::new(move |setting| {
        builds.fetch_add(1, Ordering::Relaxed);
        *seen.lock().unwrap_or_else(PoisonError::into_inner) = Some(setting);
        Ok(Arc::new(Answer) as Arc<dyn HttpTransport>)
    })
}

#[tokio::test(start_paused = true)]
async fn the_transport_waits_for_the_resolved_proxy_and_builds_once() {
    let (builds, seen) = (Arc::new(AtomicUsize::new(0)), Arc::new(Mutex::new(None)));
    let (tx, rx) = oneshot::channel();
    let transport = DeferredTransport::new(None, Some(rx), recording_build(&builds, &seen));
    let manual = url::Url::parse("http://pac-proxy:3128").expect("url");
    tx.send(ProxySetting::Manual(manual.clone()))
        .expect("receiver alive");
    transport.send(request()).await.expect("sent");
    transport.send(request()).await.expect("sent");
    assert_eq!(builds.load(Ordering::Relaxed), 1);
    assert_eq!(
        *seen.lock().unwrap_or_else(PoisonError::into_inner),
        Some(ProxySetting::Manual(manual))
    );
}

#[tokio::test(start_paused = true)]
async fn a_slow_resolver_falls_back_to_the_system_proxy() {
    let (builds, seen) = (Arc::new(AtomicUsize::new(0)), Arc::new(Mutex::new(None)));
    let (_tx, rx) = oneshot::channel::<ProxySetting>();
    let transport = DeferredTransport::new(None, Some(rx), recording_build(&builds, &seen));
    transport
        .send(request())
        .await
        .expect("sent after the timeout");
    assert_eq!(
        *seen.lock().unwrap_or_else(PoisonError::into_inner),
        Some(ProxySetting::System)
    );
}
