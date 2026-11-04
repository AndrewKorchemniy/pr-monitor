//! The avatar pictures the UI holds as textures. Each arrives once (from the service, already
//! small and round) and is uploaded once; drawing only looks one up.

use std::collections::{HashMap, VecDeque};

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use prm_domain::avatar::AvatarKey;
use prm_service::LoadedAvatar;

/// At most this many textures are kept (about 16 KiB each, 32 MiB in all). Only a session with
/// more distinct people than this ever drops one, and that person shows initials until restart.
const MAX_AVATAR_TEXTURES: usize = 2_000;

#[derive(Default)]
pub(crate) struct AvatarStore {
    textures: HashMap<AvatarKey, TextureHandle>,
    /// Oldest first, for dropping the earliest when full.
    order: VecDeque<AvatarKey>,
}

impl AvatarStore {
    /// Uploads `loaded`; a newer picture of the same person replaces the old texture.
    pub(crate) fn insert(&mut self, ctx: &egui::Context, loaded: &LoadedAvatar) {
        let size = usize::from(loaded.image.size);
        let image = ColorImage::from_rgba_unmultiplied([size, size], &loaded.image.rgba);
        let name = format!("avatar-{}", loaded.key.stem());
        let texture = ctx.load_texture(name, image, TextureOptions::LINEAR);
        if self.textures.insert(loaded.key.clone(), texture).is_none() {
            self.order.push_back(loaded.key.clone());
        }
        while self.order.len() > MAX_AVATAR_TEXTURES {
            if let Some(oldest) = self.order.pop_front() {
                self.textures.remove(&oldest);
            }
        }
    }

    pub(crate) fn texture(&self, key: &AvatarKey) -> Option<&TextureHandle> {
        self.textures.get(key)
    }
}

#[cfg(test)]
mod tests;
