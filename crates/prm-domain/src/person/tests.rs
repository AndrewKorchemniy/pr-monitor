use super::*;

fn p(n: u128, name: &str, unique: &str) -> Person {
    Person {
        id: Vsid::from_u128(n),
        name: name.into(),
        unique_name: Some(unique.into()),
        group: false,
        descriptor: None,
    }
}

fn t(s: i64) -> Timestamp {
    Timestamp::from_unix(s).unwrap()
}

#[test]
fn people_search_ranking() {
    let mut c = PeopleCache::default();
    c.harvest(
        [
            p(1, "Jane Doe", "jane@contoso.com"),
            p(2, "Benjamin Janeway", "bjaneway@contoso.com"),
            p(3, "Ann Bell", "ann@contoso.com"),
        ],
        t(10),
    );
    c.harvest([p(2, "Benjamin Janeway", "bjaneway@contoso.com")], t(11));
    let r = c.search("jane", 10);
    let ids: Vec<_> = r.iter().map(|x| x.id.unwrap()).collect();
    // Both are word-prefix matches ("Jane", "Janeway"); #2 has more hits.
    assert_eq!(ids, vec![Vsid::from_u128(2), Vsid::from_u128(1)]);
    let r = c.search("ell", 10);
    assert_eq!(r.len(), 1);
    assert_eq!(c.search("", 10), Vec::<PersonCandidate>::new());
}

#[test]
fn people_harvest_updates_names() {
    let mut c = PeopleCache::default();
    c.harvest([p(1, "Old", "old@x")], t(1));
    c.harvest([Person::new(Vsid::from_u128(1), "New")], t(2));
    assert_eq!(&*c.get(&Vsid::from_u128(1)).unwrap().name, "New");
    assert_eq!(
        c.get(&Vsid::from_u128(1)).unwrap().unique_name.as_deref(),
        Some("old@x")
    );
    c.harvest([Person::new(Vsid::from_u128(1), "  ")], t(3));
    assert_eq!(&*c.get(&Vsid::from_u128(1)).unwrap().name, "New");
}

#[test]
fn people_evict_lru() {
    let mut c = PeopleCache::default();
    for i in 1..=5u8 {
        c.harvest([p(u128::from(i), "x", "x")], t(i64::from(i)));
    }
    c.evict_to(2);
    assert_eq!(c.len(), 2);
    assert!(c.get(&Vsid::from_u128(5)).is_some());
    assert!(c.get(&Vsid::from_u128(4)).is_some());
    let persisted = c.to_persisted(10);
    assert_eq!(persisted[0].id, Vsid::from_u128(5));
}

#[test]
fn candidate_is_person_by_id_descriptor_or_unique_name() {
    let mut jane = p(1, "Jane", "jane@x");
    jane.descriptor = Some("aad.1".into());
    let directory = |descriptor: &str, unique: &str| PersonCandidate {
        id: None,
        descriptor: Some(descriptor.into()),
        name: "Jane D".into(),
        unique_name: Some(unique.into()),
        group: false,
        source: CandidateSource::Directory,
    };
    let cases = [
        (jane.to_candidate(CandidateSource::Configured), true),
        (directory("aad.1", "other@x"), true),
        (directory("aad.9", "JANE@X"), true),
        (directory("aad.9", "bob@x"), false),
        (
            p(2, "Jane", " ").to_candidate(CandidateSource::Harvested),
            false,
        ),
    ];
    for (candidate, expected) in cases {
        assert_eq!(candidate.is_person(&jane), expected, "{candidate:?}");
    }
}

#[test]
fn merge_candidates_dedupe() {
    let local = vec![p(1, "Jane", "jane@x").to_candidate(CandidateSource::Configured)];
    let remote = vec![
        PersonCandidate {
            id: None,
            descriptor: Some("aad.1".into()),
            name: "Jane D".into(),
            unique_name: Some("JANE@X".into()),
            group: false,
            source: CandidateSource::Directory,
        },
        PersonCandidate {
            id: None,
            descriptor: Some("aad.2".into()),
            name: "Bob".into(),
            unique_name: Some("bob@x".into()),
            group: false,
            source: CandidateSource::Directory,
        },
    ];
    let m = merge_candidates(local, remote, 10);
    assert_eq!(m.len(), 2);
    assert_eq!(m[0].source, CandidateSource::Configured);
    assert_eq!(&*m[1].name, "Bob");
    assert_eq!(merge_candidates(m.clone(), vec![], 1).len(), 1);
}
