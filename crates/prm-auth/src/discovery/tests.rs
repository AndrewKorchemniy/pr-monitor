use http::HeaderValue;
use prm_ado::testing::{FakeTransport, Scripted};
use prm_domain::testing::org;

use super::*;

fn with(name: &'static str, value: &'static str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(name, HeaderValue::from_static(value));
    headers
}

#[test]
fn tenant_from_resource_tenant_header() {
    let tenant = tenant_from_headers(&with(
        "x-vss-resourcetenant",
        "00000000-0000-0000-0000-000000000000, 72f988bf-86f1-41af-91ab-2d7cd011db47",
    ))
    .unwrap();
    assert_eq!(
        tenant.id.to_string(),
        "72f988bf-86f1-41af-91ab-2d7cd011db47"
    );
    assert!(!tenant.msa);
}

#[test]
fn tenant_from_www_authenticate() {
    let tenant = tenant_from_headers(&with(
        "www-authenticate",
        "Bearer authorization_uri=https://login.microsoftonline.com/72f988bf-86f1-41af-91ab-2d7cd011db47, Basic realm=\"x\"",
    ))
    .unwrap();
    assert_eq!(
        tenant.id.to_string(),
        "72f988bf-86f1-41af-91ab-2d7cd011db47"
    );
}

#[test]
fn tenant_all_zero_is_msa() {
    let tenant = tenant_from_headers(&with(
        "x-vss-resourcetenant",
        "00000000-0000-0000-0000-000000000000",
    ))
    .unwrap();
    assert!(tenant.msa);
}

#[tokio::test]
async fn tenant_404_org_not_found() {
    let transport = Arc::new(FakeTransport::new([Scripted::json(404, "")]));
    let result = TenantDiscovery::new(transport)
        .discover(&OrgEndpoint::new(org("nope")))
        .await;
    assert!(matches!(result, Err(AuthError::Other(_))));
}

#[tokio::test]
async fn discovery_sends_unauthenticated_head() {
    let transport = Arc::new(FakeTransport::new([Scripted::json(401, "").header(
        "x-vss-resourcetenant",
        "72f988bf-86f1-41af-91ab-2d7cd011db47",
    )]));
    let tenant = TenantDiscovery::new(transport.clone())
        .discover(&OrgEndpoint::new(org("contoso")))
        .await
        .unwrap();
    assert!(!tenant.msa);
    let request = &transport.requests()[0];
    assert_eq!(request.method, Method::HEAD);
    assert!(request.headers.get(http::header::AUTHORIZATION).is_none());
}
