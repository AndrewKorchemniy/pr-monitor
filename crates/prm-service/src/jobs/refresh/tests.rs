use super::*;

/// A transient probe failure (a 429, a timeout) used to be cached as "unsupported" for a day,
/// so an opted-in org silently fell back to per-project queries.
#[test]
fn probe_answer_caches_only_definite_results() {
    let cases: [(Result<OrgWideProbe, AdoError>, Option<bool>); 8] = [
        (Ok(OrgWideProbe::Supported), Some(true)),
        (Ok(OrgWideProbe::Unsupported), Some(false)),
        (
            Err(AdoError::Forbidden {
                message: "no".into(),
                code: None,
            }),
            Some(false),
        ),
        (Err(AdoError::NotFound { message: None }), Some(false)),
        (Err(AdoError::RateLimited { retry_after: None }), None),
        (Err(AdoError::Timeout), None),
        (Err(AdoError::Network("reset".into())), None),
        (
            Err(AdoError::Server {
                status: 503,
                message: None,
                activity: None,
            }),
            None,
        ),
    ];
    for (probe, expected) in cases {
        assert_eq!(probe_answer(&probe), expected, "{probe:?}");
    }
}
