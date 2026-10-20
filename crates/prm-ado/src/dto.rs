//! Wire types for Azure DevOps JSON. Crate-private: they become domain types in `map` and
//! nowhere else. Optional fields default; string enums fall back to `Unknown`/`Other`.

pub(crate) mod common;
pub(crate) mod core;
pub(crate) mod date;
pub(crate) mod error;
pub(crate) mod identity;
pub(crate) mod pr;
pub(crate) mod thread;

#[cfg(test)]
mod tests;
