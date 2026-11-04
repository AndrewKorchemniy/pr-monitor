//! Avatars: cache first, a bounded number of downloads after each refresh, and what a refusal
//! or a passing failure does.

use std::time::Duration;

use bytes::Bytes;
use prm_ado::{AdoError, testing::ApiCall};
use prm_config::{AvatarCache, CachedAvatar, OrgPersisted};
use prm_domain::{
    Person, Timestamp,
    avatar::AvatarKey,
    model::PullRequest,
    testing::{BASE_TIME_SECS, PrBuilder, key, org, record, ts},
};

use super::support::{CONTOSO, ann, bob, refreshed, setup};
use crate::{
    avatars::decode,
    command::Command,
    consts::AVATAR_FETCH_BUDGET,
    event::AppEvent,
    testing::{HarnessSetup, ServiceHarness},
};

const DAY: i64 = 24 * 3600;

fn href(person: &Person) -> String {
    format!(
        "https://dev.azure.com/contoso/_apis/GraphProfile/MemberAvatars/{}",
        person.id
    )
}

fn pictured(person: Person) -> Person {
    Person {
        avatar: Some(href(&person).into()),
        ..person
    }
}

fn pr_by(n: i32, author: Person) -> PullRequest {
    PrBuilder::new(key(CONTOSO, 1, n)).author(author).build()
}

/// A small PNG as a server would send it.
fn source_png() -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, 8, 8);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&[200; 8 * 8 * 4]).unwrap();
    writer.finish().unwrap();
    out
}

fn stored_png() -> Vec<u8> {
    decode::to_png(&decode::from_source(&source_png()).unwrap()).unwrap()
}

fn key_of(person: &Person) -> AvatarKey {
    AvatarKey::new(org(CONTOSO), person.id)
}

fn now() -> Timestamp {
    ts(BASE_TIME_SECS)
}

fn cache_entry(png: Vec<u8>, age_secs: i64) -> CachedAvatar {
    CachedAvatar {
        png,
        stored_at: ts(BASE_TIME_SECS - age_secs),
    }
}

/// One PR per person, each author pictured at an address the fake org serves.
fn served_setup(people: &[Person]) -> HarnessSetup {
    let prs = people
        .iter()
        .enumerate()
        .map(|(i, p)| pr_by(i32::try_from(i).unwrap() + 1, pictured(p.clone())))
        .collect();
    let mut start = setup(vec![(CONTOSO, prs)]);
    start.config.orgs[0].people.tracked_authors = people.to_vec();
    let served = people
        .iter()
        .map(|p| (href(p), Bytes::from(source_png())))
        .collect();
    start.orgs[0].1.avatars = served;
    start
}

fn loaded(h: &ServiceHarness) -> Vec<AvatarKey> {
    h.events()
        .into_iter()
        .filter_map(|e| {
            if let AppEvent::Avatars(list) = e {
                Some(list)
            } else {
                None
            }
        })
        .flat_map(|list| list.iter().map(|l| l.key.clone()).collect::<Vec<_>>())
        .collect()
}

fn requests(h: &ServiceHarness) -> Vec<String> {
    h.api(&org(CONTOSO)).avatar_requests()
}

#[tokio::test(start_paused = true)]
async fn a_new_face_is_downloaded_cached_and_announced() {
    let mut h = ServiceHarness::start(served_setup(&[ann()]));
    refreshed(&mut h, CONTOSO, 1).await;
    h.until("the face", |h| !loaded(h).is_empty()).await;

    assert_eq!(loaded(&h), vec![key_of(&ann())]);
    assert_eq!(requests(&h), vec![href(&ann())]);
    let stored = h.avatar_cache.get(&key_of(&ann())).unwrap();
    assert!(stored.png.starts_with(b"\x89PNG"));
    assert_eq!(stored.stored_at, now());
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn a_cached_face_needs_no_request() {
    let start = served_setup(&[ann()]);
    start
        .avatar_cache
        .write(&key_of(&ann()), &cache_entry(stored_png(), 60))
        .unwrap();
    let mut h = ServiceHarness::start(start);
    refreshed(&mut h, CONTOSO, 1).await;
    h.until("the face", |h| !loaded(h).is_empty()).await;
    h.settle().await;

    assert_eq!(loaded(&h), vec![key_of(&ann())]);
    assert_eq!(requests(&h), Vec::<String>::new());
    h.stop().await;
}

/// Cached faces are on screen before the first refresh, whatever the network does.
#[tokio::test(start_paused = true)]
async fn cached_faces_show_before_the_first_refresh() {
    let mut start = setup(vec![(CONTOSO, vec![])]);
    start.startup_delay = Duration::from_hours(1);
    let author = pictured(ann());
    start.persisted.orgs.insert(
        org(CONTOSO),
        OrgPersisted {
            records: [(key(CONTOSO, 1, 1), record(pr_by(1, author), vec![]))].into(),
            ..OrgPersisted::default()
        },
    );
    start
        .avatar_cache
        .write(&key_of(&ann()), &cache_entry(stored_png(), 60))
        .unwrap();
    let h = ServiceHarness::start(start);
    h.until("the face", |h| !loaded(h).is_empty()).await;

    assert_eq!(loaded(&h), vec![key_of(&ann())]);
    assert_eq!(requests(&h), Vec::<String>::new());
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn entries_expire_after_a_week_and_no_picture_after_a_day() {
    let cases = [
        ("6 days", stored_png(), 6 * DAY, 0),
        ("8 days", stored_png(), 8 * DAY, 1),
        ("marker 1 hour", Vec::new(), 3600, 0),
        ("marker 2 days", Vec::new(), 2 * DAY, 1),
    ];
    for (name, png, age, expected) in cases {
        let start = served_setup(&[ann()]);
        start
            .avatar_cache
            .write(&key_of(&ann()), &cache_entry(png, age))
            .unwrap();
        let mut h = ServiceHarness::start(start);
        refreshed(&mut h, CONTOSO, 1).await;
        h.until(name, |h| {
            h.api(&org(CONTOSO)).calls(ApiCall::Avatar) >= expected
        })
        .await;
        h.settle().await;

        assert_eq!(requests(&h).len(), expected as usize, "{name}");
        h.stop().await;
    }
}

#[tokio::test(start_paused = true)]
async fn downloads_are_bounded_per_refresh_and_finish_over_the_next() {
    let people: Vec<Person> = (0..45)
        .map(|n| Person::new(prm_domain::Vsid::from_u128(100 + n), format!("P{n}")))
        .collect();
    let mut h = ServiceHarness::start(served_setup(&people));
    refreshed(&mut h, CONTOSO, 1).await;
    h.until("first batch", |h| loaded(h).len() >= AVATAR_FETCH_BUDGET)
        .await;
    h.settle().await;
    assert_eq!(requests(&h).len(), AVATAR_FETCH_BUDGET);

    h.advance(Duration::from_secs(3)).await;
    h.send(Command::RefreshNow);
    refreshed(&mut h, CONTOSO, 2).await;
    h.until("the rest", |h| loaded(h).len() == 45).await;

    assert_eq!(requests(&h).len(), 45, "nobody is fetched twice");
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn a_missing_picture_is_remembered_and_does_not_switch_avatars_off() {
    let mut start = served_setup(&[ann(), bob()]);
    start.orgs[0].1.avatars.remove(&href(&ann()));
    let mut h = ServiceHarness::start(start);
    refreshed(&mut h, CONTOSO, 1).await;
    h.until("bob", |h| !loaded(h).is_empty()).await;

    assert_eq!(loaded(&h), vec![key_of(&bob())]);
    let marker = h.avatar_cache.get(&key_of(&ann())).unwrap();
    assert!(
        marker.png.is_empty(),
        "no picture is cached as an empty marker"
    );

    h.advance(Duration::from_secs(3)).await;
    h.send(Command::RefreshNow);
    refreshed(&mut h, CONTOSO, 2).await;
    h.settle().await;
    assert_eq!(requests(&h).len(), 2, "neither is asked for again");
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn a_refusal_switches_avatars_off_until_restart() {
    let mut h = ServiceHarness::start(served_setup(&[ann(), bob()]));
    h.api(&org(CONTOSO)).fail(
        ApiCall::Avatar,
        AdoError::Forbidden {
            message: "no".into(),
            code: None,
        },
    );
    refreshed(&mut h, CONTOSO, 1).await;
    h.until("the attempt", |h| {
        h.api(&org(CONTOSO)).calls(ApiCall::Avatar) >= 1
    })
    .await;
    h.settle().await;
    let attempts = h.api(&org(CONTOSO)).calls(ApiCall::Avatar);
    h.api(&org(CONTOSO)).clear_failures();

    h.advance(Duration::from_secs(3)).await;
    h.send(Command::RefreshNow);
    refreshed(&mut h, CONTOSO, 2).await;
    h.settle().await;

    assert_eq!(h.api(&org(CONTOSO)).calls(ApiCall::Avatar), attempts);
    assert!(h.avatar_cache.is_empty(), "a refusal writes nothing");
    assert_eq!(loaded(&h), Vec::new());
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn a_passing_failure_marks_nothing_and_is_retried() {
    let mut h = ServiceHarness::start(served_setup(&[ann()]));
    h.api(&org(CONTOSO))
        .fail(ApiCall::Avatar, AdoError::Timeout);
    refreshed(&mut h, CONTOSO, 1).await;
    h.until("the attempt", |h| {
        h.api(&org(CONTOSO)).calls(ApiCall::Avatar) >= 1
    })
    .await;
    h.settle().await;
    assert!(h.avatar_cache.is_empty());
    assert_eq!(loaded(&h), Vec::new());

    h.api(&org(CONTOSO)).clear_failures();
    h.advance(Duration::from_secs(3)).await;
    h.send(Command::RefreshNow);
    refreshed(&mut h, CONTOSO, 2).await;
    h.until("the face", |h| !loaded(h).is_empty()).await;
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn nearly_spent_rate_limit_skips_downloads() {
    let mut start = served_setup(&[ann()]);
    start.orgs[0].1.rate.limit = Some(200.0);
    start.orgs[0].1.rate.remaining = Some(10.0);
    let mut h = ServiceHarness::start(start);
    refreshed(&mut h, CONTOSO, 1).await;
    h.settle().await;

    assert_eq!(h.api(&org(CONTOSO)).calls(ApiCall::Avatar), 0);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn an_address_outside_the_org_is_never_requested() {
    let stranger = Person {
        avatar: Some("https://elsewhere.example/a.png".into()),
        ..bob()
    };
    let prs = vec![pr_by(1, stranger), pr_by(2, pictured(ann()))];
    let mut start = setup(vec![(CONTOSO, prs)]);
    start.config.orgs[0].people.tracked_authors = vec![ann(), bob()];
    start.orgs[0].1.avatars = [(href(&ann()), Bytes::from(source_png()))].into();
    let mut h = ServiceHarness::start(start);
    refreshed(&mut h, CONTOSO, 1).await;
    h.until("ann", |h| !loaded(h).is_empty()).await;

    assert_eq!(loaded(&h), vec![key_of(&ann())]);
    assert!(
        requests(&h).iter().all(|r| !r.contains("elsewhere")),
        "{:?}",
        requests(&h)
    );
    h.stop().await;
}
