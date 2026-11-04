use std::sync::Arc;

use prm_domain::testing::org;

use super::*;
use crate::{
    client::ClientPolicy,
    fixture,
    testing::{FakeCredential, FakeTransport, Scripted, token},
};

fn client(script: Vec<Scripted>) -> AdoClient {
    let credential = Arc::new(FakeCredential::new(token("t"), []));
    AdoClient::new(
        OrgEndpoint::new(org("contoso")),
        Arc::new(FakeTransport::new(script)),
        credential,
        ClientPolicy::default(),
        uuid::Uuid::nil(),
    )
}

#[tokio::test]
async fn probe_org_wide_multi_project_supported() {
    let probe = client(vec![Scripted::json(
        200,
        fixture!("prs_org_wide_two_projects.json"),
    )])
    .probe_org_wide(12)
    .await;
    assert_eq!(probe, Ok(OrgWideProbe::Supported));
}

#[tokio::test]
async fn probe_org_wide_single_project_org_supported() {
    let probe = client(vec![Scripted::json(200, fixture!("prs_empty.json"))])
        .probe_org_wide(1)
        .await;
    assert_eq!(probe, Ok(OrgWideProbe::Supported));
}

#[tokio::test]
async fn probe_org_wide_html_unsupported() {
    let probe = client(vec![Scripted::json(404, "{}")])
        .probe_org_wide(12)
        .await;
    assert_eq!(probe, Ok(OrgWideProbe::Unsupported));
}

/// A transient failure during the probe used to be cached as "unsupported" for a day.
#[tokio::test(start_paused = true)]
async fn probe_org_wide_transient_errors_are_errors() {
    let cases = [
        (
            "503 after retries",
            vec![Scripted::json(503, "{}"); 3],
            AdoError::Server {
                status: 503,
                message: None,
                activity: None,
            },
        ),
        (
            "timeout",
            vec![Scripted::error(crate::http::TransportError::Timeout); 3],
            AdoError::Timeout,
        ),
        (
            "network",
            vec![Scripted::error(crate::http::TransportError::Connect("down".into())); 3],
            AdoError::Network("connect: down".into()),
        ),
    ];
    for (name, script, expected) in cases {
        let probe = client(script).probe_org_wide(12).await;
        assert_eq!(probe, Err(expected), "{name}");
    }
}

#[tokio::test]
async fn probe_org_wide_auth_error_is_error() {
    let probe = client(vec![Scripted::with_type(203, "<html>", "text/html")])
        .probe_org_wide(12)
        .await;
    assert!(matches!(probe, Err(AdoError::Auth(_))), "{probe:?}");
}

#[tokio::test]
async fn connection_data_maps_me() {
    let me = client(vec![Scripted::json(200, fixture!("connection_data.json"))])
        .connection_data()
        .await
        .unwrap();
    assert_eq!(me.id.to_string(), "00000000-0000-4000-8000-000000000009");
}

#[tokio::test]
async fn search_subjects_marks_groups() {
    let found = client(vec![Scripted::json(
        200,
        fixture!("subjectquery_wrapped.json"),
    )])
    .search_subjects("pay")
    .await
    .unwrap();
    assert_eq!(found.len(), 2);
    assert!(found[1].group);
    assert_eq!(found[0].unique_name.as_deref(), Some("jane@contoso.com"));
}

#[tokio::test]
async fn storage_key_resolves_vsid() {
    let vsid = client(vec![Scripted::json(200, fixture!("storagekey.json"))])
        .storage_key("aad.AAA")
        .await
        .unwrap();
    assert_eq!(vsid.to_string(), "00000000-0000-4000-8000-000000000011");
}

#[tokio::test]
async fn identities_by_query_maps_mail() {
    let people = client(vec![Scripted::json(
        200,
        fixture!("identities_search.json"),
    )])
    .identities_by_query("jane@contoso.com")
    .await
    .unwrap();
    assert_eq!(people[0].unique_name.as_deref(), Some("jane@contoso.com"));
    assert_eq!(&*people[0].name, "Jane Doe");
}

const AVATAR: &str = "https://dev.azure.com/contoso/_apis/GraphProfile/MemberAvatars/aad.AAA";

fn png(status: u16) -> Scripted {
    Scripted::with_type(status, "png-bytes", "image/png")
}

fn avatar_client(script: Vec<Scripted>) -> (AdoClient, Arc<FakeTransport>) {
    let transport = Arc::new(FakeTransport::new(script));
    let client = AdoClient::new(
        OrgEndpoint::new(org("contoso")),
        transport.clone(),
        Arc::new(FakeCredential::new(token("t"), [])),
        ClientPolicy::default(),
        uuid::Uuid::nil(),
    );
    (client, transport)
}

#[tokio::test]
async fn avatar_of_a_trusted_address_sends_the_token_and_accepts_images() {
    let (client, transport) = avatar_client(vec![png(200)]);
    let bytes = client.avatar(AVATAR).await.unwrap();
    assert_eq!(&bytes[..], b"png-bytes");
    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].url.as_str(), AVATAR);
    assert_eq!(
        requests[0].headers.get("accept").unwrap(),
        "image/png, image/*;q=0.8"
    );
    assert!(requests[0].headers.contains_key("authorization"));
}

#[tokio::test]
async fn avatar_of_a_foreign_address_makes_no_request() {
    let (client, transport) = avatar_client(vec![png(200)]);
    for href in [
        "https://evil.test/a.png",
        "https://dev.azure.com@evil.test/contoso/a",
    ] {
        assert_eq!(
            client.avatar(href).await,
            Err(AdoError::ForeignUrl),
            "{href}"
        );
    }
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn avatar_failures_are_classified() {
    let cases = [
        ("404", png(404), AdoError::NotFound { message: None }),
        (
            "html page",
            Scripted::with_type(200, "<html>", "text/html"),
            AdoError::Auth(prm_domain::auth::AuthError::SignInPageReturned),
        ),
    ];
    for (name, scripted, expected) in cases {
        let (client, _) = avatar_client(vec![scripted]);
        assert_eq!(client.avatar(AVATAR).await, Err(expected), "{name}");
    }
}

/// A redirect is never followed (the token would leave the org) and is not an auth error, so it
/// cannot switch avatars off for the org.
#[tokio::test]
async fn avatar_redirect_is_a_plain_failure() {
    let (client, transport) = avatar_client(vec![
        Scripted::json(302, "").header("location", "https://evil.test/x.png"),
    ]);
    let error = client.avatar(AVATAR).await.unwrap_err();
    assert!(matches!(error, AdoError::Http { .. }), "{error:?}");
    assert!(!error.is_auth());
    assert_eq!(transport.requests().len(), 1);
}
