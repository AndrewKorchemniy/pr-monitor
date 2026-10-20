use super::{
    common::{ListDto, ListOrVec},
    identity::GraphSubjectDto,
    pr::{MergeStatusDto, PrStatusDto, PullRequestDto},
    thread::ThreadDto,
};
use crate::fixture;

fn prs() -> Vec<PullRequestDto> {
    serde_json::from_str::<ListDto<PullRequestDto>>(fixture!("prs_project_page1.json"))
        .unwrap()
        .value
}

#[test]
fn dto_unknown_enums_deserialize() {
    let pr: PullRequestDto = serde_json::from_str(fixture!("unknown_enums.json")).unwrap();
    assert_eq!(pr.status, PrStatusDto::Unknown);
    assert_eq!(pr.merge_status, Some(MergeStatusDto::Unknown));
}

#[test]
fn dto_vote_15() {
    let pr: PullRequestDto = serde_json::from_str(fixture!("unknown_enums.json")).unwrap();
    assert_eq!(pr.reviewers[0].vote, 15);
}

#[test]
fn dto_dates_seven_digits() {
    let first = &prs()[0];
    assert_eq!(
        first.creation_date.map(prm_domain::Timestamp::unix),
        Some(1_478_017_831)
    );
}

#[test]
fn dto_zero_date_none() {
    assert_eq!(prs()[2].creation_date, None);
}

#[test]
fn dto_unhyphenated_guid() {
    let json = r#"{"id": 1, "comments": [{"id": 1, "author": {"id": "2428198325304A9CAEB788D60D57ACFD"}}]}"#;
    let thread: ThreadDto = serde_json::from_str(json).unwrap();
    let author = thread.comments[0].author.as_ref().unwrap();
    assert_eq!(
        author.id.to_string(),
        "24281983-2530-4a9c-aeb7-88d60d57acfd"
    );
}

#[test]
fn dto_thread_properties_null() {
    let threads = serde_json::from_str::<ListDto<ThreadDto>>(fixture!("threads_mixed.json"))
        .unwrap()
        .value;
    let null_props = threads.iter().find(|t| t.id == 156).unwrap();
    assert!(null_props.properties.is_none());
}

#[test]
fn dto_subjectquery_wrapped_and_bare() {
    let wrapped: ListOrVec<GraphSubjectDto> =
        serde_json::from_str(fixture!("subjectquery_wrapped.json")).unwrap();
    let bare: ListOrVec<GraphSubjectDto> =
        serde_json::from_str(fixture!("subjectquery_bare.json")).unwrap();
    assert_eq!(wrapped.into_vec().len(), 2);
    assert_eq!(bare.into_vec().len(), 1);
}
