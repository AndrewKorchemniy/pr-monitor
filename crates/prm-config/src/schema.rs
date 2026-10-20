//! The `config.toml` model. Every struct is `#[serde(default)]` and unknown keys are ignored,
//! so older and newer files still load; unknown enum values fall back to the default.

use std::{collections::HashSet, fmt, str::FromStr, sync::Arc};

use serde::{Deserialize, Serialize};

use prm_domain::{
    OrgId, Person, Vsid,
    auth::AuthMode,
    classify::{ClassifyConfig, EmptyReviewerPolicy},
    notify::NotificationPrefs,
    plan::{DraftPolicy, TrackSpec},
    scope::ScopeSpec,
};

use crate::{limits, migrate::CURRENT_SCHEMA};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub schema_version: u32,
    pub refresh: RefreshConfig,
    pub notifications: NotificationPrefs,
    pub ui: UiPrefs,
    pub network: NetworkConfig,
    pub diagnostics: DiagnosticsConfig,
    pub orgs: Vec<OrgProfile>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA,
            refresh: RefreshConfig::default(),
            notifications: NotificationPrefs::default(),
            ui: UiPrefs::default(),
            network: NetworkConfig::default(),
            diagnostics: DiagnosticsConfig::default(),
            orgs: Vec::new(),
        }
    }
}

impl Config {
    pub fn enabled_orgs(&self) -> impl Iterator<Item = &OrgProfile> {
        self.orgs
            .iter()
            .filter(|o| o.enabled && o.org_id().is_some())
    }

    pub fn org(&self, id: &OrgId) -> Option<&OrgProfile> {
        self.orgs.iter().find(|o| o.org_id() == Some(id))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RefreshConfig {
    pub interval_minutes: u32,
    pub jitter_percent: u8,
    /// Delay of the first refresh after launch.
    pub startup_delay_secs: u32,
}

impl Default for RefreshConfig {
    fn default() -> Self {
        Self {
            interval_minutes: limits::DEFAULT_INTERVAL_MINUTES,
            jitter_percent: limits::DEFAULT_JITTER_PERCENT,
            startup_delay_secs: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiPrefs {
    /// `"system"` or a theme id owned by prm-theme.
    pub theme: String,
    /// Used when `theme = "system"` and the OS is dark.
    pub theme_dark: String,
    pub theme_light: String,
    pub density: Density,
    pub font_scale: f32,
    pub animations: Tristate,
    pub renderer: RendererPref,
    pub low_power: Tristate,
    pub start_minimized: bool,
    pub close_to_tray: bool,
    pub minimize_to_tray: bool,
    pub launch_at_login: bool,
    pub show_org_column: OrgColumn,
    pub relative_times: bool,
}

pub const THEME_SYSTEM: &str = "system";
pub const DEFAULT_DARK_THEME: &str = "graphite";
pub const DEFAULT_LIGHT_THEME: &str = "vellum";

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            theme: THEME_SYSTEM.into(),
            theme_dark: DEFAULT_DARK_THEME.into(),
            theme_light: DEFAULT_LIGHT_THEME.into(),
            density: Density::default(),
            font_scale: limits::DEFAULT_FONT_SCALE,
            animations: Tristate::default(),
            renderer: RendererPref::default(),
            low_power: Tristate::default(),
            start_minimized: false,
            close_to_tray: true,
            minimize_to_tray: true,
            launch_at_login: false,
            show_org_column: OrgColumn::default(),
            relative_times: true,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Density {
    Comfortable,
    /// Last because `#[serde(other)]` must be the final variant.
    #[default]
    #[serde(other)]
    Compact,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tristate {
    On,
    Off,
    /// Decide from the environment (remote session, software rendering, OS setting).
    #[default]
    #[serde(other)]
    Auto,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererPref {
    Glow,
    Wgpu,
    #[default]
    #[serde(other)]
    Auto,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrgColumn {
    Always,
    Never,
    /// Only when more than one org is enabled.
    #[default]
    #[serde(other)]
    Auto,
}

impl OrgColumn {
    pub const fn resolve(self, enabled_orgs: usize) -> bool {
        match self {
            Self::Always => true,
            Self::Never => false,
            Self::Auto => enabled_orgs > 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkConfig {
    pub proxy: ProxyConfig,
    pub connect_timeout_secs: u32,
    pub request_timeout_secs: u32,
    /// Concurrent requests per org.
    pub max_concurrency: u8,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            proxy: ProxyConfig::System,
            connect_timeout_secs: limits::DEFAULT_CONNECT_TIMEOUT_SECS,
            request_timeout_secs: limits::DEFAULT_REQUEST_TIMEOUT_SECS,
            max_concurrency: limits::DEFAULT_MAX_CONCURRENCY,
        }
    }
}

/// `"system"` | `"none"` | `"pac"` | `"http://proxy:8080"`. Unparsable values become `System`.
#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum ProxyConfig {
    #[default]
    System,
    None,
    /// Resolve per request through the OS (WPAD/PAC).
    Pac,
    Manual(url::Url),
}

impl From<String> for ProxyConfig {
    fn from(s: String) -> Self {
        s.parse().unwrap_or_default()
    }
}

impl FromStr for ProxyConfig {
    type Err = url::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "system" => Ok(Self::System),
            "none" => Ok(Self::None),
            "pac" => Ok(Self::Pac),
            _ => url::Url::parse(s.trim()).map(Self::Manual),
        }
    }
}

impl fmt::Display for ProxyConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::System => f.write_str("system"),
            Self::None => f.write_str("none"),
            Self::Pac => f.write_str("pac"),
            Self::Manual(url) => f.write_str(url.as_str()),
        }
    }
}

impl From<ProxyConfig> for String {
    fn from(p: ProxyConfig) -> Self {
        p.to_string()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DiagnosticsConfig {
    /// A `tracing` filter; the `PRMONITOR_LOG` env var overrides it.
    pub log_level: String,
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        Self {
            log_level: "info".into(),
        }
    }
}

/// One Azure DevOps organization and everything watched in it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OrgProfile {
    /// Derived from `url` by `normalize`; never trust a hand-edited value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<OrgId>,
    pub url: String,
    pub label: String,
    pub enabled: bool,
    pub auth: AuthMode,
    /// GUID override; empty means discover from the org.
    pub tenant: String,
    /// Track the signed-in user's own PRs too.
    pub include_me: bool,
    pub drafts: DraftPolicy,
    /// Watch, but never toast.
    pub muted: bool,
    /// Experimental single org-wide query; probed before use.
    pub org_wide_query: bool,
    /// Optional `YYYY-MM-DD`, for PAT expiry reminders.
    pub pat_expires_on: String,
    pub review: ReviewConfig,
    pub entra: EntraSettings,
    pub scope: ScopeSpec,
    pub people: PeopleConfig,
}

impl Default for OrgProfile {
    fn default() -> Self {
        Self {
            id: None,
            url: String::new(),
            label: String::new(),
            enabled: true,
            auth: AuthMode::default(),
            tenant: String::new(),
            include_me: true,
            drafts: DraftPolicy::default(),
            muted: false,
            org_wide_query: false,
            pat_expires_on: String::new(),
            review: ReviewConfig::default(),
            entra: EntraSettings::default(),
            scope: ScopeSpec::default(),
            people: PeopleConfig::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ReviewConfig {
    pub empty_reviewer_list: EmptyReviewerPolicy,
}

/// Advanced sign-in with the organization's own app registration. Never pre-filled.
#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EntraSettings {
    pub client_id: String,
    /// GUID, domain or `organizations`; empty means the org's discovered tenant.
    pub tenant: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PeopleConfig {
    pub tracked_authors: Vec<Person>,
    pub reviewers: Vec<Person>,
    /// Bots and pipelines whose threads never count.
    pub ignored_comment_authors: Vec<Person>,
}

impl OrgProfile {
    /// Set by `normalize`; `None` means the URL could not be understood.
    pub fn org_id(&self) -> Option<&OrgId> {
        self.id.as_ref()
    }

    /// The only conversion from config to classification rules.
    pub fn classify_config(&self) -> ClassifyConfig {
        ClassifyConfig {
            reviewers: ids(&self.people.reviewers),
            empty_list: self.review.empty_reviewer_list,
            ignored_comment_authors: self.ignored_ids(),
        }
    }

    pub fn ignored_ids(&self) -> HashSet<Vsid> {
        ids(&self.people.ignored_comment_authors)
    }

    pub fn track_spec(&self, me: Option<&Person>) -> TrackSpec {
        let mut authors = ids(&self.people.tracked_authors);
        if self.include_me {
            authors.extend(me.map(|m| m.id));
        }
        TrackSpec {
            authors,
            drafts: self.drafts,
        }
    }

    pub fn tenant_override(&self) -> Option<uuid::Uuid> {
        uuid::Uuid::parse_str(self.tenant.trim()).ok()
    }

    /// The label, or the org id when no label is set.
    pub fn display_label(&self) -> Arc<str> {
        match (self.label.trim(), &self.id) {
            ("", Some(id)) => Arc::from(id.as_str()),
            (label, _) => Arc::from(label),
        }
    }
}

fn ids(people: &[Person]) -> HashSet<Vsid> {
    people.iter().map(|p| p.id).collect()
}

#[cfg(test)]
mod tests;
