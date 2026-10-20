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
