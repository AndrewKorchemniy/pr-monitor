//! What the service knows about each person's avatar, and which ones to fetch next. Pure.

pub(crate) mod decode;

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use prm_domain::{OrgId, Timestamp, Vsid, avatar::AvatarKey, model::PrRecord};

use crate::consts::{AVATAR_ABSENT_TTL, AVATAR_TTL};

/// Where a person's avatar stands, and since when.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    /// A picture was loaded (from disk or the network) at this time.
    Loaded(Timestamp),
    /// There is no usable picture, as of this time.
    Absent(Timestamp),
}

impl Slot {
    const fn at(self) -> Timestamp {
        match self {
            Self::Loaded(at) | Self::Absent(at) => at,
        }
    }

    const fn ttl(self) -> Duration {
        match self {
            Self::Loaded(_) => AVATAR_TTL,
            Self::Absent(_) => AVATAR_ABSENT_TTL,
        }
    }

    /// A time in the future (a clock or file-time change) is never fresh.
    fn fresh(self, now: Timestamp) -> bool {
        self.at() <= now && now.since(self.at()) < self.ttl()
    }
}

/// One org's avatars for this session: what has settled, whether fetching is off, and whether a
/// job is running.
#[derive(Debug, Default)]
pub(crate) struct AvatarBook {
    pub slots: HashMap<Vsid, Slot>,
    /// People whose cache file has been looked at this session, found or not.
    pub checked: HashSet<Vsid>,
    /// The org refused avatar requests (401/403): no more attempts until its settings change or
    /// the app restarts.
    pub disabled: bool,
    /// A job for this org is running.
    pub busy: bool,
}

/// Someone shown in the org's PRs, and the address their picture was advertised at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Want {
    pub key: AvatarKey,
    pub url: Option<Arc<str>>,
}

/// Everyone in the records, authors before reviewers, each once, in record order. A person
/// seen both with and without an address keeps the address.
pub(crate) fn wanted<'a>(
    org: &OrgId,
    records: impl IntoIterator<Item = &'a PrRecord>,
) -> Vec<Want> {
    let records: Vec<&PrRecord> = records.into_iter().collect();
    let authors = records.iter().map(|r| &r.pr.author);
    let reviewers = records
        .iter()
        .flat_map(|r| r.pr.reviewers.iter().map(|v| &v.who));
    let mut order: Vec<Want> = Vec::new();
    let mut index: HashMap<Vsid, usize> = HashMap::new();
    for person in authors.chain(reviewers) {
        if let Some(&at) = index.get(&person.id) {
            if order[at].url.is_none() {
                order[at].url.clone_from(&person.avatar);
            }
        } else {
            index.insert(person.id, order.len());
            order.push(Want {
                key: AvatarKey::new(org.clone(), person.id),
                url: person.avatar.clone(),
            });
        }
    }
    order
}

/// Who still needs the disk looked at: nobody has looked yet this session.
pub(crate) fn unseen<'a>(
    wanted: &'a [Want],
    checked: &'a HashSet<Vsid>,
) -> impl Iterator<Item = &'a Want> {
    wanted.iter().filter(|w| !checked.contains(&w.key.id))
}

/// Who to download next, at most `budget`: people with no picture yet first, then those whose
/// entry expired, each in `wanted` order. People without an address are never picked; an address
/// outside the org still takes a slot, and is then refused without a request.
pub(crate) fn to_fetch(
    wanted: &[Want],
    slots: &HashMap<Vsid, Slot>,
    now: Timestamp,
    budget: usize,
) -> Vec<(AvatarKey, Arc<str>)> {
    let with_url = || {
        wanted
            .iter()
            .filter_map(|w| Some((&w.key, w.url.as_ref()?)))
    };
    let fresh = |key: &AvatarKey| slots.get(&key.id).is_some_and(|s| s.fresh(now));
    let new = with_url().filter(|(key, _)| !slots.contains_key(&key.id));
    let expired = with_url().filter(|(key, _)| slots.contains_key(&key.id) && !fresh(key));
    new.chain(expired)
        .take(budget)
        .map(|(key, url)| (key.clone(), url.clone()))
        .collect()
}

#[cfg(test)]
mod tests;
