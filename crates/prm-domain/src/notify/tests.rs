use std::collections::{BTreeMap, HashSet};

use super::*;
use crate::{
    model::VoteKind,
    snapshot::PrHeader,
    testing::{PrBuilder, key, org, person, record, ts},
    tombstone::CloseOutcome,
};

fn header(number: i32) -> PrHeader {
    PrHeader::from_record(&record(
        PrBuilder::new(key("contoso", 1, number))
            .title(&format!("Title {number}"))
            .build(),
        vec![],
    ))
}

fn event(number: i32, kind: ChangeKind) -> ChangeEvent {
    ChangeEvent {
        pr: header(number),
        kind,
        at: ts(1_000 + i64::from(number)),
    }
}

fn new_comment(number: i32) -> ChangeEvent {
    event(
        number,
        ChangeKind::NewComments {
            count: 1,
            authors: vec![Arc::from("Rae")].into(),
            snippet: Some("Looks off".into()),
        },
    )
}

struct Setup {
    prefs: NotificationPrefs,
    events: Vec<ChangeEvent>,
    auth: Vec<AuthProblem>,
    paused: bool,
    focused: bool,
    muted: HashSet<OrgId>,
    multi_org: bool,
}

impl Setup {
    fn new(events: Vec<ChangeEvent>) -> Self {
        Self {
            prefs: NotificationPrefs::default(),
            events,
            auth: Vec::new(),
            paused: false,
            focused: false,
            muted: HashSet::new(),
            multi_org: false,
        }
    }

    fn plan(&self) -> NotificationPlan {
        let labels: BTreeMap<OrgId, Arc<str>> = [(org("contoso"), Arc::from("Contoso"))].into();
        plan_notifications(&PolicyInput {
            prefs: &self.prefs,
            events: &self.events,
            auth_problems: &self.auth,
            paused: self.paused,
            ui_focused: self.focused,
            muted_orgs: &self.muted,
            multi_org: self.multi_org,
            org_labels: &labels,
        })
    }
}

fn shown(plan: &NotificationPlan) -> Vec<&Notification> {
    plan.ops
        .iter()
        .filter_map(|op| match op {
            NotifierOp::Show(n) => Some(n),
            NotifierOp::Retract { .. } => None,
        })
        .collect()
}

fn titles(plan: &NotificationPlan) -> Vec<String> {
    shown(plan).iter().map(|n| n.title.clone()).collect()
}

#[test]
fn policy_disabled_only_retracts() {
    let mut setup = Setup::new(vec![
        new_comment(1),
        event(
            2,
            ChangeKind::Closed {
                outcome: CloseOutcome::Completed,
            },
        ),
    ]);
    setup.prefs.enabled = false;
    let plan = setup.plan();
    assert_eq!(plan.ops.len(), 1);
    assert!(matches!(plan.ops[0], NotifierOp::Retract { .. }));
    assert_eq!(plan.suppressed, 2);
}

#[test]
fn policy_paused_suppresses() {
    let mut setup = Setup::new(vec![new_comment(1)]);
    setup.paused = true;
    assert_eq!(
        setup.plan(),
        NotificationPlan {
            ops: vec![],
            suppressed: 1
        }
    );
}

#[test]
fn policy_focused_suppress_vs_show() {
    let mut setup = Setup::new(vec![new_comment(1)]);
    setup.focused = true;
    assert_eq!(shown(&setup.plan()).len(), 0);
    setup.prefs.when_focused = WhenFocused::Show;
    assert_eq!(shown(&setup.plan()).len(), 1);
}

#[test]
fn policy_toggle_filters() {
    let mut setup = Setup::new(vec![new_comment(1)]);
    setup.prefs.events.new_comments = false;
    assert_eq!(shown(&setup.plan()).len(), 0);
}

#[test]
fn policy_muted_org() {
    let mut setup = Setup::new(vec![new_comment(1)]);
    setup.muted.insert(org("contoso"));
    assert_eq!(shown(&setup.plan()).len(), 0);
}

#[test]
fn policy_drafts_excluded_unless_included_or_ready() {
    let mut draft_comment = new_comment(1);
    draft_comment.pr.draft = true;
    let mut ready = event(2, ChangeKind::ReadyForReview);
    ready.pr.draft = true;
    let mut setup = Setup::new(vec![draft_comment, ready]);
    assert_eq!(titles(&setup.plan()), vec!["Ready for review · !2"]);
    setup.prefs.include_drafts = true;
    assert_eq!(shown(&setup.plan()).len(), 2);
}

#[test]
fn policy_one_toast_per_pr_highest_priority() {
    let by = person(2, "Rev");
    let setup = Setup::new(vec![
        new_comment(1),
        event(
            1,
            ChangeKind::ChangesRequested {
                by,
                vote: VoteKind::Rejected,
            },
        ),
    ]);
    assert_eq!(titles(&setup.plan()), vec!["Rev rejected · !1 (+1 more)"]);
}

#[test]
fn policy_three_prs_three_toasts() {
    let setup = Setup::new((1..=3).map(new_comment).collect());
    assert_eq!(shown(&setup.plan()).len(), 3);
}

#[test]
fn policy_five_prs_two_plus_summary() {
    let setup = Setup::new((1..=5).map(new_comment).collect());
    let plan = setup.plan();
    let toasts = shown(&plan);
    assert_eq!(toasts.len(), 3);
    assert_eq!(toasts[2].kind, NotificationKind::Summary);
    assert_eq!(toasts[2].title, "3 more pull requests updated");
}

#[test]
fn policy_max_1_single_summary() {
    let mut setup = Setup::new((1..=2).map(new_comment).collect());
    setup.prefs.max_per_refresh = 1;
    let plan = setup.plan();
    let toasts = shown(&plan);
    assert_eq!(toasts.len(), 1);
    assert_eq!(toasts[0].kind, NotificationKind::Summary);
}

#[test]
fn policy_auth_problem_first_and_merged() {
    let problem = |name: &str| AuthProblem {
        org: org(name),
        org_label: Arc::from(name),
        message: Arc::from("Sign in again."),
    };
    let mut setup = Setup::new(vec![new_comment(1)]);
    setup.auth = vec![problem("contoso"), problem("fabrikam")];
    let plan = setup.plan();
    let toasts = shown(&plan);
    assert_eq!(toasts[0].tag, "auth-multi");
    assert_eq!(toasts[0].title, "Sign-in needed for 2 organizations");
    assert_eq!(toasts.len(), 2);
}

#[test]
fn policy_closed_retracts() {
    let closed = event(
        1,
        ChangeKind::Closed {
            outcome: CloseOutcome::Abandoned,
        },
    );
    let plan = Setup::new(vec![closed.clone()]).plan();
    assert_eq!(
        plan.ops,
        vec![NotifierOp::Retract {
            tag: closed.pr.key.tag(),
            group: GROUP_PRS.into()
        }]
    );
}

#[test]
fn policy_multi_org_attribution() {
    let mut setup = Setup::new(vec![new_comment(1)]);
    assert_eq!(
        shown(&setup.plan())[0].attribution.as_deref(),
        Some("api · Author One")
    );
    setup.multi_org = true;
    assert_eq!(
        shown(&setup.plan())[0].attribution.as_deref(),
        Some("api · Author One · Contoso")
    );
}

#[test]
fn policy_pr_toast_links_and_tags() {
    let plan = Setup::new(vec![new_comment(7)]).plan();
    let toast = shown(&plan)[0].clone();
    assert_eq!(toast.tag, key("contoso", 1, 7).tag());
    assert_eq!(toast.body, "Title 7");
    assert_eq!(toast.detail.as_deref(), Some("“Looks off”"));
    assert!(toast.launch.starts_with("prmonitor://show?org=contoso"));
    assert!(toast.open_url.unwrap().ends_with("/pullrequest/7"));
}

fn closed(number: i32) -> ChangeEvent {
    event(
        number,
        ChangeKind::Closed {
            outcome: CloseOutcome::Completed,
        },
    )
}

fn retracted(plan: &NotificationPlan) -> Vec<String> {
    plan.ops
        .iter()
        .filter_map(|op| match op {
            NotifierOp::Retract { tag, .. } => Some(tag.clone()),
            NotifierOp::Show(_) => None,
        })
        .collect()
}

fn sign_in_problem(name: &str) -> AuthProblem {
    AuthProblem {
        org: org(name),
        org_label: Arc::from(name),
        message: Arc::from("Sign in again."),
    }
}

#[test]
fn policy_closed_toast_replaces_instead_of_retracting() {
    let mut setup = Setup::new(vec![closed(1)]);
    setup.prefs.events.closed = true;
    let plan = setup.plan();
    assert_eq!(retracted(&plan), Vec::<String>::new());
    assert_eq!(titles(&plan), vec!["Completed · !1"]);
}

/// The "closed" toggle used to skip retraction even when the closed toast was then gated,
/// muted, filtered as a draft or folded into the summary, leaving a stale toast behind.
#[test]
fn policy_closed_retracts_when_closed_toast_is_dropped() {
    let tag = key("contoso", 1, 1).tag();
    type Arrange = fn(&mut Setup);
    let cases: [(&str, Arrange); 5] = [
        ("paused", |s| s.paused = true),
        ("focused", |s| s.focused = true),
        ("muted", |s| {
            s.muted.insert(org("contoso"));
        }),
        ("draft", |s| s.events[0].pr.draft = true),
        ("overflow", |s| {
            s.prefs.max_per_refresh = 1;
            s.events.push(new_comment(2));
        }),
    ];
    for (name, arrange) in cases {
        let mut setup = Setup::new(vec![closed(1)]);
        setup.prefs.events.closed = true;
        arrange(&mut setup);
        let plan = setup.plan();
        assert_eq!(retracted(&plan), vec![tag.clone()], "{name}: {plan:?}");
        assert!(
            shown(&plan).iter().all(|n| n.tag != tag),
            "{name}: {plan:?}"
        );
    }
}

/// With one toast allowed, a sign-in toast used to leave no room, so every PR change of
/// that refresh vanished without even a summary.
#[test]
fn policy_auth_toast_never_takes_the_last_slot() {
    let mut setup = Setup::new((1..=2).map(new_comment).collect());
    setup.prefs.max_per_refresh = 1;
    setup.auth = vec![sign_in_problem("fabrikam")];
    let plan = setup.plan();
    let kinds: Vec<NotificationKind> = shown(&plan).iter().map(|n| n.kind).collect();
    assert_eq!(
        kinds,
        vec![NotificationKind::Auth, NotificationKind::Summary]
    );
}

#[test]
fn policy_muted_org_raises_no_sign_in_toast() {
    let mut setup = Setup::new(vec![]);
    setup.auth = vec![sign_in_problem("contoso")];
    setup.muted.insert(org("contoso"));
    assert_eq!(setup.plan(), NotificationPlan::default());
}
