//! Shapes shared by many endpoints.

use serde::Deserialize;
use uuid::Uuid;

/// `{ "count": N, "value": [...] }`.
#[derive(Debug, Deserialize)]
pub(crate) struct ListDto<T> {
    #[serde(default = "Vec::new")]
    pub(crate) value: Vec<T>,
}

/// Some endpoints return a bare array where others wrap it.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum ListOrVec<T> {
    List(ListDto<T>),
    Vec(Vec<T>),
}

impl<T> ListOrVec<T> {
    pub(crate) fn into_vec(self) -> Vec<T> {
        match self {
            Self::List(list) => list.value,
            Self::Vec(items) => items,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IdentityRefDto {
    pub(crate) id: Uuid,
    #[serde(default)]
    pub(crate) display_name: String,
    #[serde(default)]
    pub(crate) unique_name: Option<String>,
    #[serde(default)]
    pub(crate) descriptor: Option<String>,
    #[serde(default)]
    pub(crate) is_container: bool,
}

/// A property bag value: `{ "$type": "...", "$value": ... }`.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct PropValDto {
    #[serde(rename = "$value", default)]
    pub(crate) value: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectRefDto {
    pub(crate) id: Uuid,
    #[serde(default)]
    pub(crate) name: String,
}
