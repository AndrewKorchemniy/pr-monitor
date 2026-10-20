//! A typed Azure DevOps REST client for PR Monitor: URL building, the request pipeline
//! (headers, rate gate, retries, token refresh, sign-in-page detection), paging, and mapping
//! wire JSON to domain types. Transport and credential are injected seams.
#![forbid(unsafe_code)]

pub mod api;
pub mod client;
pub mod connector;
pub mod consts;
pub mod credential;
pub(crate) mod dto;
pub mod error;
pub mod http;
pub(crate) mod map;
pub mod paging;
pub mod rate;
pub mod retry;
pub mod urls;

#[cfg(any(test, feature = "test-support"))]
pub mod testing;

pub use api::{AdoApi, OrgWideProbe};
pub use client::{AdoClient, ClientPolicy};
pub use connector::{AdoConnector, HttpAdoConnector};
pub use credential::{Credential, StaticCredential};
pub use error::AdoError;
pub use http::{HttpTransport, ProxySetting, ReqwestTransport, TransportConfig, TransportError};
pub use rate::RateSnapshot;
pub use urls::OrgEndpoint;
