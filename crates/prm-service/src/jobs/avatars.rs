//! Loading avatars: from the disk cache first, then (after a refresh) from the network, a
//! bounded number per run. The job only reports; the actor emits events and updates the book.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use futures_util::{StreamExt, stream};
use prm_ado::{AdoApi, AdoError};
use prm_config::{AvatarCache, CachedAvatar};
use prm_domain::{
    Timestamp, Vsid,
    auth::AuthSeverity,
    avatar::{AvatarImage, AvatarKey},
};

use super::blocking;
use crate::{
    avatars::{Slot, Want, decode, to_fetch, unseen},
    consts::{AVATAR_CONCURRENCY, AVATAR_FETCH_BUDGET},
};

/// What one run starts from.
pub(crate) struct AvatarCtx {
    pub api: Arc<dyn AdoApi>,
    pub cache: Arc<dyn AvatarCache>,
    pub wanted: Vec<Want>,
    /// The org's book as it was when the job started.
    pub slots: HashMap<Vsid, Slot>,
    pub checked: HashSet<Vsid>,
    pub now: Timestamp,
    /// Download too, not just read the disk.
    pub fetch: bool,
}

/// A picture ready to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedAvatar {
    pub key: AvatarKey,
    pub image: AvatarImage,
}

/// What one run settled.
#[derive(Default)]
pub(crate) struct AvatarOutput {
    pub loaded: Vec<LoadedAvatar>,
    pub slots: Vec<(Vsid, Slot)>,
    /// Everyone whose cache file was looked at.
    pub checked: Vec<Vsid>,
    /// The org refused avatar requests; stop asking until its settings change or the app restarts.
    pub disable: bool,
}

impl AvatarOutput {
    /// Records one person's settled slot, and their picture when there is one.
    fn settle(&mut self, key: AvatarKey, slot: Slot, image: Option<AvatarImage>) {
        self.slots.push((key.id, slot));
        self.loaded
            .extend(image.map(|image| LoadedAvatar { key, image }));
    }
}

pub(crate) async fn run(ctx: AvatarCtx) -> AvatarOutput {
    let mut out = AvatarOutput::default();
    let mut slots = ctx.slots;
    read_cache(&ctx.cache, unseen(&ctx.wanted, &ctx.checked), &mut out).await;
    slots.extend(out.slots.iter().copied());
    if ctx.fetch {
        let picks = to_fetch(&ctx.wanted, &slots, ctx.now, AVATAR_FETCH_BUDGET);
        download(&ctx.api, &ctx.cache, picks, ctx.now, &mut out).await;
    }
    out
}

/// Reads each person's cache entry; a readable one is settled without any request.
async fn read_cache<'a>(
    cache: &Arc<dyn AvatarCache>,
    wants: impl Iterator<Item = &'a Want>,
    out: &mut AvatarOutput,
) {
    let keys: Vec<AvatarKey> = wants.map(|w| w.key.clone()).collect();
    if keys.is_empty() {
        return;
    }
    out.checked.extend(keys.iter().map(|k| k.id));
    let cache = cache.clone();
    let read = blocking(move || {
        keys.into_iter()
            .filter_map(|key| {
                let entry = cache.read(&key)?;
                settle_cached(key, &entry)
            })
            .collect::<Vec<_>>()
    })
    .await
    .unwrap_or_default();
    for (key, slot, image) in read {
        out.settle(key, slot, image);
    }
}

/// A cache entry as a slot and (unless it is the "no picture" marker) its picture. `None` when
/// the file is unreadable as a picture, so it is fetched again.
fn settle_cached(
    key: AvatarKey,
    entry: &CachedAvatar,
) -> Option<(AvatarKey, Slot, Option<AvatarImage>)> {
    if entry.png.is_empty() {
        return Some((key, Slot::Absent(entry.stored_at), None));
    }
    let image = decode::from_stored(&entry.png)?;
    Some((key, Slot::Loaded(entry.stored_at), Some(image)))
}

/// How one download ended.
enum Fetched {
    Settled {
        key: AvatarKey,
        slot: Slot,
        image: Option<AvatarImage>,
    },
    /// 401/403: avatars are off for this org.
    Refused,
    /// A passing failure: stop this run and try again next refresh.
    Retry,
}

async fn download(
    api: &Arc<dyn AdoApi>,
    cache: &Arc<dyn AvatarCache>,
    picks: Vec<(AvatarKey, Arc<str>)>,
    now: Timestamp,
    out: &mut AvatarOutput,
) {
    let mut results = stream::iter(picks)
        .map(|(key, url)| fetch_one(api.clone(), cache.clone(), (key, url), now))
        .buffered(AVATAR_CONCURRENCY);
    while let Some(fetched) = results.next().await {
        match fetched {
            Fetched::Settled { key, slot, image } => out.settle(key, slot, image),
            Fetched::Refused => {
                out.disable = true;
                return;
            }
            Fetched::Retry => return,
        }
    }
}

async fn fetch_one(
    api: Arc<dyn AdoApi>,
    cache: Arc<dyn AvatarCache>,
    (key, url): (AvatarKey, Arc<str>),
    now: Timestamp,
) -> Fetched {
    match api.avatar(&url).await {
        Ok(bytes) => settle_download(cache, key, bytes, now).await,
        Err(AdoError::NotFound { .. }) => marker(&cache, key, now).await,
        // Nothing usable at that address, and asking again soon will not change it.
        Err(AdoError::ForeignUrl | AdoError::Http { .. } | AdoError::Decode(_)) => absent(key, now),
        Err(AdoError::Auth(error)) if error.severity() == AuthSeverity::Transient => Fetched::Retry,
        Err(AdoError::Auth(_) | AdoError::Forbidden { .. }) => Fetched::Refused,
        Err(
            AdoError::RateLimited { .. }
            | AdoError::Server { .. }
            | AdoError::ProxyAuthRequired
            | AdoError::Network(_)
            | AdoError::Timeout
            | AdoError::Cancelled,
        ) => Fetched::Retry,
    }
}

/// "No picture" as of `now`, kept in memory only.
fn absent(key: AvatarKey, now: Timestamp) -> Fetched {
    Fetched::Settled {
        key,
        slot: Slot::Absent(now),
        image: None,
    }
}

/// "No picture" as of `now`, also written to disk so a restart does not ask again.
async fn marker(cache: &Arc<dyn AvatarCache>, key: AvatarKey, now: Timestamp) -> Fetched {
    let (cache, written) = (cache.clone(), key.clone());
    blocking(move || store(cache.as_ref(), &written, Vec::new(), now)).await;
    absent(key, now)
}

/// Decodes and caches a download. A picture that is not a usable PNG counts as no picture.
async fn settle_download(
    cache: Arc<dyn AvatarCache>,
    key: AvatarKey,
    bytes: impl AsRef<[u8]> + Send + 'static,
    now: Timestamp,
) -> Fetched {
    let work = {
        let (cache, key) = (cache.clone(), key.clone());
        blocking(move || {
            let image = decode::from_source(bytes.as_ref())?;
            store(cache.as_ref(), &key, decode::to_png(&image)?, now);
            Some(image)
        })
    };
    match work.await.flatten() {
        Some(image) => Fetched::Settled {
            key,
            slot: Slot::Loaded(now),
            image: Some(image),
        },
        None => marker(&cache, key, now).await,
    }
}

fn store(cache: &dyn AvatarCache, key: &AvatarKey, png: Vec<u8>, now: Timestamp) {
    let entry = CachedAvatar {
        png,
        stored_at: now,
    };
    if let Err(error) = cache.write(key, &entry) {
        tracing::warn!(%error, "avatar not cached");
    }
}
