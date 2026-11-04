use prm_domain::{
    Person,
    testing::{PrBuilder, key, org, person, record, ts},
};

use super::*;

const DAY: i64 = 24 * 3600;

fn with_url(n: u128, url: Option<&str>) -> Person {
    Person {
        avatar: url.map(Arc::from),
        ..person(n, "P")
    }
}

fn want(n: u128, url: Option<&str>) -> Want {
    Want {
        key: AvatarKey::new(org("contoso"), Vsid::from_u128(n)),
        url: url.map(Arc::from),
    }
}

fn ids(wants: &[Want]) -> Vec<u128> {
    wants.iter().map(|w| w.key.id.0.as_u128()).collect()
}

#[test]
fn wanted_lists_authors_before_reviewers_once_each() {
    let first = PrBuilder::new(key("contoso", 1, 1))
        .author(with_url(1, None))
        .reviewer(with_url(5, Some("r5")), 10)
        .reviewer(with_url(2, Some("r2")), 10)
        .build();
    let second = PrBuilder::new(key("contoso", 1, 2))
        .author(with_url(2, None))
        .reviewer(with_url(1, Some("a1")), 0)
        .build();
    let records = [record(first, vec![]), record(second, vec![])];
    let found = wanted(&org("contoso"), &records);
    assert_eq!(ids(&found), [1, 2, 5]);
    assert_eq!(
        found[0].url.as_deref(),
        Some("a1"),
        "a later address fills a gap"
    );
    assert_eq!(found[1].url.as_deref(), Some("r2"));
    assert_eq!(found[2].key.org, org("contoso"));
}

#[test]
fn unseen_skips_people_whose_cache_file_was_looked_at() {
    let wants = [want(1, None), want(2, None)];
    let checked = HashSet::from([Vsid::from_u128(1)]);
    assert_eq!(unseen(&wants, &checked).count(), 1);
}

#[test]
fn to_fetch_puts_unseen_before_expired_and_honours_the_budget() {
    let now = ts(100 * DAY);
    let wants = [
        want(1, Some("u")),
        want(2, Some("u")),
        want(3, Some("u")),
        want(4, Some("u")),
        want(5, None),
    ];
    let slots = HashMap::from([
        (Vsid::from_u128(1), Slot::Loaded(ts(90 * DAY))),
        (Vsid::from_u128(2), Slot::Loaded(ts(99 * DAY))),
        (Vsid::from_u128(3), Slot::Absent(ts(100 * DAY - 10))),
    ]);
    let picked = |budget| -> Vec<u128> {
        to_fetch(&wants, &slots, now, budget, |_| true)
            .iter()
            .map(|(key, _)| key.id.0.as_u128())
            .collect()
    };
    assert_eq!(picked(10), [4, 1]);
    assert_eq!(picked(1), [4]);
    assert_eq!(picked(0), Vec::<u128>::new());
}

#[test]
fn slots_expire_at_their_own_ttl() {
    let loaded = Slot::Loaded(ts(0));
    assert!(loaded.fresh(ts(6 * DAY)));
    assert!(!loaded.fresh(ts(8 * DAY)));
    let absent = Slot::Absent(ts(0));
    assert!(absent.fresh(ts(DAY - 1)));
    assert!(!absent.fresh(ts(DAY + 1)));
}

#[test]
fn a_time_in_the_future_is_never_fresh() {
    assert!(!Slot::Loaded(ts(1_000)).fresh(ts(10)));
}

#[test]
fn to_fetch_skips_addresses_the_allowlist_refuses_before_budgeting() {
    let allowed = |url: &str| url.starts_with("https://dev.azure.com/");
    let own = "https://dev.azure.com/contoso/a";
    let cases: [(&str, Vec<Want>, usize, Vec<u128>); 4] = [
        (
            "foreign first, one slot",
            vec![want(1, Some("https://evil.test/a")), want(2, Some(own))],
            1,
            vec![2],
        ),
        (
            "userinfo trick and plain http",
            vec![
                want(1, Some("http://dev.azure.com/contoso/a")),
                want(2, Some("not a url")),
                want(3, Some(own)),
            ],
            1,
            vec![3],
        ),
        (
            "only foreign",
            vec![want(1, Some("https://evil.test/a"))],
            5,
            vec![],
        ),
        (
            "own ones keep their order",
            vec![want(1, Some(own)), want(2, Some("x")), want(3, Some(own))],
            5,
            vec![1, 3],
        ),
    ];
    for (name, wants, budget, expected) in cases {
        let picked: Vec<u128> = to_fetch(&wants, &HashMap::new(), ts(0), budget, allowed)
            .iter()
            .map(|(key, _)| key.id.0.as_u128())
            .collect();
        assert_eq!(picked, expected, "{name}");
    }
}
