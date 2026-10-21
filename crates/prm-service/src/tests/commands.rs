//! Commands and events: seen marks, links, progress throttling, people, activation.

use std::{sync::Arc, time::Duration};

use prm_ado::{AdoError, testing::ApiCall};
use prm_domain::{
    ids::RequestId,
    person::{CandidateSource, PersonCandidate},
    testing::{org, vsid},
    uri::AppUri,
};

use super::support::{CONTOSO, list_calls, pr, refreshed, setup};
use crate::{
    command::Command,
    event::{AppEvent, SearchPhase},
    state::AuthStateView,
    testing::ServiceHarness,
    text,
};

fn zed() -> PersonCandidate {
    PersonCandidate {
        id: None,
        descriptor: Some("aad.zed".into()),
        name: "Zed Remote".into(),
        unique_name: Some("zed@contoso.com".into()),
        group: false,
        source: CandidateSource::Directory,
    }
}

fn people_results(h: &ServiceHarness) -> Vec<(RequestId, SearchPhase, Vec<Arc<str>>)> {
    h.events()
        .into_iter()
        .filter_map(|e| {
            if let AppEvent::PeopleResults {
                request,
                phase,
                candidates,
                ..
            } = e
            {
                let names = candidates.iter().map(|c| c.name.clone()).collect();
                Some((request, phase, names))
            } else {
                None
            }
        })
        .collect()
}

#[tokio::test(start_paused = true)]
async fn mark_seen_clears_unread_and_persists_debounced() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    h.api(&org(CONTOSO)).update(|o| o.prs.push(pr(CONTOSO, 2)));
    h.send(Command::RefreshNow);
    let key = pr(CONTOSO, 2).key;
    let unread = key.clone();
    h.wait_for("unread row", move |s| {
        s.row(&unread).is_some_and(|r| r.unread)
    })
    .await;
    let repo = h.repo.clone();
    let flushed = key.clone();
    h.wait_for("cycle saved", move |_| {
        repo.last_state()
            .is_some_and(|s| s.orgs[&org(CONTOSO)].records.contains_key(&flushed))
    })
    .await;

    h.send(Command::MarkSeen(vec![key.clone()]));
    let read = key.clone();
    h.wait_for("read row", move |s| s.row(&read).is_some_and(|r| !r.unread))
        .await;
    assert!(
        !h.repo.last_state().unwrap().seen.contains_key(&key),
        "debounced"
    );

    let repo = h.repo.clone();
    let seen = key.clone();
    h.wait_for("seen saved", move |_| {
        repo.last_state()
            .is_some_and(|s| s.seen.contains_key(&seen))
    })
    .await;
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn open_pr_rejects_non_allowlisted_host() {
    let mut evil = pr(CONTOSO, 1);
    evil.web_url = "https://evil.example.com/pull/1".into();
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![evil.clone(), pr(CONTOSO, 2)])]));
    refreshed(&mut h, CONTOSO, 1).await;

    h.send(Command::OpenPr(evil.key.clone()));
    h.send(Command::OpenPr(pr(CONTOSO, 2).key));
    let opener = h.opener.clone();
    h.wait_for("one link opened", move |_| opener.opened().len() == 1)
        .await;

    assert_eq!(
        h.opener.opened()[0].as_str(),
        pr(CONTOSO, 2).web_url.as_ref()
    );
    h.stop().await;
}

const PRS: i32 = 100;

#[tokio::test(start_paused = true)]
async fn progress_publish_throttled_10hz() {
    let prs = (1..=PRS).map(|n| pr(CONTOSO, n)).collect();
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, prs)]));
    let before = h.snapshot().generation;

    let state = refreshed(&mut h, CONTOSO, 1).await;

    // why: job results (capability, autostart, saves) publish unthrottled at varying moments;
    // only the per-PR progress updates are throttled, so compare against their count.
    let publishes = state.generation - before;
    let progress_updates = u64::try_from(PRS).unwrap();
    assert!(
        publishes < progress_updates / 5,
        "{publishes} publishes for {progress_updates}+ progress updates"
    );
    assert_eq!(state.rows.len(), 100);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn people_search_local_then_final_latest_wins() {
    let mut start = setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]);
    start.orgs[0].1.subjects = vec![zed()];
    let mut h = ServiceHarness::start(start);
    refreshed(&mut h, CONTOSO, 1).await;
    let search = |query: &str, n: u64| Command::SearchPeople {
        org: org(CONTOSO),
        query: query.into(),
        request: RequestId(n),
    };

    h.send(search("ze", 1));
    h.send(search("zed", 2));
    h.send(search("zed r", 3));
    h.advance(Duration::from_millis(300)).await;
    let events = h.events.clone();
    h.wait_for("directory results", move |_| {
        events.events().iter().any(|e| {
            matches!(
                e,
                AppEvent::PeopleResults {
                    phase: SearchPhase::Final,
                    request: RequestId(3),
                    ..
                }
            )
        })
    })
    .await;

    let results = people_results(&h);
    let requests = |wanted: &SearchPhase| -> Vec<RequestId> {
        results
            .iter()
            .filter(|(_, phase, _)| phase == wanted)
            .map(|(request, _, _)| *request)
            .collect()
    };
    // why: "ze" is too short for the directory, so its local results are final at once.
    assert_eq!(requests(&SearchPhase::Final), [RequestId(1), RequestId(3)]);
    assert_eq!(requests(&SearchPhase::Local), [RequestId(2), RequestId(3)]);
    let last = results.last().map(|(_, _, names)| names.clone());
    assert!(last.is_some_and(|names| names.iter().any(|n| n.as_ref() == "Zed Remote")));
    h.stop().await;
}

/// A failed directory search used to show only as a passing message; the picker kept saying
/// it was searching.
#[tokio::test(start_paused = true)]
async fn people_search_failure_reported_with_local_results() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    h.api(&org(CONTOSO))
        .fail(ApiCall::SearchSubjects, AdoError::Network("down".into()));

    h.send(Command::SearchPeople {
        org: org(CONTOSO),
        query: "ann".into(),
        request: RequestId(1),
    });
    h.advance(Duration::from_millis(300)).await;
    let events = h.events.clone();
    h.wait_for("finished search", move |_| {
        events.events().iter().any(|e| {
            matches!(
                e,
                AppEvent::PeopleResults {
                    phase: SearchPhase::Failed(_),
                    ..
                }
            )
        })
    })
    .await;

    let results = people_results(&h);
    let Some((request, SearchPhase::Failed(error), names)) = results.last() else {
        panic!("expected a failed final phase: {results:?}");
    };
    assert_eq!(*request, RequestId(1));
    assert_eq!(error.title.as_ref(), text::SEARCH_FAILED);
    assert!(names.iter().any(|n| n.as_ref() == "Ann"), "{names:?}");
    h.stop().await;
}

/// Signed out, the directory is not searched; the picker must not read that as "nobody
/// matches".
#[tokio::test(start_paused = true)]
async fn people_search_signed_out_is_local_only() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    h.send(Command::SignOut { org: org(CONTOSO) });
    h.wait_for("signed out", |s| {
        s.org(&org(CONTOSO))
            .is_some_and(|o| o.auth.state == AuthStateView::SignedOut)
    })
    .await;

    h.send(Command::SearchPeople {
        org: org(CONTOSO),
        query: "ann".into(),
        request: RequestId(1),
    });
    h.advance(Duration::from_millis(300)).await;
    h.settle().await;

    let results = people_results(&h);
    let [(request, SearchPhase::LocalOnly, names)] = results.as_slice() else {
        panic!("expected one local-only phase: {results:?}");
    };
    assert_eq!(*request, RequestId(1));
    assert!(names.iter().any(|n| n.as_ref() == "Ann"), "{names:?}");
    assert_eq!(h.api(&org(CONTOSO)).calls(ApiCall::SearchSubjects), 0);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn people_search_finds_harvested_people_locally() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;

    h.send(Command::SearchPeople {
        org: org(CONTOSO),
        query: "an".into(),
        request: RequestId(1),
    });
    h.settle().await;

    let results = people_results(&h);
    assert_eq!(results.len(), 1);
    assert!(results[0].2.iter().any(|n| n.as_ref() == "Ann"));
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn resolve_person_via_storage_key() {
    let mut start = setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]);
    start.orgs[0]
        .1
        .storage_keys
        .insert("aad.zed".into(), vsid(9));
    let mut h = ServiceHarness::start(start);
    refreshed(&mut h, CONTOSO, 1).await;

    h.send(Command::ResolvePerson {
        org: org(CONTOSO),
        candidate: zed(),
        request: RequestId(5),
    });
    let events = h.events.clone();
    h.wait_for("resolved", move |_| {
        events
            .events()
            .iter()
            .any(|e| matches!(e, AppEvent::PersonResolved { .. }))
    })
    .await;

    let resolved = h.events().into_iter().find_map(|e| {
        if let AppEvent::PersonResolved {
            request, result, ..
        } = e
        {
            Some((request, result))
        } else {
            None
        }
    });
    let (request, person) = resolved.unwrap();
    assert_eq!(request, RequestId(5));
    let person = person.unwrap();
    assert_eq!(person.id, vsid(9));
    assert_eq!(person.name.as_ref(), "Zed Remote");
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn activation_show_forwarded_refresh_triggers() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    let show = AppUri::Show {
        key: Some(pr(CONTOSO, 1).key),
    };

    h.send(Command::Activate(show.clone()));
    h.send(Command::Activate(AppUri::Refresh));
    refreshed(&mut h, CONTOSO, 2).await;

    assert!(h.events().contains(&AppEvent::Activate(show)));
    assert_eq!(list_calls(&h, CONTOSO), 2);
    h.stop().await;
}

#[tokio::test(start_paused = true)]
async fn test_notification_bypasses_policy() {
    let mut h = ServiceHarness::start(setup(vec![(CONTOSO, vec![pr(CONTOSO, 1)])]));
    refreshed(&mut h, CONTOSO, 1).await;
    h.send(Command::UiPresence {
        visible: true,
        focused: true,
    });

    h.send(Command::TestNotification);
    let events = h.events.clone();
    h.wait_for("test result", move |_| {
        events
            .events()
            .contains(&AppEvent::NotificationTestResult(Ok(())))
    })
    .await;

    assert_eq!(h.notifier.shown().len(), 1);
    h.stop().await;
}
