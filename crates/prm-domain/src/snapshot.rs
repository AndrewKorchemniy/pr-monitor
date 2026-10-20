//! Classified snapshots of an org's tracked PRs, and fingerprints that drive unread dots.
//! Depends on `classify`; `diff` and `tombstone` depend on this module, never the reverse.

use std::{collections::BTreeMap, fmt, sync::Arc};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    classify::{Classification, ClassifyConfig, classify},
    hash::Fnv64,
    ids::{PrKey, Vsid},
    model::PrRecord,
    person::Person,
};

/// The parts of a PR that events and toasts describe.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrHeader {
    pub key: PrKey,
    pub title: Arc<str>,
    pub repo_name: Arc<str>,
    pub project_name: Arc<str>,
    pub author: Person,
    pub web_url: Arc<str>,
    #[serde(default)]
    pub draft: bool,
}

impl PrHeader {
    pub fn from_record(rec: &PrRecord) -> Self {
        let pr = &rec.pr;
        Self {
            key: pr.key.clone(),
            title: pr.title.clone(),
            repo_name: pr.repo_name.clone(),
            project_name: pr.project_name.clone(),
            author: pr.author.clone(),
            web_url: pr.web_url.clone(),
            draft: pr.draft,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassifiedPr {
    pub header: PrHeader,
    pub classification: Classification,
    pub head_commit: Option<Arc<str>>,
    pub fingerprint: Fingerprint,
    /// Carried over from a previous fetch after a failure; never produces events.
    pub stale: bool,
    /// Reviewers, comment authors and the PR author by id, so events can name people.
    pub people: BTreeMap<Vsid, Person>,
}

impl ClassifiedPr {
    /// The person with this id as seen on the PR; an id-only person if unknown.
    pub fn person(&self, id: Vsid) -> Person {
        self.people
            .get(&id)
            .cloned()
            .unwrap_or_else(|| Person::new(id, ""))
    }
}

pub type OrgSnapshot = BTreeMap<PrKey, ClassifiedPr>;

/// What the user has seen of a PR. Persisted as 16 lowercase hex digits.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Fingerprint(pub u64);

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

impl Serialize for Fingerprint {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Fingerprint {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        u64::from_str_radix(&s, 16)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

/// The fingerprint the user last saw, per PR.
pub type SeenMap = BTreeMap<PrKey, Fingerprint>;

pub fn classify_record(rec: &PrRecord, cfg: &ClassifyConfig, stale: bool) -> ClassifiedPr {
    let header = PrHeader::from_record(rec);
    let classification = classify(&rec.pr, &rec.threads, cfg);
    let head_commit = rec.pr.head_commit.clone();
    let fingerprint = fingerprint(&header, &classification, head_commit.as_deref());
    ClassifiedPr {
        header,
        classification,
        head_commit,
        fingerprint,
        stale,
        people: people_of(rec),
    }
}

fn people_of(rec: &PrRecord) -> BTreeMap<Vsid, Person> {
    let reviewers = rec.pr.reviewers.iter().map(|r| &r.who);
    let commenters = rec
        .threads
        .iter()
        .flat_map(|t| &t.comments)
        .filter_map(|c| c.author.as_ref());
    let mut people = BTreeMap::new();
    for person in reviewers.chain(commenters).chain([&rec.pr.author]) {
        people.entry(person.id).or_insert_with(|| person.clone());
    }
    people
}

/// Persisted (via [`SeenMap`]): changing the inputs or their order resets unread state for
/// every user. Covers everything a user would want to look at again: status, thread counts,
/// title, draft flag, counted votes, comment keys and the head commit.
fn fingerprint(
    header: &PrHeader,
    class: &Classification,
    head_commit: Option<&str>,
) -> Fingerprint {
    let mut h = Fnv64::new();
    h.write_u8(class.status.stable_code());
    h.write_u32(class.active_threads);
    h.write_u32(class.total_threads);
    h.write_str(&header.title);
    h.write_bool(header.draft);
    h.write_u64(class.counted_votes.len() as u64);
    for (who, vote) in &class.counted_votes {
        h.write_bytes(who.as_bytes());
        h.write_i16(vote.0);
    }
    h.write_u64(class.comments.len() as u64);
    for (thread, comment) in class.comments.keys() {
        h.write_i32(thread.0);
        h.write_i32(comment.0);
    }
    h.write_opt_str(head_commit);
    Fingerprint(h.finish())
}

#[cfg(test)]
mod tests;
