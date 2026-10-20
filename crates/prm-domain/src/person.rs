//! People: configured, harvested from PR data, or found in the directory.

use std::{collections::HashMap, sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};

use crate::{ids::Vsid, time::Timestamp};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Person {
    /// `IdentityRef.id` (VSID / storage key). The only id matched against PR data.
    pub id: Vsid,
    #[serde(default)]
    pub name: Arc<str>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unique_name: Option<Arc<str>>,
    /// `isContainer`: a team or group.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub group: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptor: Option<Arc<str>>,
}

impl Person {
    pub fn new(id: Vsid, name: impl Into<Arc<str>>) -> Self {
        Self {
            id,
            name: name.into(),
            unique_name: None,
            group: false,
            descriptor: None,
        }
    }

    /// Name for display; falls back to the unique name, then the id.
    pub fn display_name(&self) -> Arc<str> {
        if non_blank(&self.name) {
            self.name.clone()
        } else if let Some(unique) = self.unique_name.as_ref().filter(|u| non_blank(u)) {
            unique.clone()
        } else {
            Arc::from(self.id.to_string())
        }
    }

    /// Copies the non-blank fields of `newer` over this person's (same id).
    fn update_from(&mut self, newer: Person) {
        if non_blank(&newer.name) {
            self.name = newer.name;
        }
        if newer.unique_name.as_deref().is_some_and(non_blank) {
            self.unique_name = newer.unique_name;
        }
        if newer.descriptor.is_some() {
            self.descriptor = newer.descriptor;
        }
        self.group |= newer.group;
    }

    pub fn to_candidate(&self, source: CandidateSource) -> PersonCandidate {
        PersonCandidate {
            id: Some(self.id),
            descriptor: self.descriptor.clone(),
            name: self.name.clone(),
            unique_name: self.unique_name.clone(),
            group: self.group,
            source,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateSource {
    Configured,
    Harvested,
    /// Graph subject query.
    Directory,
    /// IMS identities exact match.
    Exact,
}

/// A search result. Directory results may lack an id until resolved via storage keys.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonCandidate {
    pub id: Option<Vsid>,
    pub descriptor: Option<Arc<str>>,
    pub name: Arc<str>,
    pub unique_name: Option<Arc<str>>,
    pub group: bool,
    pub source: CandidateSource,
}

impl PersonCandidate {
    /// A `Person` if the candidate already carries its VSID.
    pub fn to_person(&self) -> Option<Person> {
        self.id.map(|id| Person {
            id,
            name: self.name.clone(),
            unique_name: self.unique_name.clone(),
            group: self.group,
            descriptor: self.descriptor.clone(),
        })
    }
}

#[derive(Clone, Debug)]
struct Entry {
    person: Person,
    last_seen: Timestamp,
    /// How often this person appeared; ranks search results.
    hits: u32,
}

impl Entry {
    fn first_seen(person: Person, now: Timestamp) -> Self {
        Self {
            person,
            last_seen: now,
            hits: 1,
        }
    }

    fn seen_again(&mut self, person: Person, now: Timestamp) {
        self.person.update_from(person);
        self.last_seen = self.last_seen.max(now);
        self.hits = self.hits.saturating_add(1);
    }
}

/// Whether a name carries any visible text.
fn non_blank(s: &str) -> bool {
    !s.trim().is_empty()
}

/// People seen in PR data (authors, reviewers, comment authors), for the instant local picker.
#[derive(Clone, Debug, Default)]
pub struct PeopleCache {
    entries: HashMap<Vsid, Entry>,
}

impl PeopleCache {
    /// `people` is in most-recent-first order (as written by [`Self::to_persisted`]); that
    /// order is kept by stamping each person one second older than the previous.
    pub fn from_persisted(people: Vec<Person>, now: Timestamp) -> Self {
        let mut cache = Self::default();
        for (age_secs, person) in (0u64..).zip(people) {
            cache.harvest([person], now.saturating_sub(Duration::from_secs(age_secs)));
        }
        cache
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// A newer non-empty name, unique name or descriptor wins.
    pub fn harvest<I: IntoIterator<Item = Person>>(&mut self, people: I, now: Timestamp) {
        for person in people.into_iter().filter(|p| !p.id.is_nil()) {
            if let Some(entry) = self.entries.get_mut(&person.id) {
                entry.seen_again(person, now);
            } else {
                self.entries
                    .insert(person.id, Entry::first_seen(person, now));
            }
        }
    }

    pub fn get(&self, id: &Vsid) -> Option<&Person> {
        self.entries.get(id).map(|e| &e.person)
    }

    /// Case-insensitive. Rank: word-prefix of name/unique name (0) before substring (1);
    /// then hits descending, then name ascending.
    pub fn search(&self, query: &str, limit: usize) -> Vec<PersonCandidate> {
        let q = query.trim().to_lowercase();
        if q.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut scored: Vec<(u8, &Entry)> = self
            .entries
            .values()
            .filter_map(|e| {
                let name = e.person.name.to_lowercase();
                let unique = e
                    .person
                    .unique_name
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase();
                if word_prefix(&name, &q) || word_prefix(&unique, &q) {
                    Some((0, e))
                } else if name.contains(&q) || unique.contains(&q) {
                    Some((1, e))
                } else {
                    None
                }
            })
            .collect();
        scored.sort_by(|(ra, a), (rb, b)| {
            ra.cmp(rb)
                .then(b.hits.cmp(&a.hits))
                .then_with(|| {
                    a.person
                        .name
                        .to_lowercase()
                        .cmp(&b.person.name.to_lowercase())
                })
                .then(a.person.id.cmp(&b.person.id))
        });
        scored
            .into_iter()
            .take(limit)
            .map(|(_, e)| e.person.to_candidate(CandidateSource::Harvested))
            .collect()
    }

    /// Keeps the `cap` most recently seen people.
    pub fn evict_to(&mut self, cap: usize) {
        if self.entries.len() <= cap {
            return;
        }
        let mut order: Vec<(Timestamp, Vsid)> = self
            .entries
            .iter()
            .map(|(id, e)| (e.last_seen, *id))
            .collect();
        order.sort_by(|a, b| b.cmp(a));
        for (_, id) in order.into_iter().skip(cap) {
            self.entries.remove(&id);
        }
    }

    /// Most recently seen first, at most `cap`.
    pub fn to_persisted(&self, cap: usize) -> Vec<Person> {
        let mut all: Vec<&Entry> = self.entries.values().collect();
        all.sort_by(|a, b| {
            b.last_seen
                .cmp(&a.last_seen)
                .then(a.person.id.cmp(&b.person.id))
        });
        all.into_iter()
            .take(cap)
            .map(|e| e.person.clone())
            .collect()
    }
}

fn word_prefix(haystack: &str, q: &str) -> bool {
    haystack.starts_with(q)
        || haystack
            .char_indices()
            .filter(|(_, c)| matches!(c, ' ' | '.' | '-' | '_' | '@' | '\\' | '[' | '('))
            .any(|(i, c)| haystack[i + c.len_utf8()..].starts_with(q))
}

/// Dedupe remote against local by id, then descriptor, then unique name (case-insensitive).
/// Local entries win and come first.
pub fn merge_candidates(
    local: Vec<PersonCandidate>,
    remote: Vec<PersonCandidate>,
    limit: usize,
) -> Vec<PersonCandidate> {
    let mut out: Vec<PersonCandidate> = Vec::with_capacity(local.len() + remote.len());
    for c in local.into_iter().chain(remote) {
        let dup = out.iter().any(|o| {
            (o.id.is_some() && o.id == c.id)
                || (o.descriptor.is_some() && o.descriptor == c.descriptor)
                || match (&o.unique_name, &c.unique_name) {
                    (Some(a), Some(b)) => non_blank(a) && a.eq_ignore_ascii_case(b),
                    _ => false,
                }
        });
        if !dup {
            out.push(c);
        }
        if out.len() >= limit {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests;
