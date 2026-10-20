use super::*;

#[test]
fn jwt_exp_reads_unsigned_payload() {
    assert_eq!(
        jwt_exp("eyJhbGciOiJub25lIn0.eyJleHAiOjE3OTI0MTg1OTF9."),
        Some(1_792_418_591)
    );
}

#[test]
fn jwt_exp_rejects_garbage() {
    assert_eq!(jwt_exp("not-a-jwt"), None);
    assert_eq!(jwt_exp("a.%%%.c"), None);
    assert_eq!(jwt_exp("a.eyJmb28iOjF9.c"), None);
}
