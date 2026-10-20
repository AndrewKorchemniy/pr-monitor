//! Exactly what `az` is run with. Pure, so the arguments and environment are unit-tested.

use std::{ffi::OsString, path::Path};

use prm_domain::auth::AzBroker;

use crate::{
    consts::{ADO_RESOURCE, AZ_LOGIN_TIMEOUT, AZ_TOKEN_TIMEOUT},
    process::ProcessSpec,
};

/// Quiet, machine-readable output in every `az` call.
fn base_env() -> Vec<(OsString, OsString)> {
    [
        ("AZURE_CORE_NO_COLOR", "1"),
        ("AZURE_CORE_SURVEY_MESSAGE", "no"),
        ("PYTHONIOENCODING", "utf-8"),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v.into()))
    .collect()
}

fn args(parts: &[&str]) -> Vec<OsString> {
    parts.iter().map(OsString::from).collect()
}

/// `az account get-access-token --resource <ADO> --output json --only-show-errors [--tenant <id>]`.
pub fn token_spec(az: &Path, tenant: Option<uuid::Uuid>) -> ProcessSpec {
    let mut command = args(&[
        "account",
        "get-access-token",
        "--resource",
        ADO_RESOURCE,
        "--output",
        "json",
        "--only-show-errors",
    ]);
    if let Some(tenant) = tenant {
        command.extend(args(&["--tenant", &tenant.to_string()]));
    }
    ProcessSpec {
        program: az.to_path_buf(),
        args: command,
        env: base_env(),
        cwd: None,
        timeout: AZ_TOKEN_TIMEOUT,
        hide_window: true,
    }
}

/// `az login [--tenant <id>] --allow-no-subscriptions --output none --only-show-errors`.
/// Many Azure DevOps users have no Azure subscription, hence `--allow-no-subscriptions`.
pub fn login_spec(az: &Path, tenant: Option<uuid::Uuid>, broker: AzBroker) -> ProcessSpec {
    let mut command = args(&["login"]);
    if let Some(tenant) = tenant {
        command.extend(args(&["--tenant", &tenant.to_string()]));
    }
    command.extend(args(&[
        "--allow-no-subscriptions",
        "--output",
        "none",
        "--only-show-errors",
    ]));
    let mut env = base_env();
    // The new interactive picker waits on stdin; there is none.
    env.push(("AZURE_CORE_LOGIN_EXPERIENCE_V2".into(), "no".into()));
    match broker {
        AzBroker::Default => {}
        AzBroker::Browser => {
            env.push(("AZURE_CORE_ENABLE_BROKER_ON_WINDOWS".into(), "false".into()));
        }
    }
    ProcessSpec {
        program: az.to_path_buf(),
        args: command,
        env,
        cwd: None,
        timeout: AZ_LOGIN_TIMEOUT,
        hide_window: true,
    }
}
