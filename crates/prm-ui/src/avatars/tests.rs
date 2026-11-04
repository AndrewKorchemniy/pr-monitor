use prm_domain::{
    Vsid,
    avatar::{AVATAR_PX, AvatarImage},
    testing::org,
};

use super::*;

fn key(n: u128) -> AvatarKey {
    AvatarKey::new(org("contoso"), Vsid::from_u128(n))
}

fn loaded(n: u128, shade: u8) -> LoadedAvatar {
    let side = usize::from(AVATAR_PX);
    let image = AvatarImage::new(AVATAR_PX, vec![shade; side * side * 4]).unwrap();
    LoadedAvatar { key: key(n), image }
}

#[test]
fn a_loaded_picture_becomes_a_texture() {
    let ctx = egui::Context::default();
    let mut store = AvatarStore::default();
    assert!(store.texture(&key(1)).is_none());
    store.insert(&ctx, &loaded(1, 10));
    let texture = store.texture(&key(1)).unwrap();
    assert_eq!(texture.size(), [64, 64]);
}

#[test]
fn a_newer_picture_replaces_the_texture_and_keeps_its_place() {
    let ctx = egui::Context::default();
    let mut store = AvatarStore::default();
    store.insert(&ctx, &loaded(1, 10));
    let first = store.texture(&key(1)).unwrap().id();
    store.insert(&ctx, &loaded(1, 200));
    assert_eq!(store.textures.len(), 1);
    assert_eq!(store.order.len(), 1);
    assert_ne!(store.texture(&key(1)).unwrap().id(), first);
}

#[test]
fn the_oldest_textures_go_first_when_the_store_is_full() {
    let ctx = egui::Context::default();
    let mut store = AvatarStore::default();
    let total = MAX_AVATAR_TEXTURES as u128 + 3;
    for n in 0..total {
        store.insert(&ctx, &loaded(n, 1));
    }
    assert_eq!(store.textures.len(), MAX_AVATAR_TEXTURES);
    assert_eq!(store.order.len(), MAX_AVATAR_TEXTURES);
    assert!(store.texture(&key(0)).is_none());
    assert!(store.texture(&key(2)).is_none());
    assert!(store.texture(&key(3)).is_some());
    assert!(store.texture(&key(total - 1)).is_some());
}
