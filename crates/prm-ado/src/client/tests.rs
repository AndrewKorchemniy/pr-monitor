use std::time::Duration;

use prm_domain::{
    plan::PrQuery,
    testing::{org, project},
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

use super::*;
use crate::{
    api::AdoApi,
    fixture,
    http::{ProxySetting, ReqwestTransport, TransportConfig},
    testing::{FakeCredential, FakeTransport, Scripted, token},
};

const JSON: &str = "application/json; charset=utf-8";

fn real_transport() -> Arc<dyn HttpTransport> {
    let config = TransportConfig {
        user_agent: "pr-monitor-tests".into(),
        connect_timeout: Duration::from_secs(5),
        request_timeout: Duration::from_secs(10),
        proxy: ProxySetting::None,
    };
    Arc::new(ReqwestTransport::new(&config).unwrap())
}

fn endpoint_for(base: &str) -> OrgEndpoint {
    let url = Url::parse(&format!("{base}/contoso/")).unwrap();
    OrgEndpoint::with_bases(org("contoso"), url.clone(), url)
}

fn client(
    transport: Arc<dyn HttpTransport>,
    credential: Arc<FakeCredential>,
    base: &str,
) -> AdoClient {
    AdoClient::new(
        endpoint_for(base),
        transport,
        credential,
        ClientPolicy::default(),
        uuid::Uuid::nil(),
    )
}

fn credential() -> Arc<FakeCredential> {
    Arc::new(FakeCredential::new(token("first"), [token("second")]))
}

async fn server_with(status: u16, body: &str, content_type: &str) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(status).set_body_raw(body.to_owned(), content_type))
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn client_sends_required_headers() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/contoso/_apis/git/repositories"))
        .and(header("x-tfs-fedauthredirect", "Suppress"))
        .and(header("x-vss-forcemsapassthrough", "true"))
        .and(header("authorization", "Bearer first"))
        .and(header("accept", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(fixture!("repositories.json"), JSON))
        .expect(1)
        .mount(&server)
        .await;
    let repos = client(real_transport(), credential(), &server.uri())
        .repositories()
        .await
        .unwrap();
    assert_eq!(repos.len(), 5);
}

#[tokio::test]
async fn client_203_html_sign_in_page() {
    let server = server_with(203, fixture!("signin_page.html"), "text/html").await;
    let result = client(real_transport(), credential(), &server.uri())
        .repositories()
        .await;
    assert_eq!(result, Err(AdoError::Auth(AuthError::SignInPageReturned)));
}

#[tokio::test]
async fn client_200_text_html_sign_in_page() {
    let server = server_with(
        200,
        fixture!("signin_page.html"),
        "text/html; charset=utf-8",
    )
    .await;
    let result = client(real_transport(), credential(), &server.uri())
        .repositories()
        .await;
    assert_eq!(result, Err(AdoError::Auth(AuthError::SignInPageReturned)));
}

#[tokio::test]
async fn client_302_not_followed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("location", "https://login.example/"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let result = client(real_transport(), credential(), &server.uri())
        .repositories()
        .await;
    assert_eq!(result, Err(AdoError::Auth(AuthError::SignInPageReturned)));
}

#[tokio::test]
async fn client_401_refreshes_once_then_ok() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(header("authorization", "Bearer first"))
        .respond_with(
            ResponseTemplate::new(401).set_body_raw(fixture!("vss_error_401_tf400813.json"), JSON),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(header("authorization", "Bearer second"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(fixture!("repositories.json"), JSON))
        .mount(&server)
        .await;
    let cred = credential();
    let repos = client(real_transport(), cred.clone(), &server.uri())
        .repositories()
        .await
        .unwrap();
    assert_eq!(repos.len(), 5);
    assert_eq!(cred.rejected(), vec![None, Some(token("first").id)]);
}

#[tokio::test]
async fn client_401_twice_unauthorized_with_tf_code() {
    let server = server_with(401, fixture!("vss_error_401_tf400813.json"), JSON).await;
    let result = client(real_transport(), credential(), &server.uri())
        .repositories()
        .await;
    assert_eq!(
        result,
        Err(AdoError::Auth(AuthError::Unauthorized {
            code: Some("TF400813".into())
        }))
    );
}

#[tokio::test]
async fn client_vss_error_message_parsed() {
    let server = server_with(403, fixture!("vss_error_403.json"), JSON).await;
    let result = client(real_transport(), credential(), &server.uri())
        .repositories()
        .await;
    let Err(AdoError::Forbidden { message, code }) = result else {
        panic!("expected Forbidden, got {result:?}");
    };
    assert!(message.starts_with("VS403463"));
    assert_eq!(code.as_deref(), Some("VS403463"));
}

#[tokio::test]
async fn client_407_proxy_auth_required() {
    let server = server_with(407, "", JSON).await;
    let result = client(real_transport(), credential(), &server.uri())
        .repositories()
        .await;
    assert_eq!(result, Err(AdoError::ProxyAuthRequired));
}

#[tokio::test]
async fn client_projects_continuation_loop() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wiremock::matchers::query_param(
            "continuationToken",
            "page2",
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(fixture!("projects_page2.json"), JSON),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw(fixture!("projects_page1.json"), JSON)
                .insert_header("x-ms-continuationtoken", "page2"),
        )
        .mount(&server)
        .await;
    let projects = client(real_transport(), credential(), &server.uri())
        .projects()
        .await
        .unwrap();
    let names: Vec<&str> = projects.iter().map(|p| &*p.name).collect();
    assert_eq!(names, vec!["Payments", "Platform"]);
}

#[tokio::test]
async fn client_prs_paging_empty_page_terminates() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(wiremock::matchers::query_param("$skip", "3"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(fixture!("prs_empty.json"), JSON))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(wiremock::matchers::query_param("$skip", "0"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(fixture!("prs_project_page1.json"), JSON),
        )
        .expect(1)
        .mount(&server)
        .await;
    let query = PrQuery::Project {
        project: project(1),
    };
    let prs = client(real_transport(), credential(), &server.uri())
        .active_pull_requests(&query)
        .await
        .unwrap();
    assert_eq!(prs.len(), 3);
}

fn fake_client(script: Vec<Scripted>) -> (Arc<FakeTransport>, AdoClient) {
    let transport = Arc::new(FakeTransport::new(script));
    let client = client(transport.clone(), credential(), "https://dev.example");
    (transport, client)
}

const REPOS: &str = r#"{"count":0,"value":[]}"#;

#[tokio::test(start_paused = true)]
async fn client_429_retry_after_honored() {
    let (transport, client) = fake_client(vec![
        Scripted::json(429, "{}").header("retry-after", "20"),
        Scripted::json(200, REPOS),
    ]);
    let start = tokio::time::Instant::now();
    client.repositories().await.unwrap();
    assert!(start.elapsed() >= Duration::from_secs(20));
    assert_eq!(transport.requests().len(), 2);
}

#[tokio::test(start_paused = true)]
async fn client_429_without_retry_after_waits_default() {
    let (_transport, client) =
        fake_client(vec![Scripted::json(429, "{}"), Scripted::json(200, REPOS)]);
    let start = tokio::time::Instant::now();
    client.repositories().await.unwrap();
    assert!(start.elapsed() >= RetryPolicy::default().default_429_wait);
}

#[tokio::test(start_paused = true)]
async fn client_retry_after_on_200_delays_next() {
    let (_transport, client) = fake_client(vec![
        Scripted::json(200, REPOS).header("retry-after", "15"),
        Scripted::json(200, REPOS),
    ]);
    client.repositories().await.unwrap();
    let start = tokio::time::Instant::now();
    client.repositories().await.unwrap();
    assert!(start.elapsed() >= Duration::from_secs(15));
}

#[tokio::test(start_paused = true)]
async fn client_5xx_backoff_then_error() {
    let (transport, client) = fake_client(vec![
        Scripted::json(503, "{}"),
        Scripted::json(503, "{}"),
        Scripted::json(503, "{}"),
    ]);
    let result = client.repositories().await;
    assert!(
        matches!(result, Err(AdoError::Server { status: 503, .. })),
        "{result:?}"
    );
    assert_eq!(transport.requests().len(), 3);
}

#[tokio::test(start_paused = true)]
async fn client_network_error_retried() {
    let (transport, client) = fake_client(vec![
        Scripted::error(TransportError::Connect("reset".into())),
        Scripted::json(200, REPOS),
    ]);
    client.repositories().await.unwrap();
    assert_eq!(transport.requests().len(), 2);
}

#[tokio::test(start_paused = true)]
async fn client_semaphore_caps_concurrency() {
    let script: Vec<Scripted> = (0..20)
        .map(|_| Scripted::json(200, REPOS).delayed(Duration::from_millis(100)))
        .collect();
    let (transport, client) = fake_client(script);
    let client = Arc::new(client);
    let calls = (0..20).map(|_| {
        let client = client.clone();
        tokio::spawn(async move { client.repositories().await })
    });
    for call in calls.collect::<Vec<_>>() {
        call.await.unwrap().unwrap();
    }
    assert!(transport.peak_in_flight() <= MAX_CONCURRENCY);
    assert_eq!(transport.requests().len(), 20);
}
