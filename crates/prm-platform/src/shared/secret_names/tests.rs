use prm_domain::testing::org;

use super::*;

#[test]
fn dpapi_file_name_hex() {
    let name = dpapi_file_name(&SecretKey::pat(&org("ab")));
    assert_eq!(name, "61622f706174.bin");
}

#[test]
fn is_dpapi_file_name_table() {
    let ours = dpapi_file_name(&SecretKey::entra_refresh(&org("contoso")));
    let cases = [
        (ours.as_str(), true),
        ("61622f706174.bin", true),
        (".bin", false),
        ("abc.bin", false),
        ("61622F706174.bin", false),
        ("notes.bin", false),
        ("61622f706174.txt", false),
        ("61622f706174", false),
    ];
    for (name, expected) in cases {
        assert_eq!(is_dpapi_file_name(name), expected, "{name}");
    }
}

type Check = fn(&SecretStoreError) -> bool;

#[test]
fn keyring_error_mapping_table() {
    use keyring_core::Error;
    let platform = || Box::new(std::io::Error::other("x")) as _;
    let cases: [(Error, Check); 6] = [
        (Error::NoStorageAccess(platform()), |e| {
            matches!(e, SecretStoreError::Unavailable(_))
        }),
        (Error::PlatformFailure(platform()), |e| {
            matches!(e, SecretStoreError::Unavailable(_))
        }),
        (Error::NoDefaultStore, |e| {
            matches!(e, SecretStoreError::Unavailable(_))
        }),
        (Error::TooLong("secret".into(), 2_560), |e| {
            *e == SecretStoreError::TooLong(9_000)
        }),
        (Error::BadEncoding(vec![0xff]), |e| {
            matches!(e, SecretStoreError::Corrupt(_))
        }),
        (Error::Invalid("user".into(), "empty".into()), |e| {
            matches!(e, SecretStoreError::Other(_))
        }),
    ];
    for (error, check) in cases {
        let mapped = map_keyring_error(&error, 9_000);
        assert!(check(&mapped), "{error:?} -> {mapped:?}");
    }
}
