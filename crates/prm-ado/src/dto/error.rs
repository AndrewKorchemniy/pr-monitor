//! The VSS error body: `{"message": "TF400813: ...", "typeKey": "...", "errorCode": 0}`.

use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct VssErrorDto {
    #[serde(default)]
    pub(crate) message: String,
}
