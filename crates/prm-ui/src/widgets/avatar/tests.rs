use egui_kittest::{Harness, kittest::Queryable as _};
use prm_domain::{
    Vsid,
    avatar::{AVATAR_PX, AvatarImage},
    testing::org,
};
use prm_service::LoadedAvatar;

use super::*;
use crate::theme::{self, fonts};

fn key(n: u128) -> AvatarKey {
    AvatarKey::new(org("contoso"), Vsid::from_u128(n))
}

fn picture(n: u128) -> LoadedAvatar {
    let side = usize::from(AVATAR_PX);
    let image = AvatarImage::new(AVATAR_PX, vec![200; side * side * 4]).unwrap();
    LoadedAvatar { key: key(n), image }
}

fn draw(store: AvatarStore, n: u128) -> Harness<'static, AvatarStore> {
    let mut h = Harness::builder()
        .with_context_setup(fonts::install)
        .build_ui_state(
            move |ui, store: &mut AvatarStore| {
                let theme = theme::fallback();
                let k = key(n);
                let who = AvatarFace {
                    key: &k,
                    initials: "MP",
                    name: "Maya Patel",
                };
                let response = avatar(ui, &theme, store, &who, 20.0);
                assert_eq!(response.rect.size(), vec2(20.0, 20.0));
            },
            store,
        );
    h.run();
    h
}

#[test]
fn without_a_picture_the_face_is_the_initials() {
    let store = AvatarStore::default();
    assert_eq!(face(&store, &key(1)), Face::Initials);
    let h = draw(store, 1);
    h.get_by_label("Maya Patel");
}

#[test]
fn with_a_picture_the_face_is_the_image_and_the_label_is_unchanged() {
    let ctx = egui::Context::default();
    let mut store = AvatarStore::default();
    store.insert(&ctx, &picture(1));
    assert!(matches!(face(&store, &key(1)), Face::Image(_)));
    assert_eq!(
        face(&store, &key(2)),
        Face::Initials,
        "others keep initials"
    );
    let h = draw(store, 1);
    h.get_by_label("Maya Patel");
}

#[test]
fn a_person_keeps_their_colour() {
    let tints = &theme::fallback().avatar_tints;
    let hue = |n: u128| avatar_hue(key(n).id);
    assert_eq!(hue(7), hue(7));
    assert!(tints.len() > hue(7));
    let distinct: std::collections::HashSet<usize> = (0..60).map(hue).collect();
    assert!(
        distinct.len() > 12,
        "people spread over the hues: {}",
        distinct.len()
    );
}
