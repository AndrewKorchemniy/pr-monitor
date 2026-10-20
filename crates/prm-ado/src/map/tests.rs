use prm_domain::testing::org;

use super::*;
use crate::{
    dto::{common::ListDto, identity::ConnectionDataDto},
    fixture,
};

fn endpoint() -> OrgEndpoint {
    OrgEndpoint::new(org("contoso"))
}

fn prs() -> Vec<PullRequest> {
    serde_json::from_str::<ListDto<PullRequestDto>>(fixture!("prs_project_page1.json"))
        .unwrap()
        .value
        .into_iter()
        .map(|d| pull_request(&endpoint(), d))
        .collect()
}

#[test]
fn map_pull_request_snapshot() {
    insta::assert_json_snapshot!(prs()[0]);
}

#[test]
fn map_branches_stripped() {
    let pr = &prs()[0];
    assert_eq!(&*pr.source_branch, "npaulk/my_work");
    assert_eq!(&*pr.target_branch, "main");
    assert_eq!(
        &*pr.web_url,
        "https://dev.azure.com/contoso/Payments/_git/web%20app/pullrequest/22"
    );
    assert!(pr.auto_complete);
}

#[test]
fn map_missing_creation_date_is_epoch() {
    assert_eq!(prs()[2].created_at, Timestamp::UNIX_EPOCH);
    assert_eq!(prs()[1].merge_status, MergeStatus::Conflicts);
}

#[test]
fn map_thread_digest_snapshot() {
    let threads: Vec<ThreadDigest> =
        serde_json::from_str::<ListDto<ThreadDto>>(fixture!("threads_mixed.json"))
            .unwrap()
            .value
            .into_iter()
            .map(thread)
            .collect();
    assert_eq!(threads[0].system_type.as_deref(), Some("VoteUpdate"));
    assert_eq!(threads[5].status, ThreadStatus::Missing);
    assert_eq!(threads[10].comments[0].kind, CommentKind::Unknown);
    insta::assert_json_snapshot!(threads);
}

#[test]
fn map_snippet_char_boundary() {
    let long = "é".repeat(400);
    let dto: ThreadDto = serde_json::from_value(serde_json::json!({
        "id": 1, "status": "active",
        "comments": [{ "id": 1, "commentType": "text", "content": long }]
    }))
    .unwrap();
    let snippet = thread(dto).comments[0].snippet.clone().unwrap();
    assert_eq!(snippet.chars().count(), 160);
}

#[test]
fn map_me_uses_account_property() {
    let data: ConnectionDataDto = serde_json::from_str(fixture!("connection_data.json")).unwrap();
    let me = ims_person(data.authenticated_user);
    assert_eq!(&*me.name, "Jia-hao Tseng");
    assert_eq!(me.unique_name.as_deref(), Some("jtseng@contoso.com"));
}
