use super::*;

#[test]
fn known_vectors() {
    // Reference values for FNV-1a 64.
    assert_eq!(fnv64(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv64(b"a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(fnv64(b"foobar"), 0x8594_4171_f739_67e8);
}

#[test]
fn length_prefix_disambiguates() {
    let mut a = Fnv64::new();
    a.write_str("ab");
    a.write_str("c");
    let mut b = Fnv64::new();
    b.write_str("a");
    b.write_str("bc");
    assert_ne!(a.finish(), b.finish());
}
