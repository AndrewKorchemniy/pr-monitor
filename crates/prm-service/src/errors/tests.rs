use std::time::Duration;

use super::*;

#[test]
fn ui_error_from_ado_error_table() {
    let cases: [(AdoError, &str, Option<UserAction>, bool); 6] = [
        (
            AdoError::Forbidden {
                message: "no".into(),
                code: Some("TF401019".into()),
            },
            text::FORBIDDEN,
            Some(UserAction::OpenSettings(SettingsSection::Scope)),
            false,
        ),
        (
            AdoError::Network("dns".into()),
            text::UNREACHABLE,
            Some(UserAction::Retry),
            true,
        ),
        (
            AdoError::Timeout,
            text::TIMED_OUT,
            Some(UserAction::Retry),
            true,
        ),
        (
            AdoError::RateLimited {
                retry_after: Some(Duration::from_secs(5)),
            },
            text::RATE_LIMITED,
            None,
            true,
        ),
        (
            AdoError::Decode("bad json".into()),
            text::UNEXPECTED_RESPONSE,
            None,
            false,
        ),
        (
            AdoError::ProxyAuthRequired,
            text::PROXY_AUTH,
            Some(UserAction::OpenDocs(DocLink::Troubleshooting)),
            false,
        ),
    ];
    for (error, title, action, transient) in cases {
        let ui = UiError::from(&error);
        assert_eq!(ui.title.as_ref(), title, "{error:?}");
        assert_eq!(ui.action, action, "{error:?}");
        assert_eq!(ui.transient, transient, "{error:?}");
    }
}

#[test]
fn ui_error_keeps_code() {
    let error = AdoError::Forbidden {
        message: "no".into(),
        code: Some("TF401019".into()),
    };
    assert_eq!(UiError::from(&error).code.as_deref(), Some("TF401019"));
}

#[test]
fn ui_error_from_auth_passes_actions_through() {
    let auth = AuthError::ConditionalAccessBlocked("AADSTS53003: blocked".into());
    let ui = UiError::from(&AdoError::Auth(auth.clone()));
    assert_eq!(ui.title.as_ref(), auth.to_string());
    assert_eq!(ui.action, auth.primary_action());
    assert_eq!(ui.secondary, auth.secondary_action());
    assert!(!ui.transient);
}

#[test]
fn ui_error_proxy_offers_settings_second() {
    let ui = UiError::from(&AdoError::ProxyAuthRequired);
    assert_eq!(
        ui.secondary,
        Some(UserAction::OpenSettings(SettingsSection::General))
    );
}
