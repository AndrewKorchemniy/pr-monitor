//! connectionData, IMS identities and Graph subjects.

use std::collections::HashMap;

use serde::Deserialize;
use uuid::Uuid;

use super::common::PropValDto;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConnectionDataDto {
    pub(crate) authenticated_user: ImsIdentityDto,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImsIdentityDto {
    pub(crate) id: Uuid,
    #[serde(default)]
    pub(crate) subject_descriptor: Option<String>,
    #[serde(default)]
    pub(crate) provider_display_name: Option<String>,
    #[serde(default)]
    pub(crate) custom_display_name: Option<String>,
    #[serde(default)]
    pub(crate) is_container: bool,
    /// `Account`, `Mail`, ...
    #[serde(default)]
    pub(crate) properties: HashMap<String, PropValDto>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GraphSubjectDto {
    #[serde(default)]
    pub(crate) subject_kind: Option<String>,
    pub(crate) descriptor: String,
    #[serde(default)]
    pub(crate) display_name: String,
    #[serde(default)]
    pub(crate) mail_address: Option<String>,
    #[serde(default)]
    pub(crate) principal_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct StorageKeyDto {
    pub(crate) value: Uuid,
}
