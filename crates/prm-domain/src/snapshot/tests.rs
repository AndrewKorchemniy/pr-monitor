use super::*;
use crate::testing::{PrBuilder, ThreadBuilder, key, person, record, vsid};

fn classified(rec: &PrRecord) -> ClassifiedPr {
    let cfg = ClassifyConfig {
        reviewers: [vsid(2)].into_iter().collect(),
        ..ClassifyConfig::default()
    };
    classify_record(rec, &cfg, false)
}

fn base() -> PrBuilder {
    PrBuilder::new(key("contoso", 1, 1)).author(person(1, "A"))
}

#[test]
fn fingerprint_changes_on_new_comment_vote_and_push() {
    let plain = classified(&record(base().build(), vec![]));
    let commented = classified(&record(
        base().build(),
        vec![ThreadBuilder::human(1).comment(person(2, "R")).build()],
    ));
    let voted = classified(&record(base().reviewer(person(2, "R"), 10).build(), vec![]));
    let pushed = classified(&record(base().head("abc").build(), vec![]));

    assert_ne!(plain.fingerprint, commented.fingerprint);
    assert_ne!(plain.fingerprint, voted.fingerprint);
    assert_ne!(plain.fingerprint, pushed.fingerprint);
}

#[test]
fn fingerprint_same_input_same_value() {
    let first = classified(&record(base().build(), vec![]));
    let second = classified(&record(base().build(), vec![]));
    assert_eq!(first.fingerprint, second.fingerprint);
}

#[test]
fn fingerprint_stable_across_releases() {
    // Persisted in state.json: if this snapshot changes, every user's unread state resets.
    let rec = record(base().title("Fix the thing").build(), vec![]);
    insta::assert_snapshot!(classified(&rec).fingerprint.to_string());
}

#[test]
fn fingerprint_serializes_as_hex() {
    let fingerprint = Fingerprint(0x9f86_d081_884c_7d65);
    let json = serde_json::to_string(&fingerprint).unwrap();
    assert_eq!(json, "\"9f86d081884c7d65\"");
    assert_eq!(
        serde_json::from_str::<Fingerprint>(&json).unwrap(),
        fingerprint
    );
}

#[test]
fn classified_people_include_reviewers_commenters_and_author() {
    let rec = record(
        base().reviewer(person(2, "Rev"), 10).build(),
        vec![
            ThreadBuilder::human(1)
                .comment(person(3, "Commenter"))
                .build(),
        ],
    );
    let ids: Vec<_> = classified(&rec).people.into_keys().collect();
    assert_eq!(ids, vec![vsid(1), vsid(2), vsid(3)]);
}
