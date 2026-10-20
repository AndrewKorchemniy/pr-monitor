//! Turning a failed `az` run into the right `AuthError` (and so the right button for the user).

use std::sync::Arc;

use prm_domain::auth::AuthError;

use crate::{
    consts::EXIT_NOT_FOUND,
    process::{ProcessOutput, RunError},
};

/// How much stderr is kept for the details view.
const STDERR_TAIL_CHARS: usize = 400;

/// Maps a spawn error to an `AuthError`.
pub fn map_run_error(error: &RunError) -> AuthError {
    match error {
        RunError::NotFound => AuthError::AzNotInstalled,
        RunError::Timeout => AuthError::AzTimeout,
        RunError::Cancelled => AuthError::Cancelled,
        RunError::Io(message) | RunError::InvalidInput(message) => AuthError::AzFailed {
            stderr: message.clone(),
            exit_code: None,
        },
    }
}

/// Maps a non-zero exit to an `AuthError`; the first matching rule wins (case-insensitive).
pub fn map_failure(out: &ProcessOutput, tenant: Option<uuid::Uuid>) -> AuthError {
    let stderr = out.stderr.to_ascii_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| stderr.contains(n));
    if out.status == Some(EXIT_NOT_FOUND) || has(&["is not recognized", "command not found"]) {
        AuthError::AzNotInstalled
    } else if has(&["aadsts53003"]) {
        AuthError::ConditionalAccessBlocked(first_line(&out.stderr))
    } else if has(&["aadsts65001", "aadsts90094", "consent"]) {
        AuthError::ConsentRequired {
            admin: stderr.contains("admin"),
            detail: first_line(&out.stderr),
        }
    } else if has(&[
        "aadsts50076",
        "aadsts50079",
        "aadsts50173",
        "aadsts700082",
        "aadsts50078",
        "interaction_required",
    ]) {
        AuthError::InteractionRequired {
            reason: first_line(&out.stderr),
        }
    } else if has(&["aadsts50020", "az login", "no subscription found"]) {
        AuthError::AzNotLoggedIn { tenant }
    } else {
        AuthError::AzFailed {
            stderr: tail(&out.stderr),
            exit_code: out.status,
        }
    }
}

fn first_line(text: &str) -> Arc<str> {
    Arc::from(
        text.lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or_default(),
    )
}

fn tail(text: &str) -> Arc<str> {
    let chars: Vec<char> = text.trim().chars().collect();
    let start = chars.len().saturating_sub(STDERR_TAIL_CHARS);
    Arc::from(chars[start..].iter().collect::<String>())
}
