//! Avatar identity and pixels: what names a picture in memory and on disk, and the decoded
//! form the UI turns into a texture. No IO.

use std::sync::Arc;

use crate::{
    hash::Fnv64,
    ids::{OrgId, Vsid},
};

/// Side length, in pixels, of every stored avatar. A 20 pt avatar at 2x scale needs 40 px;
/// 64 leaves headroom and costs 16 KiB.
pub const AVATAR_PX: u16 = 64;

/// Whose picture it is. Deliberately omits the address: a re-issued address for the same
/// person keeps its cache.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AvatarKey {
    pub org: OrgId,
    pub id: Vsid,
}

impl AvatarKey {
    pub const fn new(org: OrgId, id: Vsid) -> Self {
        Self { org, id }
    }

    /// The file stem under the avatar folder: 16 lowercase hex digits. Persisted, so the
    /// recipe (FNV-1a of `org`, `/`, `vsid`) must never change.
    pub fn stem(&self) -> String {
        let mut hash = Fnv64::new();
        hash.write_bytes(self.org.to_string().as_bytes());
        hash.write_u8(b'/');
        hash.write_bytes(self.id.as_bytes());
        format!("{:016x}", hash.finish())
    }
}

/// A decoded avatar: square, straight-alpha RGBA8, already masked to a circle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AvatarImage {
    pub size: u16,
    pub rgba: Arc<[u8]>,
}

impl AvatarImage {
    /// `None` unless `rgba` is exactly `size` x `size` pixels.
    pub fn new(size: u16, rgba: impl Into<Arc<[u8]>>) -> Option<Self> {
        let rgba = rgba.into();
        let side = usize::from(size);
        (side > 0 && rgba.len() == side * side * 4).then_some(Self { size, rgba })
    }
}

#[cfg(test)]
mod tests;
