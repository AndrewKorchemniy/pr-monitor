//! A realistic, entirely fictional data set for screenshots: two orgs, a dozen pull requests
//! in every state, a catalog of projects and repositories, and tracked people. Variants crowd
//! the status bar with more orgs or a running refresh, or have nothing open.

use std::sync::Arc;

use prm_config::{Config, OrgProfile};
use prm_domain::{
    Vsid,
    avatar::{AVATAR_PX, AvatarImage, AvatarKey},
    ids::ProjectId,
    model::{Catalog, Project, Repository},
    person::Person,
    scope::{ProjectScope, RepoSel, ScopeSpec},
    testing::{
        BASE_TIME_SECS, PrBuilder, ThreadBuilder, group, key, org, person, record, repo, rows, ts,
    },
    view::summary::summarize,
};
use prm_service::{
    AppState, AuthStateView, LoadedAvatar, RefreshPhase,
    state::{RefreshProgress, RefreshStatus},
};

use crate::testing::org_status;

/// Required reviewer groups on some demo pull requests, by id.
const GROUP_REPO: u128 = 30;
const GROUP_PLATFORM: u128 = 31;
const GROUP_PAYMENTS: u128 = 32;

/// Pictures for two of the demo people (the rest show their initials): a soft vertical
/// gradient in a round frame, standing in for a profile photo.
pub(crate) fn demo_avatars() -> Vec<LoadedAvatar> {
    let side = usize::from(AVATAR_PX);
    let picture = |n: u128, (r, g, b): (u8, u8, u8)| {
        let rgba: Vec<u8> = (0..side * side)
            .flat_map(|i| {
                let fade = 255 - u8::try_from(i / side * 90 / side).unwrap_or(0);
                let (dx, dy) = ((i % side) as f32 - 31.5, (i / side) as f32 - 31.5);
                let alpha = if dx.hypot(dy) < 32.0 { 255 } else { 0 };
                [r.min(fade), g.min(fade), b.min(fade), alpha]
            })
            .collect();
        LoadedAvatar {
            key: AvatarKey::new(org("contoso"), Vsid::from_u128(n)),
            image: AvatarImage::new(AVATAR_PX, rgba).expect("a square of the stated size"),
        }
    };
    vec![picture(1, (230, 150, 70)), picture(2, (70, 150, 230))]
}

/// "Now" for every demo screen: an hour after the newest PR was created.
pub(crate) fn now() -> prm_domain::Timestamp {
    ts(BASE_TIME_SECS + 3_600)
}

fn hours_ago(h: i64) -> prm_domain::Timestamp {
    ts(BASE_TIME_SECS - h * 3_600)
}

fn people() -> [Person; 6] {
    [
        person(1, "Maya Patel"),
        person(2, "Jonas Berg"),
        person(3, "Priya Raman"),
        person(4, "Leo Martins"),
        person(7, "Rita Okafor"),
        person(8, "Sam Whitfield"),
    ]
}

#[expect(
    clippy::too_many_lines,
    reason = "one record per demo pull request reads best as a list"
)]
/// Twelve PRs across four repositories: every status, drafts, conflicts, auto-complete, votes
/// and threads.
fn demo_rows() -> Arc<[prm_domain::view::PrRow]> {
    let [maya, jonas, priya, leo, rita, sam] = people();
    let pr = |repo_n: u128, n: i32, title: &str, author: &Person, age_h: i64| {
        PrBuilder::new(key("contoso", repo_n, n))
            .author(author.clone())
            .title(title)
            .created(hours_ago(age_h))
    };
    let named = |mut pr: prm_domain::model::PullRequest| {
        let names = ["pr-monitor", "build-tools", "docs-site", "telemetry"];
        let name = (1..=4)
            .zip(names)
            .find(|&(n, _)| pr.key.repo == repo(n))
            .map_or("pr-monitor", |(_, name)| name);
        pr.repo_name = name.into();
        pr.project_name = "Platform".into();
        pr
    };
    let comment = |id: i32, who: &Person| ThreadBuilder::human(id).comment(who.clone()).build();
    let records = vec![
        record(
            named(
                pr(
                    1,
                    4711,
                    "Fix token refresh when the az session expires",
                    &maya,
                    2,
                )
                .build(),
            ),
            vec![],
        ),
        record(
            named(pr(1, 4708, "Retry 429 responses with Retry-After", &jonas, 20).build()),
            vec![comment(1, &rita), comment(2, &sam)],
        ),
        record(
            named(
                pr(
                    2,
                    4702,
                    "Paging terminator for continuation tokens",
                    &priya,
                    96,
                )
                .reviewer_for(rita.clone(), -5, &[GROUP_PAYMENTS])
                .required_group(group(GROUP_PAYMENTS, "Payments Approvers"), -5)
                .build(),
            ),
            vec![comment(3, &rita)],
        ),
        record(
            named(prm_domain::model::PullRequest {
                auto_complete: true,
                ..pr(2, 4699, "Group dashboard rows by author", &leo, 30)
                    .reviewer(rita.clone(), 10)
                    .reviewer(sam.clone(), 5)
                    .required_group(group(GROUP_PLATFORM, "Platform Owners"), 0)
                    .build()
            }),
            vec![],
        ),
        record(
            named(
                pr(3, 4690, "Move theme tokens into one registry", &maya, 50)
                    .reviewer_for(sam.clone(), 10, &[GROUP_REPO])
                    .required_group(group(GROUP_REPO, "IPCO Repo Approver - CF"), 10)
                    .required_group(group(GROUP_PLATFORM, "Platform Owners"), 0)
                    .build(),
            ),
            vec![],
        ),
        record(
            named(
                pr(3, 4688, "OKLab contrast guard for borders", &jonas, 5)
                    .draft()
                    .build(),
            ),
            vec![],
        ),
        record(
            named(
                pr(4, 4685, "Toast grouping per pull request", &priya, 140)
                    .conflicts()
                    .build(),
            ),
            vec![comment(4, &sam)],
        ),
        record(
            named(
                pr(
                    4,
                    4681,
                    "Credential Manager chunking for long tokens, so sign-ins that outgrow one entry survive a restart",
                    &leo,
                    8,
                )
                .build(),
            ),
            vec![],
        ),
        record(
            named(
                pr(1, 4677, "Single-instance handoff during shutdown", &sam, 70)
                    .reviewer(rita.clone(), 10)
                    .build(),
            ),
            vec![],
        ),
        record(
            named(pr(2, 4670, "Settings: people picker modal", &maya, 12).build()),
            vec![comment(5, &rita)],
        ),
        record(
            named(
                pr(3, 4664, "Bump windows-rs to 0.62", &jonas, 200)
                    .reviewer(rita, 10)
                    .reviewer(sam, 10)
                    .build(),
            ),
            vec![],
        ),
        record(
            named(pr(4, 4659, "Low-power hover bands", &priya, 26).build()),
            vec![],
        ),
    ];
    rows(&records, &[7, 8]).into()
}

fn demo_catalog() -> Catalog {
    let projects = [(1, "Platform"), (2, "Payments"), (3, "Identity")];
    let repo_names = [
        (1, 1, "pr-monitor"),
        (2, 1, "build-tools"),
        (3, 1, "docs-site"),
        (4, 1, "telemetry"),
        (5, 2, "payments-api"),
        (6, 2, "ledger"),
        (7, 2, "checkout-web"),
        (8, 3, "sign-in"),
        (9, 3, "token-service"),
        (10, 3, "directory-sync"),
    ];
    let project = |n: u128| ProjectId::from_u128(n);
    Catalog {
        fetched_at: now(),
        projects: projects
            .iter()
            .map(|&(n, name)| Project {
                id: project(n),
                name: Arc::from(name),
            })
            .collect(),
        repos: repo_names
            .iter()
            .map(|&(n, p, name)| Repository {
                id: repo(n),
                name: Arc::from(name),
                project: project(p),
                project_name: Arc::from(projects[(p - 1) as usize].1),
                disabled: false,
                in_maintenance: false,
                is_fork: false,
                web_url: None,
            })
            .collect(),
    }
}

fn demo_config() -> Config {
    let [maya, jonas, priya, leo, rita, sam] = people();
    let mut contoso = OrgProfile {
        id: Some(org("contoso")),
        url: "https://dev.azure.com/contoso".to_owned(),
        label: "contoso".to_owned(),
        ..OrgProfile::default()
    };
    contoso.scope = ScopeSpec {
        all_projects: false,
        projects: vec![ProjectScope {
            id: Some(ProjectId::from_u128(1)),
            name: Arc::from("Platform"),
            all_repos: false,
            repos: vec![
                RepoSel {
                    id: Some(repo(1)),
                    name: Arc::from("pr-monitor"),
                },
                RepoSel {
                    id: Some(repo(2)),
                    name: Arc::from("build-tools"),
                },
            ],
        }],
    };
    contoso.people.tracked_authors = vec![maya, jonas, priya, leo];
    contoso.people.reviewers = vec![rita, sam];
    let fabrikam = OrgProfile {
        id: Some(org("fabrikam")),
        url: "https://dev.azure.com/fabrikam".to_owned(),
        label: "fabrikam".to_owned(),
        ..OrgProfile::default()
    };
    Config {
        orgs: vec![contoso, fabrikam],
        ..Config::default()
    }
}

/// Two signed-in orgs, twelve PRs, a catalog and people: what a busy morning looks like.
pub(crate) fn demo_state() -> AppState {
    let rows = demo_rows();
    let mut contoso = org_status("contoso", AuthStateView::SignedIn);
    contoso.catalog = Some(Arc::new(demo_catalog()));
    contoso.auth.me = Some(person(1, "Maya Patel"));
    contoso.auth.method = Some(prm_domain::auth::AuthMethod::AzureCli);
    contoso.scope.repos_in_scope = 2;
    contoso.scope.repos_total = 10;
    let fabrikam = org_status("fabrikam", AuthStateView::SignedIn);
    AppState {
        summary: summarize(rows.iter()),
        rows_generation: 1,
        rows,
        orgs: Arc::from([contoso, fabrikam]),
        config: Arc::new(demo_config()),
        refresh: RefreshStatus {
            last_completed: Some(ts(BASE_TIME_SECS + 3_420)),
            next_due: Some(ts(BASE_TIME_SECS + 3_720)),
            ..RefreshStatus::default()
        },
        ..AppState::default()
    }
}

/// The demo with four orgs, two with long names: a crowded status bar.
pub(crate) fn crowded_state() -> AppState {
    let mut state = demo_state();
    let more = ["northwind-traders", "adventure-works"]
        .map(|name| org_status(name, AuthStateView::SignedIn));
    state.orgs = state.orgs.iter().cloned().chain(more).collect();
    state
}

/// The demo partway through reading comments, slowed by throttling, with notifications paused.
pub(crate) fn busy_state() -> AppState {
    let mut state = demo_state();
    state.refresh.in_progress = true;
    state.refresh.progress = Arc::from([RefreshProgress {
        org: org("contoso"),
        phase: RefreshPhase::Threads,
        done: 12,
        total: 40,
    }]);
    state.refresh.slowed = true;
    state.notifications.paused_until = Some(ts(BASE_TIME_SECS + 7_200));
    state
}

/// The demo orgs on a quiet day: signed in, with no open pull requests.
pub(crate) fn quiet_state() -> AppState {
    let mut state = demo_state();
    state.rows = Arc::from([]);
    state.summary = summarize([]);
    state.rows_generation += 1;
    state
}

/// The demo with contoso's people list emptied and the signed-in user's own pull requests
/// left out: it tracks everyone else in its repositories.
pub(crate) fn everyone_state() -> AppState {
    let mut state = demo_state();
    let mut config = (*state.config).clone();
    config.orgs[0].people.tracked_authors.clear();
    config.orgs[0].include_me = false;
    state.config = Arc::new(config);
    state
}

/// The same data with no orgs configured: the first-run screen.
pub(crate) fn empty_state() -> AppState {
    AppState {
        first_run: true,
        ..AppState::default()
    }
}
