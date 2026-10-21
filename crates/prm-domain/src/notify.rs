//! The notification policy: change events in, toast operations out. Deterministic; the
//! platform layer only renders what this decides.

pub mod text;

use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use serde::{Deserialize, Serialize};

use crate::{
    diff::{ChangeEvent, ChangeKind},
    ids::{OrgId, PrKey},
    uri::AppUri,
};

pub const GROUP_PRS: &str = "prs";
pub const GROUP_SUMMARY: &str = "summary";
pub const GROUP_AUTH: &str = "auth";
pub const TAG_SUMMARY: &str = "summary";
/// Toasts per refresh are clamped to this range.
pub const MAX_TOASTS_RANGE: std::ops::RangeInclusive<u8> = 1..=5;
/// Toasts per refresh unless the user changes it.
pub const DEFAULT_MAX_TOASTS: u8 = 3;

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    NewPr,
    ReadyForReview,
    NeedsReReview,
    Reviewed,
    ChangesRequested,
    NewVote,
    NewComments,
    CommentsResolved,
    Reactivated,
    Closed,
    NewPush,
    AuthProblem,
}

impl EventType {
    pub const fn of(kind: &ChangeKind) -> Self {
        match kind {
            ChangeKind::NewPr => Self::NewPr,
            ChangeKind::Reactivated => Self::Reactivated,
            ChangeKind::ReadyForReview => Self::ReadyForReview,
            ChangeKind::Closed { .. } => Self::Closed,
            ChangeKind::Reviewed { .. } => Self::Reviewed,
            ChangeKind::NeedsReReview => Self::NeedsReReview,
            ChangeKind::ChangesRequested { .. } => Self::ChangesRequested,
            ChangeKind::NewVote { .. } => Self::NewVote,
            ChangeKind::NewComments { .. } => Self::NewComments,
            ChangeKind::CommentsResolved => Self::CommentsResolved,
            ChangeKind::NewPush => Self::NewPush,
        }
    }

    /// Which change becomes a PR's toast headline: higher wins.
    pub const fn priority(self) -> u8 {
        match self {
            Self::ChangesRequested => 100,
            Self::AuthProblem => 95,
            Self::NeedsReReview => 90,
            Self::NewComments => 80,
            Self::NewPr => 70,
            Self::ReadyForReview => 65,
            Self::Reviewed => 60,
            Self::Reactivated => 50,
            Self::CommentsResolved => 40,
            Self::NewVote => 30,
            Self::NewPush => 20,
            Self::Closed => 10,
        }
    }
}

/// Which change kinds may raise a toast. Defaults: all on except new votes, closes and pushes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EventToggles {
    pub new_pr: bool,
    pub ready_for_review: bool,
    pub needs_re_review: bool,
    pub reviewed: bool,
    pub changes_requested: bool,
    pub new_vote: bool,
    pub new_comments: bool,
    pub comments_resolved: bool,
    pub reactivated: bool,
    pub closed: bool,
    pub new_push: bool,
    pub auth_problem: bool,
}

impl Default for EventToggles {
    fn default() -> Self {
        Self {
            new_pr: true,
            ready_for_review: true,
            needs_re_review: true,
            reviewed: true,
            changes_requested: true,
            new_vote: false,
            new_comments: true,
            comments_resolved: true,
            reactivated: true,
            closed: false,
            new_push: false,
            auth_problem: true,
        }
    }
}

impl EventToggles {
    pub const fn enabled(&self, event: EventType) -> bool {
        match event {
            EventType::NewPr => self.new_pr,
            EventType::ReadyForReview => self.ready_for_review,
            EventType::NeedsReReview => self.needs_re_review,
            EventType::Reviewed => self.reviewed,
            EventType::ChangesRequested => self.changes_requested,
            EventType::NewVote => self.new_vote,
            EventType::NewComments => self.new_comments,
            EventType::CommentsResolved => self.comments_resolved,
            EventType::Reactivated => self.reactivated,
            EventType::Closed => self.closed,
            EventType::NewPush => self.new_push,
            EventType::AuthProblem => self.auth_problem,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WhenFocused {
    Show,
    /// While the window is in front, skip OS toasts (in-app highlights still show).
    /// Last because `#[serde(other)]` must be the final variant.
    #[default]
    #[serde(other)]
    Suppress,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationPrefs {
    pub enabled: bool,
    pub when_focused: WhenFocused,
    /// Clamped to [`MAX_TOASTS_RANGE`]. A sign-in toast never takes the last slot, so PR
    /// changes always get at least one toast.
    #[serde(deserialize_with = "crate::lenient::saturating_u8")]
    pub max_per_refresh: u8,
    pub sound: bool,
    pub include_drafts: bool,
    pub events: EventToggles,
}

impl Default for NotificationPrefs {
    fn default() -> Self {
        Self {
            enabled: true,
            when_focused: WhenFocused::Suppress,
            max_per_refresh: DEFAULT_MAX_TOASTS,
            sound: true,
            include_drafts: false,
            events: EventToggles::default(),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NotificationKind {
    Pr,
    Summary,
    Auth,
    Test,
}

/// A toast, ready to render. `tag` + `group` identify it, so a later toast with the same
/// pair replaces it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notification {
    pub tag: String,
    pub group: String,
    pub title: String,
    pub body: String,
    /// Optional third text line (comment snippet, re-review reason).
    pub detail: Option<String>,
    pub attribution: Option<String>,
    /// An [`AppUri`] the toast body activates.
    pub launch: String,
    pub open_url: Option<String>,
    pub silent: bool,
    pub kind: NotificationKind,
}

impl Notification {
    /// The "Send a test notification" toast; bypasses the policy.
    pub fn test() -> Self {
        Self {
            tag: "test".into(),
            group: GROUP_SUMMARY.into(),
            title: text::TEST_TITLE.into(),
            body: text::test_body(),
            detail: None,
            attribution: None,
            launch: AppUri::Show { key: None }.to_string(),
            open_url: None,
            silent: false,
            kind: NotificationKind::Test,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotifierOp {
    Show(Notification),
    Retract { tag: String, group: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthProblem {
    pub org: OrgId,
    pub org_label: Arc<str>,
    pub message: Arc<str>,
}

pub struct PolicyInput<'a> {
    pub prefs: &'a NotificationPrefs,
    pub events: &'a [ChangeEvent],
    /// Orgs that newly need sign-in this cycle.
    pub auth_problems: &'a [AuthProblem],
    pub paused: bool,
    pub ui_focused: bool,
    pub muted_orgs: &'a HashSet<OrgId>,
    pub multi_org: bool,
    pub org_labels: &'a BTreeMap<OrgId, Arc<str>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct NotificationPlan {
    pub ops: Vec<NotifierOp>,
    /// Events that would have toasted but were gated (paused, focused, disabled).
    pub suppressed: u32,
}

/// One PR's events, headed by its highest-priority change.
struct PrGroup<'a> {
    headline: &'a ChangeEvent,
    extra: usize,
}

pub fn plan_notifications(input: &PolicyInput<'_>) -> NotificationPlan {
    let (shows, suppressed) = plan_toasts(input);
    let mut ops = closed_retractions(input.events, &shows);
    ops.extend(shows.into_iter().map(NotifierOp::Show));
    NotificationPlan { ops, suppressed }
}

/// The toasts to show, and how many events the gates suppressed.
fn plan_toasts(input: &PolicyInput<'_>) -> (Vec<Notification>, u32) {
    let prefs = input.prefs;
    let auth_problems: Vec<AuthProblem> = input
        .auth_problems
        .iter()
        .filter(|p| !input.muted_orgs.contains(&p.org))
        .cloned()
        .collect();
    let focused_gate = input.ui_focused && prefs.when_focused == WhenFocused::Suppress;
    if !prefs.enabled || input.paused || focused_gate {
        let gated = input.events.len() + auth_problems.len();
        return (Vec::new(), u32::try_from(gated).unwrap_or(u32::MAX));
    }
    let groups = group_by_pr(input.events.iter().filter(|e| should_toast(e, input)));
    let mut budget = usize::from(
        prefs
            .max_per_refresh
            .clamp(*MAX_TOASTS_RANGE.start(), *MAX_TOASTS_RANGE.end()),
    );
    let mut shows = Vec::new();
    if prefs.events.auth_problem && !auth_problems.is_empty() {
        shows.push(auth_toast(&auth_problems, prefs.sound));
        // why: the sign-in toast never takes the last slot, or PR changes would vanish unseen.
        budget = budget.saturating_sub(1).max(1);
    }
    shows.extend(fit_to_budget(&groups, budget, input));
    (shows, 0)
}

/// A closed PR's earlier toast is withdrawn unless this plan shows one that replaces it
/// (a "closed" toast with the same tag).
fn closed_retractions(events: &[ChangeEvent], shows: &[Notification]) -> Vec<NotifierOp> {
    let shown: HashSet<&str> = shows
        .iter()
        .filter(|n| n.kind == NotificationKind::Pr)
        .map(|n| n.tag.as_str())
        .collect();
    events
        .iter()
        .filter(|e| matches!(e.kind, ChangeKind::Closed { .. }))
        .map(|e| e.pr.key.tag())
        .filter(|tag| !shown.contains(tag.as_str()))
        .map(|tag| NotifierOp::Retract {
            tag,
            group: GROUP_PRS.into(),
        })
        .collect()
}

fn should_toast(event: &ChangeEvent, input: &PolicyInput<'_>) -> bool {
    let prefs = input.prefs;
    let draft_ok =
        !event.pr.draft || prefs.include_drafts || matches!(event.kind, ChangeKind::ReadyForReview);
    prefs.events.enabled(EventType::of(&event.kind))
        && !input.muted_orgs.contains(&event.pr.key.org)
        && draft_ok
}

/// Groups by PR, ordered by headline priority, then newest, then key.
fn group_by_pr<'a>(events: impl Iterator<Item = &'a ChangeEvent>) -> Vec<PrGroup<'a>> {
    let mut by_pr: BTreeMap<&PrKey, Vec<&ChangeEvent>> = BTreeMap::new();
    for event in events {
        by_pr.entry(&event.pr.key).or_default().push(event);
    }
    let mut groups: Vec<PrGroup<'a>> = by_pr
        .into_values()
        .filter_map(|list| {
            let headline = *list.iter().max_by_key(|e| priority(e))?;
            Some(PrGroup {
                headline,
                extra: list.len() - 1,
            })
        })
        .collect();
    groups.sort_by(|a, b| {
        priority(b.headline)
            .cmp(&priority(a.headline))
            .then(b.headline.at.cmp(&a.headline.at))
            .then(a.headline.pr.key.cmp(&b.headline.pr.key))
    });
    groups
}

fn priority(event: &ChangeEvent) -> u8 {
    EventType::of(&event.kind).priority()
}

/// All groups if they fit; otherwise `budget - 1` PR toasts and one summary for the rest.
fn fit_to_budget(
    groups: &[PrGroup<'_>],
    budget: usize,
    input: &PolicyInput<'_>,
) -> Vec<Notification> {
    if groups.is_empty() {
        return Vec::new();
    }
    let shown = if groups.len() <= budget {
        groups.len()
    } else {
        budget.saturating_sub(1)
    };
    let (individual, rest) = groups.split_at(shown);
    let mut toasts: Vec<Notification> = individual.iter().map(|g| pr_toast(g, input)).collect();
    if !rest.is_empty() {
        toasts.push(summary_toast(rest, input.prefs.sound));
    }
    toasts
}

fn pr_toast(group: &PrGroup<'_>, input: &PolicyInput<'_>) -> Notification {
    let pr = &group.headline.pr;
    let headline = text::headline(&group.headline.kind, pr.key.number.0, group.extra);
    let org_label = input.multi_org.then(|| {
        input
            .org_labels
            .get(&pr.key.org)
            .map_or(pr.key.org.as_str(), |l| l)
    });
    Notification {
        tag: pr.key.tag(),
        group: GROUP_PRS.into(),
        title: headline.title,
        body: pr.title.to_string(),
        detail: headline.detail,
        attribution: Some(text::attribution(
            &pr.repo_name,
            &pr.author.display_name(),
            org_label,
        )),
        launch: AppUri::Show {
            key: Some(pr.key.clone()),
        }
        .to_string(),
        open_url: Some(pr.web_url.to_string()),
        silent: !input.prefs.sound,
        kind: NotificationKind::Pr,
    }
}

fn summary_toast(rest: &[PrGroup<'_>], sound: bool) -> Notification {
    Notification {
        tag: TAG_SUMMARY.into(),
        group: GROUP_SUMMARY.into(),
        title: text::summary_title(rest.len()),
        body: text::summary_body(rest.iter().map(|g| &*g.headline.pr.title)),
        detail: None,
        attribution: None,
        launch: AppUri::Show { key: None }.to_string(),
        open_url: None,
        silent: !sound,
        kind: NotificationKind::Summary,
    }
}

fn auth_toast(problems: &[AuthProblem], sound: bool) -> Notification {
    let (tag, body) = match problems {
        [only] => (
            format!("auth-{}", only.org),
            format!("{}: {}", only.org_label, only.message),
        ),
        many => (
            "auth-multi".to_owned(),
            many.iter()
                .map(|p| &*p.org_label)
                .collect::<Vec<_>>()
                .join(", "),
        ),
    };
    Notification {
        tag,
        group: GROUP_AUTH.into(),
        title: text::auth_title(problems.len()),
        body,
        detail: None,
        attribution: None,
        launch: AppUri::Show { key: None }.to_string(),
        open_url: None,
        silent: !sound,
        kind: NotificationKind::Auth,
    }
}

#[cfg(test)]
mod tests;
