//! Lenient date fields: unparseable or unset (`0001-01-01`) dates become `None`.

use prm_domain::Timestamp;
use serde::{Deserialize, Deserializer};

pub(crate) fn opt_ts<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Timestamp>, D::Error> {
    let text = Option::<String>::deserialize(d)?;
    Ok(text.as_deref().and_then(Timestamp::parse_ado))
}
