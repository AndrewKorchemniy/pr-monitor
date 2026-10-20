use std::{ffi::OsString, path::PathBuf, sync::Arc, time::Duration};

use prm_domain::{
    auth::TenantInfo,
    testing::{ManualClock, org, ts},
};

use super::{locate::HostOs, *};
use crate::{
    process::ProcessOutput,
    testing::{FakeCommandRunner, failure, success},
};

const TOKEN_OK: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/az_token_ok.json"
));
const TOKEN_NO_EXPIRY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/az_token_no_expires_on.json"
));
const TENANT: uuid::Uuid = uuid::Uuid::from_u128(0xd1);

fn ctx(tenant: Option<TenantInfo>) -> AuthContext {
    AuthContext {
        org: org("contoso"),
        tenant,
    }
}

fn source(runner: FakeCommandRunner) -> (Arc<FakeCommandRunner>, AzureCliSource) {
    let runner = Arc::new(runner);
    let source = AzureCliSource::new(
        runner.clone(),
        AzLocator::fixed(PathBuf::from("az.cmd")),
        Arc::new(ManualClock::new(ts(1_000))),
    );
    (runner, source)
}

fn strings(values: &[OsString]) -> Vec<String> {
    values
        .iter()
        .map(|v| v.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn az_parse_with_expires_on() {
    let (secret, expires) = parse_token_output(TOKEN_OK.as_bytes(), ts(0)).unwrap();
    assert!(secret.expose().starts_with("eyJ"));
    assert_eq!(expires.unix(), 1_792_418_591);
}

#[test]
fn az_parse_without_expires_on_uses_jwt_exp() {
    let (_, expires) = parse_token_output(TOKEN_NO_EXPIRY.as_bytes(), ts(0)).unwrap();
    assert_eq!(expires.unix(), 1_792_418_591);
}

#[test]
fn az_parse_neither_uses_5min_fallback() {
    let (_, expires) = parse_token_output(br#"{"accessToken":"opaque"}"#, ts(1_000)).unwrap();
    assert_eq!(expires.unix(), 1_300);
}

#[test]
fn az_parse_empty_token_errors() {
    let result = parse_token_output(br#"{"accessToken":"  "}"#, ts(0));
    assert!(matches!(result, Err(AuthError::AzFailed { .. })));
}

type Check = fn(&AuthError) -> bool;

#[test]
fn az_stderr_mapping_table() {
    let cases: [(i32, &str, Check); 7] = [
        (9009, "", |e| *e == AuthError::AzNotInstalled),
        (1, "'az' is not recognized as an internal command", |e| {
            *e == AuthError::AzNotInstalled
        }),
        (1, "ERROR: AADSTS53003: Access has been blocked", |e| {
            matches!(e, AuthError::ConditionalAccessBlocked(_))
        }),
        (
            1,
            "AADSTS65001: The user or administrator has not consented",
            |e| matches!(e, AuthError::ConsentRequired { admin: true, .. }),
        ),
        (
            1,
            "AADSTS50076: Due to a configuration change ... multi-factor",
            |e| matches!(e, AuthError::InteractionRequired { .. }),
        ),
        (1, "ERROR: Please run 'az login' to setup account.", |e| {
            matches!(e, AuthError::AzNotLoggedIn { .. })
        }),
        (2, "something odd", |e| {
            matches!(
                e,
                AuthError::AzFailed {
                    exit_code: Some(2),
                    ..
                }
            )
        }),
    ];
    for (code, stderr, check) in cases {
        let out = ProcessOutput {
            status: Some(code),
            stdout: Vec::new(),
            stderr: stderr.into(),
        };
        let mapped = map_failure(&out, None);
        assert!(check(&mapped), "{stderr:?} -> {mapped:?}");
    }
}

#[tokio::test]
async fn az_exit0_with_warning_ok() {
    let runner = FakeCommandRunner::default().answer(
        "account",
        Ok(ProcessOutput {
            status: Some(0),
            stdout: TOKEN_OK.as_bytes().to_vec(),
            stderr: "WARNING: something cosmetic".into(),
        }),
    );
    let (_, source) = source(runner);
    assert!(source.acquire(&ctx(None)).await.is_ok());
}

#[test]
fn az_token_spec_hidden_systemroot_args_env() {
    let spec = token_spec(&PathBuf::from("az.cmd"), Some(TENANT));
    assert!(spec.hide_window);
    assert_eq!(spec.cwd, None, "runner defaults to %SystemRoot%");
    assert_eq!(spec.timeout, Duration::from_secs(60));
    assert_eq!(
        strings(&spec.args),
        [
            "account",
            "get-access-token",
            "--resource",
            "499b84ac-1321-427f-aa17-267ca6975798",
            "--output",
            "json",
            "--only-show-errors",
            "--tenant",
            "00000000-0000-0000-0000-0000000000d1"
        ]
    );
    assert!(
        spec.env
            .iter()
            .any(|(k, v)| k == "PYTHONIOENCODING" && v == "utf-8")
    );
}

#[test]
fn az_login_spec_allow_no_subscriptions_and_login_v2_off() {
    let spec = login_spec(&PathBuf::from("az.cmd"), None, AzBroker::Default);
    let args = strings(&spec.args);
    assert_eq!(args.first().map(String::as_str), Some("login"));
    assert!(args.contains(&"--allow-no-subscriptions".to_owned()));
    assert!(
        spec.env
            .iter()
            .any(|(k, v)| k == "AZURE_CORE_LOGIN_EXPERIENCE_V2" && v == "no")
    );
    assert!(
        !spec
            .env
            .iter()
            .any(|(k, _)| k == "AZURE_CORE_ENABLE_BROKER_ON_WINDOWS")
    );
}

#[test]
fn az_login_browser_sets_broker_env() {
    let spec = login_spec(&PathBuf::from("az.cmd"), None, AzBroker::Browser);
    assert!(
        spec.env
            .iter()
            .any(|(k, v)| k == "AZURE_CORE_ENABLE_BROKER_ON_WINDOWS" && v == "false")
    );
}

#[tokio::test]
async fn az_tenant_passed_as_guid_only() {
    let (runner, source) =
        source(FakeCommandRunner::default().answer("account", success(TOKEN_OK)));
    source
        .acquire(&ctx(Some(TenantInfo::from_id(TENANT))))
        .await
        .unwrap();
    let args = strings(&runner.specs()[0].args);
    assert_eq!(
        args.last().map(String::as_str),
        Some("00000000-0000-0000-0000-0000000000d1")
    );
}

#[tokio::test]
async fn az_msa_org_unsupported() {
    let (runner, source) = source(FakeCommandRunner::default());
    let msa = TenantInfo::from_id(uuid::Uuid::nil());
    assert_eq!(
        source.acquire(&ctx(Some(msa))).await,
        Err(AuthError::MsaOrgUnsupported)
    );
    assert_eq!(runner.specs().len(), 0);
}

#[tokio::test]
async fn az_login_cancel_returns_cancelled() {
    let runner = FakeCommandRunner::default()
        .answer("login", success(""))
        .with_delay(Duration::from_secs(60));
    let (_, source) = source(runner);
    let cancel = CancellationToken::new();
    cancel.cancel();
    let result = source.login(&ctx(None), AzBroker::Default, cancel).await;
    assert_eq!(result, Err(AuthError::Cancelled));
}

#[tokio::test]
async fn az_failure_maps_not_logged_in() {
    let runner = FakeCommandRunner::default().answer(
        "account",
        failure(1, "Please run 'az login' to setup account."),
    );
    let (_, source) = source(runner);
    assert!(matches!(
        source.acquire(&ctx(None)).await,
        Err(AuthError::AzNotLoggedIn { .. })
    ));
}

#[test]
fn az_timeout_maps_az_timeout() {
    assert_eq!(map_run_error(&RunError::Timeout), AuthError::AzTimeout);
    assert_eq!(
        map_run_error(&RunError::NotFound),
        AuthError::AzNotInstalled
    );
}

#[test]
fn az_candidate_paths_windows() {
    let env = |name: &str| match name {
        "ProgramFiles" => Some(OsString::from(r"C:\Program Files")),
        "ProgramFiles(x86)" => Some(OsString::from(r"C:\Program Files (x86)")),
        _ => None,
    };
    let paths = locate::candidate_paths(HostOs::Windows, Some(PathBuf::from("on-path")), &env);
    assert_eq!(paths.len(), 3);
    assert_eq!(paths[0], PathBuf::from("on-path"));
    let expected = PathBuf::from(r"C:\Program Files")
        .join("Microsoft SDKs")
        .join("Azure")
        .join("CLI2")
        .join("wbin")
        .join("az.cmd");
    assert_eq!(paths[1], expected);
}

#[test]
fn az_candidate_paths_unix() {
    let paths = locate::candidate_paths(HostOs::Unix, None, &|_| None);
    assert!(paths.contains(&PathBuf::from("/usr/bin/az")));
}
