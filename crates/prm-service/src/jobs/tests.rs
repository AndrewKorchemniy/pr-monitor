use prm_ado::testing::{ApiCall, FakeAdoApi, FakeOrg};
use prm_auth::testing::FakeOrgAuth;
use prm_domain::testing::{org, person};

use super::*;
use crate::testing::RecordingNotifier;

fn valid_pat() -> Secret {
    let mut value = "a".repeat(76);
    value.push_str("AZDOxxxx");
    Secret::new(value)
}

fn api(me: Option<Person>) -> Arc<FakeAdoApi> {
    Arc::new(FakeAdoApi::new(
        &org("contoso"),
        FakeOrg {
            me,
            ..FakeOrg::default()
        },
    ))
}

#[tokio::test]
async fn set_pat_validates_format_before_network() {
    let check = api(Some(person(1, "Me")));
    let auth = Arc::new(FakeOrgAuth::default());
    let result = set_pat(check.clone(), auth.clone(), Secret::new("garbage")).await;
    assert_eq!(result, Err(AdoError::Auth(AuthError::PatFormat)));
    assert_eq!(check.calls(ApiCall::ConnectionData), 0);
    assert_eq!(auth.stored_pats(), Vec::<String>::new());
}

#[tokio::test]
async fn set_pat_401_reported_invalid_and_not_stored() {
    let check = api(None);
    let auth = Arc::new(FakeOrgAuth::default());
    let result = set_pat(check, auth.clone(), valid_pat()).await;
    assert_eq!(result, Err(AdoError::Auth(AuthError::PatInvalid)));
    assert_eq!(auth.stored_pats(), Vec::<String>::new());
}

#[tokio::test]
async fn set_pat_stores_after_successful_check() {
    let auth = Arc::new(FakeOrgAuth::default());
    let me = set_pat(api(Some(person(1, "Me"))), auth.clone(), valid_pat()).await;
    assert_eq!(me, Ok(person(1, "Me")));
    assert_eq!(auth.stored_pats().len(), 1);
}

#[test]
fn pat_error_maps_forbidden_to_scope() {
    let forbidden = AdoError::Forbidden {
        message: "x".into(),
        code: None,
    };
    assert_eq!(
        pat_error(forbidden),
        AdoError::Auth(AuthError::PatInsufficientScope)
    );
    assert_eq!(pat_error(AdoError::Timeout), AdoError::Timeout);
}

#[test]
fn notifier_ops_applied_in_order_failures_counted() {
    let notifier = RecordingNotifier::default();
    let ops = vec![
        NotifierOp::Retract {
            tag: "a".into(),
            group: "prs".into(),
        },
        NotifierOp::Show(prm_domain::notify::Notification::test()),
    ];
    assert_eq!(apply_notifier_ops(&notifier, &ops), 0);
    assert_eq!(notifier.ops(), ops);
    notifier.fail_next();
    assert_eq!(apply_notifier_ops(&notifier, &ops), 1);
}
