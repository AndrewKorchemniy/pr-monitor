//! `state.json`: machine-written state (last PR data, seen fingerprints, UI layout). The
//! service is its only writer.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use prm_domain::{
    OrgId, Person, PrKey, Timestamp,
    auth::{AuthMethod, SettingsSection, TenantInfo},
    model::PrRecord,
    snapshot::SeenMap,
    tombstone::Tombstones,
    view::{GroupBy, Sort, ViewQuery},
};

pub const STATE_SCHEMA: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PersistedState {
    pub schema_version: u32,
    pub saved_at: Option<Timestamp>,
    pub notifications_paused_until: Option<Timestamp>,
    pub orgs: BTreeMap<OrgId, OrgPersisted>,
    pub seen: SeenMap,
    pub ui: UiState,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            schema_version: STATE_SCHEMA,
            saved_at: None,
            notifications_paused_until: None,
            orgs: BTreeMap::new(),
            seen: SeenMap::new(),
            ui: UiState::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OrgPersisted {
    pub tenant: Option<TenantCache>,
    pub me: Option<Person>,
    pub last_good_method: Option<AuthMethod>,
    /// The user signed out: no silent sign-in until they sign in again.
    pub signed_out: bool,
    /// A snapshot exists, so the next refresh diffs against it (and may toast).
    pub baseline: bool,
    pub last_refresh_ok: Option<Timestamp>,
    pub org_wide_probe: Option<ProbeCache>,
    pub records: BTreeMap<PrKey, PrRecord>,
    pub tombstones: Tombstones,
    /// Most recently seen first; capped at `limits::MAX_PERSISTED_PEOPLE`.
    pub people: Vec<Person>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantCache {
    pub info: TenantInfo,
    pub discovered_at: Timestamp,
}

/// Whether the undocumented org-wide PR query worked when last probed.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeCache {
    pub ok: bool,
    pub at: Timestamp,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct UiState {
    pub window: Option<WindowRect>,
    /// Column id → width in points.
    pub columns: BTreeMap<String, f32>,
    pub hidden_columns: Vec<String>,
    pub sort: Sort,
    pub group: GroupBy,
    pub filter: ViewQuery,
    pub selected: Option<PrKey>,
    pub collapsed_groups: Vec<String>,
    pub settings_section: Option<SettingsSection>,
    pub peek_open: bool,
    pub peek_width: Option<f32>,
}

/// Outer window rectangle in logical points.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    #[serde(default)]
    pub maximized: bool,
}
