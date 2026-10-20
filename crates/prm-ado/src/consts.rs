//! API versions, header names and paging limits for Azure DevOps Services.

/// GA REST version for git, core and identities.
pub const API_GA: &str = "7.1";
pub const API_CONNECTION_DATA: &str = "7.1-preview.1";
pub const API_GRAPH_SUBJECT_QUERY: &str = "7.1-preview.1";
pub const API_GRAPH_STORAGE_KEY: &str = "7.1";
pub const API_IDENTITIES: &str = "7.1";

/// `$top` for PR list pages.
pub const PAGE_SIZE: u32 = 500;
/// Safety stop for skip paging.
pub const MAX_PAGES: u32 = 50;
/// Safety stop for continuation-token paging.
pub const MAX_CONTINUATION_PAGES: u32 = 100;
pub const ORG_WIDE_PROBE_TOP: u32 = 50;
/// Identities per `identityIds` request.
pub const IDENTITY_BATCH: usize = 50;
/// Default concurrent requests per org.
pub const MAX_CONCURRENCY: usize = 6;

/// Turns the sign-in redirect into a plain 401.
pub const H_FEDAUTH: &str = "x-tfs-fedauthredirect";
pub const V_FEDAUTH: &str = "Suppress";
pub const H_MSA_PASSTHROUGH: &str = "x-vss-forcemsapassthrough";
pub const V_MSA_PASSTHROUGH: &str = "true";
/// Correlates server logs with one app run.
pub const H_SESSION: &str = "x-tfs-session";
pub const H_CONTINUATION: &str = "x-ms-continuationtoken";
pub const H_RESOURCE_TENANT: &str = "x-vss-resourcetenant";
pub const H_E2EID: &str = "x-vss-e2eid";
pub const H_ACTIVITY: &str = "activityid";
pub const H_RETRY_AFTER: &str = "retry-after";
pub const H_RL_LIMIT: &str = "x-ratelimit-limit";
pub const H_RL_REMAINING: &str = "x-ratelimit-remaining";
pub const H_RL_RESET: &str = "x-ratelimit-reset";
pub const H_RL_DELAY: &str = "x-ratelimit-delay";
pub const H_RL_RESOURCE: &str = "x-ratelimit-resource";
