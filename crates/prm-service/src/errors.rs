//! `UiError`: an error as the user sees it, with the button that fixes it. Every lower-level
//! error is mapped here, once.

use std::sync::Arc;

use prm_ado::AdoError;
use prm_config::ConfigError;
use prm_domain::{
    app::DocLink,
    auth::{AuthError, AuthSeverity, SettingsSection, UserAction},
    ports::PortError,
};

use crate::text;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiError {
    pub title: Arc<str>,
    pub detail: Option<Arc<str>>,
    /// A `TF…`/`VS…`/`AADSTS…` code, for support.
    pub code: Option<Arc<str>>,
    pub action: Option<UserAction>,
    pub secondary: Option<UserAction>,
    /// Likely to clear up on its own; the UI shows it quietly.
    pub transient: bool,
}

impl UiError {
    /// An error with only a title.
    pub fn new(title: &str) -> Self {
        Self {
            title: Arc::from(title),
            detail: None,
            code: None,
            action: None,
            secondary: None,
            transient: false,
        }
    }

    /// A bug or a panicked job; the log has the details.
    pub fn internal(detail: &str) -> Self {
        Self {
            detail: Some(Arc::from(detail)),
            ..Self::new(text::INTERNAL_ERROR)
        }
    }

    /// A port (OS adapter) failure with a title describing what was attempted.
    pub fn port(title: &str, error: &PortError) -> Self {
        Self {
            detail: Some(error.message.clone()),
            ..Self::new(title)
        }
    }

    fn with_detail(mut self, detail: Option<Arc<str>>) -> Self {
        self.detail = detail;
        self
    }

    fn with_code(mut self, code: Option<&str>) -> Self {
        self.code = code.map(Arc::from);
        self
    }

    fn transient(mut self, action: Option<UserAction>) -> Self {
        self.transient = true;
        self.action = action;
        self
    }
}

impl From<&AuthError> for UiError {
    fn from(error: &AuthError) -> Self {
        Self {
            title: Arc::from(error.to_string()),
            detail: error.detail(),
            code: error.code().map(Arc::from),
            action: error.primary_action(),
            secondary: error.secondary_action(),
            transient: error.severity() == AuthSeverity::Transient,
        }
    }
}

impl From<&AdoError> for UiError {
    fn from(error: &AdoError) -> Self {
        let code = error.code();
        match error {
            AdoError::Auth(auth) => Self::from(auth),
            AdoError::Forbidden { message, .. } => Self {
                action: Some(UserAction::OpenSettings(SettingsSection::Scope)),
                ..Self::new(text::FORBIDDEN).with_detail(Some(message.clone()))
            }
            .with_code(code),
            AdoError::NotFound { message } => {
                Self::new(text::NOT_FOUND).with_detail(message.clone())
            }
            AdoError::RateLimited { .. } => Self::new(text::RATE_LIMITED).transient(None),
            AdoError::Server { activity, .. } => Self::new(text::UNREACHABLE)
                .with_detail(activity.clone())
                .transient(Some(UserAction::Retry)),
            AdoError::Network(message) => Self::new(text::UNREACHABLE)
                .with_detail(Some(message.clone()))
                .transient(Some(UserAction::Retry)),
            AdoError::Timeout => Self::new(text::TIMED_OUT).transient(Some(UserAction::Retry)),
            AdoError::Http { message, .. } => Self::new(text::HTTP_ERROR)
                .with_detail(Some(message.clone()))
                .with_code(code),
            AdoError::ProxyAuthRequired => Self {
                action: Some(UserAction::OpenDocs(DocLink::Troubleshooting)),
                secondary: Some(UserAction::OpenSettings(SettingsSection::General)),
                ..Self::new(text::PROXY_AUTH)
            },
            AdoError::Decode(message) => {
                Self::new(text::UNEXPECTED_RESPONSE).with_detail(Some(message.clone()))
            }
            AdoError::Cancelled => Self::new(text::CANCELLED).transient(None),
        }
    }
}

impl From<&ConfigError> for UiError {
    fn from(error: &ConfigError) -> Self {
        Self::new(text::SETTINGS_NOT_SAVED).with_detail(Some(Arc::from(error.to_string())))
    }
}

#[cfg(test)]
mod tests;
