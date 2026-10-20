//! Dashboard rows, precomputed once per data change so the UI never formats per frame.

use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

use crate::{
    classify::{PrStatus, ReviewState, ReviewerSet, VoteTally, is_human_thread, is_unresolved},
    ids::{OrgId, PrKey, Vsid},
    model::{MergeStatus, PrRecord, Reviewer, ThreadDigest, VoteKind},
    person::Person,
    snapshot::{ClassifiedPr, Fingerprint, OrgSnapshot, SeenMap},
    time::Timestamp,
    view::{
        age::{AgeLabel, age_label},
        text::{initials, strip_ref_prefix},
    },
};

/// How many open threads the peek drawer previews.
pub const MAX_THREAD_PREVIEWS: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewerChip {
    pub name: Arc<str>,
    pub initials: Arc<str>,
    pub vote: VoteKind,
    pub required: bool,
    /// On the org's reviewer list (their vote counts).
    pub listed: bool,
    pub group: bool,
}

/// An open human thread, for the peek drawer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadPreview {
    pub author: Arc<str>,
    pub file: Option<Arc<str>>,
    pub snippet: Arc<str>,
    pub at: Option<Timestamp>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrRow {
    pub key: PrKey,
    pub org_label: Arc<str>,
    /// "!4711".
    pub number_text: Arc<str>,
    pub title: Arc<str>,
    pub repo_name: Arc<str>,
    pub project_name: Arc<str>,
    pub author: Person,
    pub author_initials: Arc<str>,
    pub status: PrStatus,
    pub review: ReviewState,
    pub tally: VoteTally,
    pub active_threads: u32,
    pub total_threads: u32,
    /// "2/5": active/total human threads.
    pub comments_text: Arc<str>,
    pub draft: bool,
    pub conflicts: bool,
    pub auto_complete: bool,
    pub source_branch: Arc<str>,
    pub target_branch: Arc<str>,
    pub created_at: Timestamp,
    /// The latest of creation, thread updates and comment times.
    pub last_activity: Timestamp,
    /// Valid when built; the UI refreshes ages through `AgeCache`.
    pub age: AgeLabel,
    /// Listed voters first, then required reviewers, then by name.
    pub reviewers: Arc<[ReviewerChip]>,
    pub threads_preview: Arc<[ThreadPreview]>,
    pub web_url: Arc<str>,
    /// Lowercase "!4711 title repo project author unique source target org" for filtering.
    pub haystack: Box<str>,
    pub title_key: Box<str>,
    pub repo_key: Box<str>,
    pub author_key: Box<str>,
    pub fingerprint: Fingerprint,
    /// Changed since the user last looked (fingerprint differs from the seen one).
    pub unread: bool,
    pub stale: bool,
}

/// One org's data for [`build_rows`].
pub struct RowBuildInput<'a> {
    pub org: &'a OrgId,
    pub org_label: &'a Arc<str>,
    pub snapshot: &'a OrgSnapshot,
    pub records: &'a BTreeMap<PrKey, PrRecord>,
    pub reviewers: &'a ReviewerSet,
    pub ignored: &'a HashSet<Vsid>,
    /// The whole org failed its last refresh; every row is shown as stale.
    pub org_stale: bool,
}

/// Rows for all orgs, in default order: attention rank, then most recent activity, then key.
pub fn build_rows(orgs: &[RowBuildInput<'_>], seen: &SeenMap, now: Timestamp) -> Vec<PrRow> {
    let mut rows: Vec<PrRow> = orgs
        .iter()
        .flat_map(|org| {
            org.snapshot.values().filter_map(move |classified| {
                let record = org.records.get(&classified.header.key)?;
                Some(build_row(org, classified, record, seen, now))
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        a.status
            .attention_rank()
            .cmp(&b.status.attention_rank())
            .then(b.last_activity.cmp(&a.last_activity))
            .then(a.key.cmp(&b.key))
    });
    rows
}

fn build_row(
    org: &RowBuildInput<'_>,
    classified: &ClassifiedPr,
    record: &PrRecord,
    seen: &SeenMap,
    now: Timestamp,
) -> PrRow {
    let header = &classified.header;
    let class = &classified.classification;
    let pr = &record.pr;
    let author_name = header.author.display_name();
    PrRow {
        key: header.key.clone(),
        org_label: org.org_label.clone(),
        number_text: format!("!{}", header.key.number).into(),
        title: header.title.clone(),
        repo_name: header.repo_name.clone(),
        project_name: header.project_name.clone(),
        author: header.author.clone(),
        author_initials: initials(&author_name).into(),
        status: class.status,
        review: class.review,
        tally: class.tally,
        active_threads: class.active_threads,
        total_threads: class.total_threads,
        comments_text: format!("{}/{}", class.active_threads, class.total_threads).into(),
        draft: header.draft,
        conflicts: pr.merge_status == MergeStatus::Conflicts,
        auto_complete: pr.auto_complete,
        source_branch: strip_ref_prefix(&pr.source_branch).into(),
        target_branch: strip_ref_prefix(&pr.target_branch).into(),
        created_at: pr.created_at,
        last_activity: last_activity(record),
        age: age_label(now, pr.created_at),
        reviewers: reviewer_chips(&pr.reviewers, org.reviewers).into(),
        threads_preview: thread_previews(&record.threads, org.ignored).into(),
        web_url: header.web_url.clone(),
        haystack: haystack(classified, record, org.org_label).into(),
        title_key: header.title.to_lowercase().into(),
        repo_key: header.repo_name.to_lowercase().into(),
        author_key: author_name.to_lowercase().into(),
        fingerprint: classified.fingerprint,
        unread: seen.get(&header.key) != Some(&classified.fingerprint),
        stale: classified.stale || org.org_stale,
    }
}

fn last_activity(record: &PrRecord) -> Timestamp {
    let thread_times = record.threads.iter().filter_map(|t| t.last_updated);
    let comment_times = record
        .threads
        .iter()
        .flat_map(|t| &t.comments)
        .filter_map(|c| c.published);
    thread_times
        .chain(comment_times)
        .fold(record.pr.created_at, Timestamp::max)
}

fn reviewer_chips(reviewers: &[Reviewer], listed: &ReviewerSet) -> Vec<ReviewerChip> {
    let mut chips: Vec<ReviewerChip> = reviewers
        .iter()
        .map(|r| {
            let name = r.who.display_name();
            ReviewerChip {
                initials: initials(&name).into(),
                name,
                vote: r.vote.kind(),
                required: r.required,
                listed: listed.contains(&r.who.id),
                group: r.who.group,
            }
        })
        .collect();
    chips.sort_by(|a, b| {
        let a_key = (!(a.listed && a.vote.is_review()), !a.required);
        let b_key = (!(b.listed && b.vote.is_review()), !b.required);
        a_key
            .cmp(&b_key)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    chips
}

/// The newest open human threads, newest first.
fn thread_previews(threads: &[ThreadDigest], ignored: &HashSet<Vsid>) -> Vec<ThreadPreview> {
    let mut open: Vec<ThreadPreview> = threads
        .iter()
        .filter(|t| is_unresolved(t.status) && is_human_thread(t, ignored))
        .filter_map(|t| {
            let first = t.comments.iter().find(|c| !c.deleted)?;
            let newest = t.comments.iter().filter_map(|c| c.published).max();
            Some(ThreadPreview {
                author: first
                    .author
                    .as_ref()
                    .map_or_else(|| Arc::from(""), Person::display_name),
                file: t.file_path.clone(),
                snippet: first.snippet.clone().unwrap_or_else(|| Arc::from("")),
                at: t.last_updated.or(newest),
            })
        })
        .collect();
    open.sort_by_key(|preview| std::cmp::Reverse(preview.at));
    open.truncate(MAX_THREAD_PREVIEWS);
    open
}

fn haystack(classified: &ClassifiedPr, record: &PrRecord, org_label: &str) -> String {
    let header = &classified.header;
    let pr = &record.pr;
    [
        &format!("!{}", header.key.number),
        &*header.title,
        &*header.repo_name,
        &*header.project_name,
        &*header.author.display_name(),
        header.author.unique_name.as_deref().unwrap_or_default(),
        strip_ref_prefix(&pr.source_branch),
        strip_ref_prefix(&pr.target_branch),
        org_label,
    ]
    .join(" ")
    .to_lowercase()
}
