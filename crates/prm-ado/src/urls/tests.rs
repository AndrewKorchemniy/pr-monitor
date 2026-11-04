use prm_domain::testing::{org, project, repo};

use super::*;

fn endpoint() -> OrgEndpoint {
    OrgEndpoint::new(org("contoso"))
}

#[test]
fn url_pr_web_encodes_spaces() {
    let url = endpoint().pr_web("My Project", "web app", PrNumber(22));
    assert_eq!(
        url.as_str(),
        "https://dev.azure.com/contoso/My%20Project/_git/web%20app/pullrequest/22"
    );
}

#[test]
fn url_active_prs_literal_dollar_params() {
    let url = endpoint().active_prs(
        &PrQuery::Project {
            project: project(1),
        },
        500,
        500,
    );
    assert_eq!(
        url.as_str(),
        "https://dev.azure.com/contoso/00000000-0000-0000-0000-000000000001/_apis/git/pullrequests?searchCriteria.status=active&$top=500&$skip=500&api-version=7.1"
    );
}

#[test]
fn url_active_prs_repo_and_org_wide() {
    let repo_url = endpoint().active_prs(
        &PrQuery::Repo {
            project: project(1),
            repo: repo(2),
        },
        0,
        500,
    );
    assert!(repo_url.path().ends_with(
        "/00000000-0000-0000-0000-000000000001/_apis/git/repositories/00000000-0000-0000-0000-000000000002/pullrequests"
    ));
    let org_url = endpoint().active_prs(&PrQuery::OrgWide, 0, 50);
    assert_eq!(org_url.path(), "/contoso/_apis/git/pullrequests");
}

#[test]
fn url_storage_key_descriptor_segment() {
    let url = endpoint().storage_key("aad.AbC/d=");
    assert_eq!(url.host_str(), Some("vssps.dev.azure.com"));
    assert!(
        url.path()
            .ends_with("/_apis/graph/storagekeys/aad.AbC%2Fd=")
    );
}

#[test]
fn url_continuation_token_encoded() {
    let url = endpoint().projects(Some("a b&c"));
    assert!(url.query().unwrap().contains("continuationToken=a+b%26c"));
}

#[test]
fn url_identities_query_encoded() {
    let url = endpoint().identities_search("jane@contoso.com");
    assert!(
        url.query()
            .unwrap()
            .contains("filterValue=jane%40contoso.com")
    );
}

#[test]
fn avatar_url_accepts_only_the_orgs_own_hosts() {
    let accepted = [
        "https://dev.azure.com/contoso/_apis/GraphProfile/MemberAvatars/aad.AAA",
        "https://vssps.dev.azure.com/contoso/_apis/graph/Subjects/aad.AAA/avatars",
        "https://contoso.visualstudio.com/_apis/GraphProfile/MemberAvatars/aad.AAA",
        "https://dev.azure.com/Contoso/_apis/GraphProfile/MemberAvatars/aad.AAA",
    ];
    for href in accepted {
        assert!(endpoint().avatar_url(href).is_some(), "{href}");
    }
    let rejected = [
        "",
        "   ",
        "/_apis/GraphProfile/MemberAvatars/aad.AAA",
        "javascript:alert(1)",
        "http://dev.azure.com/contoso/_apis/x",
        "https://dev.azure.com/other/_apis/x",
        "https://dev.azure.com/contosoevil/_apis/x",
        "https://dev.azure.com.evil.test/contoso/_apis/x",
        "https://dev.azure.com@evil.test/contoso/_apis/x",
        "https://user:pw@dev.azure.com/contoso/_apis/x",
        "https://dev.azure.com:8443/contoso/_apis/x",
        "https://contoso.visualstudio.com:8443/_apis/x",
        "https://other.visualstudio.com/_apis/x",
        "https://evil.test/contoso/_apis/x",
    ];
    for href in rejected {
        assert!(endpoint().avatar_url(href).is_none(), "{href}");
    }
}

#[test]
fn avatar_url_follows_the_test_bases() {
    let base = |s: &str| Url::parse(s).unwrap();
    let endpoint = OrgEndpoint::with_bases(
        org("contoso"),
        base("http://127.0.0.1:9000/contoso/"),
        base("http://127.0.0.1:9001/contoso/"),
    );
    assert!(
        endpoint
            .avatar_url("http://127.0.0.1:9000/contoso/a")
            .is_some()
    );
    assert!(
        endpoint
            .avatar_url("http://127.0.0.1:9001/contoso/a")
            .is_some()
    );
    assert!(
        endpoint
            .avatar_url("http://127.0.0.1:9002/contoso/a")
            .is_none()
    );
}

#[test]
fn allows_avatar_agrees_with_avatar_url() {
    for href in [
        "https://dev.azure.com/contoso/_apis/GraphProfile/MemberAvatars/aad.AAA",
        "https://evil.test/a.png",
        "https://dev.azure.com@evil.test/contoso/a",
        "not a url",
    ] {
        assert_eq!(
            endpoint().allows_avatar(href),
            endpoint().avatar_url(href).is_some(),
            "{href}"
        );
    }
}
