use prm_domain::testing::org;

use super::*;

#[test]
fn secret_key_all_for_lists_every_per_org_slot() {
    let contoso = org("contoso");

    let keys = SecretKey::all_for(&contoso);

    assert_eq!(
        keys,
        [SecretKey::pat(&contoso), SecretKey::entra_refresh(&contoso)]
    );
}
