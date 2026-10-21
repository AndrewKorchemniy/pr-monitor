//! Adapters that work on every OS: activation handoff, keyring secrets, the URL opener and null
//! fallbacks.

pub(crate) mod handoff_client;
pub(crate) mod handoff_server;
pub(crate) mod keyring_store;
pub(crate) mod null;
pub(crate) mod secret_store;
pub(crate) mod url_opener;

use std::io;

use interprocess::local_socket::Name;
use prm_domain::ports::{PortError, PortErrorKind};

/// A foreign error as a `Failed` port error (message only). The one mapping for errors that
/// have no more specific kind.
pub(crate) fn failed(error: impl std::fmt::Display) -> PortError {
    PortError::new(PortErrorKind::Failed, error.to_string())
}

/// What an adapter returns for a feature this OS does not have.
#[cfg(not(windows))]
pub(crate) fn unsupported() -> PortError {
    PortError::new(PortErrorKind::Unsupported, "not supported on this platform")
}

/// The local socket name for an instance key's pipe name: a namespaced name where the OS has
/// them (Windows pipes, Linux abstract sockets), a socket file in the temp dir on macOS.
fn socket_name(pipe_name: &str) -> io::Result<Name<'static>> {
    use interprocess::local_socket::{GenericNamespaced, NameType, ToNsName};
    if GenericNamespaced::is_supported() {
        return pipe_name.to_owned().to_ns_name::<GenericNamespaced>();
    }
    use interprocess::local_socket::{GenericFilePath, ToFsName};
    std::env::temp_dir()
        .join(pipe_name)
        .to_fs_name::<GenericFilePath>()
}
