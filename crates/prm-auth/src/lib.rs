//! Authentication for Azure DevOps: Azure CLI session (default), "Sign in with Microsoft" via
//! `az login`, personal access tokens, and an advanced Entra sign-in with the organization's
//! own app. Per org: tenant discovery, a single-flight token cache and an Auto chain.

pub mod azcli;
pub mod cache;
pub mod chain;
pub mod consts;
pub mod discovery;
pub mod entra;
pub mod factory;
pub mod jwt;
pub mod org;
pub mod pat;
pub mod process;
pub mod secrets;
pub mod source;

#[cfg(any(test, feature = "test-support"))]
pub mod testing;

pub use factory::{AuthFactory, DefaultAuthFactory};
pub use org::{AuthAvailability, AuthSnapshot, AuthSpec, OrgAuth, OrgAuthenticator};
pub use process::{CommandRunner, TokioCommandRunner};
pub use secrets::{
    ChunkedSecretStore, InMemorySecretStore, SecretKey, SecretStore, SecretStoreError,
};
pub use source::{AuthContext, TokenSource};
