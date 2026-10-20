//! Azure DevOps resource ids, timeouts and limits for authentication.

use std::time::Duration;

/// The Azure DevOps resource (application) id.
pub const ADO_RESOURCE: &str = "499b84ac-1321-427f-aa17-267ca6975798";
pub const ENTRA_SCOPES: &str =
    "499b84ac-1321-427f-aa17-267ca6975798/.default offline_access openid profile";
pub const ENTRA_AUTHORITY: &str = "https://login.microsoftonline.com";

/// Tokens are refreshed when less than this remains. `az` guarantees at least 5 minutes, so a
/// larger skew could re-spawn `az` on every request.
pub const TOKEN_SKEW: Duration = Duration::from_secs(120);
/// A failed silent acquisition is not retried for this long (avoids spawning `az` per request).
pub const NEG_CACHE_TTL: Duration = Duration::from_secs(30);
pub const AZ_TOKEN_TIMEOUT: Duration = prm_domain::auth::AZ_TIMEOUT;
pub const AZ_LOGIN_TIMEOUT: Duration = prm_domain::auth::INTERACTIVE_SIGN_IN_TIMEOUT;
pub const ENTRA_SIGNIN_TIMEOUT: Duration = prm_domain::auth::INTERACTIVE_SIGN_IN_TIMEOUT;
/// Assumed lifetime when `az` reports no expiry and the token isn't a readable JWT.
pub const AZ_FALLBACK_LIFETIME: Duration = Duration::from_secs(300);
/// A cached tenant is trusted for this long before rediscovery.
pub const TENANT_TTL: Duration = Duration::from_hours(7 * 24);
/// Bound on waiting for `taskkill` after a timeout or cancel.
pub const TREE_KILL_TIMEOUT: Duration = Duration::from_secs(5);

/// Secret store chunk size (Credential Manager caps blobs at 2560 bytes).
pub const CHUNK_SIZE: usize = 2000;
/// Marks a chunked-secret header: `PRMC1:{parts}:{len}:{fnv64 hex}`.
pub const CHUNK_MAGIC: &str = "PRMC1:";

/// Windows `CREATE_NO_WINDOW`: no console flash when a GUI app spawns `az`.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// `cmd.exe` exit code for "command not found".
pub const EXIT_NOT_FOUND: i32 = 9009;
