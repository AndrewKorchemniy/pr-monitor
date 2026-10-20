use super::*;

#[test]
fn backoff_grows_and_caps_with_jitter() {
    let policy = RetryPolicy::default();
    let mut rng = fastrand::Rng::with_seed(7);
    for (attempt, nominal_ms) in [(0u32, 500u64), (1, 1_000), (2, 2_000), (3, 4_000)] {
        let delay = backoff_delay(&policy, attempt, &mut rng).as_millis();
        let nominal = u128::from(nominal_ms);
        assert!(
            delay >= nominal * 8 / 10 && delay <= nominal * 12 / 10,
            "attempt {attempt}: {delay}ms"
        );
    }
    assert!(backoff_delay(&policy, 20, &mut rng) <= policy.max_backoff);
}
