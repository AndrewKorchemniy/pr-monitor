//! Projects and repositories.

use serde::Deserialize;
use uuid::Uuid;

use super::common::ProjectRefDto;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectDto {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) state: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RepositoryDto {
    pub(crate) id: Uuid,
    pub(crate) name: String,
    pub(crate) project: ProjectRefDto,
    #[serde(default)]
    pub(crate) is_disabled: bool,
    #[serde(default)]
    pub(crate) is_in_maintenance: bool,
    #[serde(default)]
    pub(crate) is_fork: bool,
    #[serde(default)]
    pub(crate) web_url: Option<String>,
}
