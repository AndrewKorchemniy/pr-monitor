//! Starting avatar jobs and applying their results.

use std::sync::Arc;

use prm_domain::OrgId;

use super::Service;
use crate::{
    avatars::{self, to_fetch, unseen},
    consts::{AVATAR_CACHE_KEEP, AVATAR_FETCH_BUDGET, HEADROOM_LOW},
    event::AppEvent,
    jobs::{
        AvatarOutput, JobKind, JobOutput, JobTag,
        avatars::{self as job, AvatarCtx},
        blocking,
    },
};

impl Service {
    /// At start: cached faces for every org (they need no network, so they show on the first
    /// paint), and old cache entries deleted.
    pub(super) fn start_avatar_startup(&mut self) {
        for idx in 0..self.orgs.len() {
            self.start_avatars(idx, false);
        }
        let cache = self.deps.avatar_cache.clone();
        let before = self.deps.clock.now().saturating_sub(AVATAR_CACHE_KEEP);
        self.spawn(JobTag::global(JobKind::Avatars), async move {
            blocking(move || cache.prune(before)).await;
            JobOutput::AvatarsPruned
        });
    }

    /// Starts a job for the org unless one runs or there is nothing to do. `fetch` also downloads
    /// (after a refresh), unless avatars are off for the org or its rate limit is nearly spent.
    pub(super) fn start_avatars(&mut self, idx: usize, fetch: bool) {
        let now = self.deps.clock.now();
        let org = &mut self.orgs[idx];
        let book = &mut org.link.avatars;
        let low_headroom = org.live.rate.headroom().is_some_and(|h| h < HEADROOM_LOW);
        let fetch = fetch && !book.disabled && !low_headroom;
        let wanted = avatars::wanted(&org.data.id, org.data.records.values());
        let unread = unseen(&wanted, &book.checked).next().is_some();
        let endpoint = org.link.api.endpoint();
        let picks = to_fetch(&wanted, &book.slots, now, AVATAR_FETCH_BUDGET, |href| {
            endpoint.allows_avatar(href)
        });
        let due = fetch && !picks.is_empty();
        if book.busy || !(unread || due) {
            return;
        }
        book.busy = true;
        let ctx = AvatarCtx {
            api: org.link.api.clone(),
            cache: self.deps.avatar_cache.clone(),
            slots: book.slots.clone(),
            checked: book.checked.clone(),
            wanted,
            now,
            fetch,
        };
        let (id, generation) = (org.data.id.clone(), org.link.generation);
        self.spawn(JobTag::org(&id, JobKind::Avatars), async move {
            let output = job::run(ctx).await;
            JobOutput::Avatars {
                org: id,
                generation,
                output,
            }
        });
    }

    pub(super) fn on_avatars(&mut self, org: &OrgId, generation: u64, output: AvatarOutput) {
        let Some(idx) = self.index(org) else {
            return;
        };
        let link = &mut self.orgs[idx].link;
        link.avatars.busy = false;
        let AvatarOutput {
            loaded,
            slots,
            checked,
            disable,
        } = output;
        // why: a picture is the person's whatever credentials fetched it; only a refusal
        // belongs to the connection that was replaced.
        link.avatars.slots.extend(slots);
        link.avatars.checked.extend(checked);
        if disable && link.generation == generation && !link.avatars.disabled {
            link.avatars.disabled = true;
            tracing::info!(org = %org, "the org refused avatar requests; avatars off until its settings change or restart");
        }
        if !loaded.is_empty() {
            self.deps.events.emit(AppEvent::Avatars(Arc::from(loaded)));
        }
    }
}
