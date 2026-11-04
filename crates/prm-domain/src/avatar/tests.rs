use super::*;
use crate::testing::org;

#[test]
fn avatar_stem_is_golden() {
    let key = AvatarKey::new(org("contoso"), Vsid::from_u128(7));
    assert_eq!(key.stem(), GOLDEN);
}

#[test]
fn avatar_stem_depends_on_org_and_person() {
    let a = AvatarKey::new(org("contoso"), Vsid::from_u128(7));
    let b = AvatarKey::new(org("fabrikam"), Vsid::from_u128(7));
    let c = AvatarKey::new(org("contoso"), Vsid::from_u128(8));
    assert_ne!(a.stem(), b.stem());
    assert_ne!(a.stem(), c.stem());
    assert_eq!(a.stem().len(), 16);
}

#[test]
fn avatar_image_checks_its_length() {
    assert!(AvatarImage::new(2, vec![0; 16]).is_some());
    assert!(AvatarImage::new(2, vec![0; 15]).is_none());
    assert!(AvatarImage::new(0, Vec::new()).is_none());
}

const GOLDEN: &str = "0c3fcacf8bdfbaf8";
