//! Auth vocabulary shared by prm-ado, prm-auth, prm-service and the UI.
//!
//! Three related enums, from most to least persistent:
//! - [`AuthMode`]: what the user configured for an org (`auto` tries every method).
//! - [`AuthMethod`]: the concrete method that produced a token.
//! - [`SignInMethod`]: a one-off sign-in action the user triggered.
//!
//! The `#[error]` texts on [`AuthError`] are the user-facing copy (one voice, one place), and
//! [`AuthError::profile`] describes each variant's severity and actions in one match.

use std::{sync::Arc, time::Duration};

use serde::{Deserialize, Serialize};

use crate::{
    app::{APP_DISPLAY_NAME, DocLink},
    secret::{Secret, TokenId},
    time::Timestamp,
};

/// How long a background `az account get-access-token` may run.
pub const AZ_TIMEOUT: Duration = Duration::from_secs(60);
/// How long an interactive sign-in (browser, `az login`) may take.
pub const INTERACTIVE_SIGN_IN_TIMEOUT: Duration = Duration::from_mins(5);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    AzureCli,
    Pat,
    Entra,
}

impl AuthMethod {
    pub const fn label(self) -> &'static str {
        match self {
            Self::AzureCli => "Azure CLI",
            Self::Pat => "Personal access token",
            Self::Entra => "Your organization's app",
        }
    }
}

/// Config value: which method(s) an org may use. Unknown values fall back to `Auto`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    AzureCli,
    Pat,
    Entra,
    /// Last because `#[serde(other)]` must be the final variant.
    #[default]
    #[serde(other)]
    Auto,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AzBroker {
    /// Azure CLI's choice (WAM on Windows with az >= 2.61).
    #[default]
    Default,
    /// Force the system browser (`AZURE_CORE_ENABLE_BROKER_ON_WINDOWS=false`).
    Browser,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignInMethod {
    /// Re-check the configured method without any UI.
    Silent,
    AzLogin {
        broker: AzBroker,
    },
    Entra,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantInfo {
    pub id: uuid::Uuid,
    /// The all-zero GUID: an org backed by Microsoft accounts.
    pub msa: bool,
}

impl TenantInfo {
    pub fn from_id(id: uuid::Uuid) -> Self {
        Self {
            id,
            msa: id.is_nil(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthHeader {
    Bearer(Secret),
    /// `base64(":" + pat)`, pre-encoded.
    Basic(Secret),
}

impl AuthHeader {
    /// The secret inside, for its log-safe id.
    pub fn secret(&self) -> &Secret {
        match self {
            Self::Bearer(s) | Self::Basic(s) => s,
        }
    }

    /// The full `Authorization` header value.
    pub fn header_value(&self) -> String {
        match self {
            Self::Bearer(s) => format!("Bearer {}", s.expose()),
            Self::Basic(s) => format!("Basic {}", s.expose()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessToken {
    pub header: AuthHeader,
    pub expires_at: Option<Timestamp>,
    pub method: AuthMethod,
    pub id: TokenId,
}

impl AccessToken {
    /// No expiry means valid. Otherwise more than `skew` must remain.
    pub fn valid_for(&self, now: Timestamp, skew: Duration) -> bool {
        let Some(expires_at) = self.expires_at else {
            return true;
        };
        expires_at > now && expires_at.since(now) > skew
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    #[error("Azure CLI isn't installed.")]
    AzNotInstalled,
    #[error("Azure CLI isn't signed in to this organization's tenant.")]
    AzNotLoggedIn { tenant: Option<uuid::Uuid> },
    #[error("Azure CLI couldn't get a token.")]
    AzFailed {
        stderr: Arc<str>,
        exit_code: Option<i32>,
    },
    #[error("Azure CLI didn't respond within {} seconds.", AZ_TIMEOUT.as_secs())]
    AzTimeout,
    #[error("Your sign-in needs another step, like multi-factor authentication.")]
    InteractionRequired { reason: Arc<str> },
    #[error("An admin must approve access to Azure DevOps.")]
    ConsentRequired { admin: bool, detail: Arc<str> },
    #[error("Your organization's sign-in policy blocked access from this device.")]
    ConditionalAccessBlocked(Arc<str>),
    #[error("This organization uses Microsoft accounts. Use a personal access token.")]
    MsaOrgUnsupported,
    #[error("The token is invalid, expired or revoked.")]
    PatInvalid,
    #[error("The token is missing required access.")]
    PatInsufficientScope,
    #[error("No token saved for this organization.")]
    PatMissing,
    #[error("That doesn't look like an Azure DevOps token.")]
    PatFormat,
    #[error("Add your app's client ID to use this sign-in method.")]
    EntraNotConfigured,
    #[error("Azure DevOps asked for sign-in again.")]
    SignInPageReturned,
    #[error("Azure DevOps rejected the credentials.")]
    Unauthorized { code: Option<Arc<str>> },
    #[error("Sign-in cancelled.")]
    Cancelled,
    #[error("Sign-in timed out after {} minutes.", INTERACTIVE_SIGN_IN_TIMEOUT.as_secs() / 60)]
    Timeout,
    #[error("Signed out.")]
    SignedOut,
    #[error("No sign-in method is set up yet.")]
    NoMethod {
        tried: Arc<[(AuthMethod, Arc<str>)]>,
    },
    #[error("Can't reach Azure DevOps.")]
    Network(Arc<str>),
    #[error("The system credential store couldn't be used.")]
    SecretStore(Arc<str>),
    // why: unexpected responses from auth endpoints that fit no variant; the text is shown as-is.
    #[error("{0}")]
    Other(Arc<str>),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum AuthSeverity {
    /// Retry later; keep the current state.
    Transient,
    /// The user must sign in (or fix the PAT).
    NeedsSignIn,
    /// Policy blocks access; only an admin can fix it.
    Blocked,
}

/// Which button the UI shows for an error. Resolved into a [`UserAction`] by
/// [`AuthError::primary_action`] / [`AuthError::secondary_action`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Action {
    None,
    ContactAdmin,
    SignInWithMicrosoft,
    ChooseSignIn,
    EnterPat,
    Retry,
    Docs(DocLink),
}

/// Everything the UI needs to know about one error variant, described in one place.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct ErrorProfile {
    severity: AuthSeverity,
    /// Lower is more actionable (used by the Auto chain to pick which failure to show).
    rank: u8,
    primary: Action,
    secondary: Action,
}

const fn profile(
    severity: AuthSeverity,
    rank: u8,
    primary: Action,
    secondary: Action,
) -> ErrorProfile {
    ErrorProfile {
        severity,
        rank,
        primary,
        secondary,
    }
}

impl AuthError {
    fn profile(&self) -> ErrorProfile {
        use Action::{ChooseSignIn, ContactAdmin, Docs, EnterPat, Retry, SignInWithMicrosoft};
        use AuthSeverity::{Blocked, NeedsSignIn, Transient};
        match self {
            Self::ConditionalAccessBlocked(_) => {
                profile(Blocked, 1, ContactAdmin, SignInWithMicrosoft)
            }
            Self::ConsentRequired { .. } => profile(Blocked, 2, ContactAdmin, EnterPat),
            Self::InteractionRequired { .. } => {
                profile(NeedsSignIn, 3, SignInWithMicrosoft, EnterPat)
            }
            Self::AzNotLoggedIn { .. } => profile(NeedsSignIn, 4, SignInWithMicrosoft, EnterPat),
            Self::Unauthorized { .. } | Self::SignInPageReturned => {
                profile(NeedsSignIn, 5, ChooseSignIn, Action::None)
            }
            Self::PatInvalid | Self::PatInsufficientScope | Self::PatFormat => {
                profile(NeedsSignIn, 6, EnterPat, Docs(DocLink::CreatePat))
            }
            Self::MsaOrgUnsupported => profile(NeedsSignIn, 7, EnterPat, Docs(DocLink::CreatePat)),
            Self::AzTimeout | Self::AzFailed { .. } | Self::Network(_) | Self::SecretStore(_) => {
                profile(Transient, 8, Retry, EnterPat)
            }
            Self::Other(_) => profile(Transient, 8, Retry, Action::None),
            Self::AzNotInstalled => {
                profile(NeedsSignIn, 9, EnterPat, Docs(DocLink::InstallAzureCli))
            }
            Self::PatMissing
            | Self::EntraNotConfigured
            | Self::NoMethod { .. }
            | Self::SignedOut => profile(NeedsSignIn, 10, ChooseSignIn, Action::None),
            Self::Cancelled | Self::Timeout => profile(Transient, 11, Action::None, Action::None),
        }
    }

    pub fn severity(&self) -> AuthSeverity {
        self.profile().severity
    }

    pub fn rank(&self) -> u8 {
        self.profile().rank
    }

    pub fn primary_action(&self) -> Option<UserAction> {
        self.to_user_action(self.profile().primary)
    }

    pub fn secondary_action(&self) -> Option<UserAction> {
        self.to_user_action(self.profile().secondary)
    }

    fn to_user_action(&self, action: Action) -> Option<UserAction> {
        match action {
            Action::None => None,
            Action::ContactAdmin => Some(UserAction::ContactAdmin {
                message: Arc::from(admin_message(self.detail().as_deref())),
            }),
            Action::SignInWithMicrosoft => Some(UserAction::SignInWithMicrosoft),
            Action::ChooseSignIn => Some(UserAction::ChooseSignIn),
            Action::EnterPat => Some(UserAction::EnterPat),
            Action::Retry => Some(UserAction::Retry),
            Action::Docs(link) => Some(UserAction::OpenDocs(link)),
        }
    }

    /// `TF…`, `VS…` or `AADSTS…` when one appears in the error's detail.
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Unauthorized { code } => code.as_deref(),
            Self::InteractionRequired { reason: text }
            | Self::ConsentRequired { detail: text, .. }
            | Self::ConditionalAccessBlocked(text)
            | Self::AzFailed { stderr: text, .. } => extract_code(text),
            Self::AzNotInstalled
            | Self::AzNotLoggedIn { .. }
            | Self::AzTimeout
            | Self::MsaOrgUnsupported
            | Self::PatInvalid
            | Self::PatInsufficientScope
            | Self::PatMissing
            | Self::PatFormat
            | Self::EntraNotConfigured
            | Self::SignInPageReturned
            | Self::Cancelled
            | Self::Timeout
            | Self::SignedOut
            | Self::NoMethod { .. }
            | Self::Network(_)
            | Self::SecretStore(_)
            | Self::Other(_) => None,
        }
    }

    /// Technical detail for logs and the "details" disclosure; never secret-bearing.
    pub fn detail(&self) -> Option<Arc<str>> {
        match self {
            Self::AzFailed { stderr: text, .. }
            | Self::InteractionRequired { reason: text }
            | Self::ConsentRequired { detail: text, .. }
            | Self::ConditionalAccessBlocked(text)
            | Self::Network(text)
            | Self::SecretStore(text) => Some(text.clone()),
            Self::NoMethod { tried } if !tried.is_empty() => Some(Arc::from(describe_tried(tried))),
            Self::NoMethod { .. }
            | Self::AzNotInstalled
            | Self::AzNotLoggedIn { .. }
            | Self::AzTimeout
            | Self::MsaOrgUnsupported
            | Self::PatInvalid
            | Self::PatInsufficientScope
            | Self::PatMissing
            | Self::PatFormat
            | Self::EntraNotConfigured
            | Self::SignInPageReturned
            | Self::Unauthorized { .. }
            | Self::Cancelled
            | Self::Timeout
            | Self::SignedOut
            | Self::Other(_) => None,
        }
    }
}

fn describe_tried(tried: &[(AuthMethod, Arc<str>)]) -> String {
    tried
        .iter()
        .map(|(method, error)| format!("{}: {error}", method.label()))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Text the user can paste to their admin when a policy blocks sign-in.
fn admin_message(detail: Option<&str>) -> String {
    let base = format!(
        "{APP_DISPLAY_NAME}, a desktop app that reads Azure DevOps pull requests, was blocked \
         from signing in. Please allow it, or tell me which sign-in method to use."
    );
    match detail.filter(|d| !d.is_empty()) {
        Some(detail) => format!("{base}\n\nError: {detail}"),
        None => base,
    }
}

/// Minimum digits after a prefix for a token to count as an error code.
const MIN_CODE_DIGITS: usize = 3;

/// Finds the first `AADSTS`, `TF` or `VS` prefix followed by 3 or more digits, starting at a
/// word boundary. Prefixes are tried in that order.
pub fn extract_code(text: &str) -> Option<&str> {
    ["AADSTS", "TF", "VS"].into_iter().find_map(|prefix| {
        text.match_indices(prefix).find_map(|(start, _)| {
            let at_word_start = start == 0 || !text.as_bytes()[start - 1].is_ascii_alphanumeric();
            let digits_start = start + prefix.len();
            let digits = text[digits_start..]
                .bytes()
                .take_while(u8::is_ascii_digit)
                .count();
            (at_word_start && digits >= MIN_CODE_DIGITS)
                .then(|| &text[start..digits_start + digits])
        })
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserAction {
    SignInWithMicrosoft,
    SignInEntra,
    EnterPat,
    ChooseSignIn,
    Retry,
    ContactAdmin { message: Arc<str> },
    OpenDocs(DocLink),
    OpenSettings(SettingsSection),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingsSection {
    #[default]
    General,
    Organizations,
    Auth,
    People,
    Scope,
    Notifications,
    Appearance,
    About,
}

#[cfg(test)]
mod tests;
