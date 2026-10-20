use super::*;

#[test]
fn token_validity_with_skew() {
    let now = Timestamp::from_unix(1_000).unwrap();
    let tok = |exp: Option<i64>| AccessToken {
        header: AuthHeader::Bearer(Secret::new("t")),
        expires_at: exp.and_then(Timestamp::from_unix),
        method: AuthMethod::AzureCli,
        id: TokenId(1),
    };
    let skew = Duration::from_secs(120);
    assert!(tok(None).valid_for(now, skew));
    assert!(tok(Some(1_121)).valid_for(now, skew));
    assert!(!tok(Some(1_119)).valid_for(now, skew));
    assert!(!tok(Some(900)).valid_for(now, skew));
}

#[test]
fn actions_and_ranks_table() {
    let ca = AuthError::ConditionalAccessBlocked("AADSTS53003: blocked".into());
    assert_eq!(ca.severity(), AuthSeverity::Blocked);
    assert!(matches!(
        ca.primary_action(),
        Some(UserAction::ContactAdmin { .. })
    ));
    assert_eq!(ca.secondary_action(), Some(UserAction::SignInWithMicrosoft));
    assert_eq!(ca.code(), Some("AADSTS53003"));
    assert_eq!(ca.rank(), 1);

    let pat = AuthError::PatInvalid;
    assert_eq!(pat.primary_action(), Some(UserAction::EnterPat));
    assert_eq!(
        pat.secondary_action(),
        Some(UserAction::OpenDocs(DocLink::CreatePat))
    );

    assert_eq!(AuthError::Cancelled.primary_action(), None);
    assert_eq!(AuthError::Cancelled.severity(), AuthSeverity::Transient);
    assert_eq!(
        AuthError::AzNotInstalled.secondary_action(),
        Some(UserAction::OpenDocs(DocLink::InstallAzureCli))
    );
    assert!(
        AuthError::ConsentRequired {
            admin: true,
            detail: "x".into()
        }
        .rank()
            < AuthError::AzNotLoggedIn { tenant: None }.rank()
    );
}

#[test]
fn extract_codes() {
    assert_eq!(
        extract_code("TF400813: The user is not authorized"),
        Some("TF400813")
    );
    assert_eq!(extract_code("error VS403463 conditional"), Some("VS403463"));
    assert_eq!(extract_code("AADSTS50076: mfa"), Some("AADSTS50076"));
    assert_eq!(extract_code("STF12 x"), None);
    assert_eq!(extract_code("no code"), None);
    assert_eq!(
        AuthError::Unauthorized {
            code: Some("TF400813".into())
        }
        .code(),
        Some("TF400813")
    );
}

#[test]
fn auth_mode_unknown_falls_back() {
    #[derive(serde::Deserialize)]
    struct W {
        m: AuthMode,
    }
    let w: W = serde_json::from_str(r#"{"m":"bogus"}"#).unwrap();
    assert_eq!(w.m, AuthMode::Auto);
    let w: W = serde_json::from_str(r#"{"m":"azure_cli"}"#).unwrap();
    assert_eq!(w.m, AuthMode::AzureCli);
}
