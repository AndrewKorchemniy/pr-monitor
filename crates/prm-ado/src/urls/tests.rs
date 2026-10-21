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
