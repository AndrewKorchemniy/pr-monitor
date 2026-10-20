//! Adapters that work on every OS: activation handoff, keyring secrets, the URL opener and null
//! fallbacks.

pub mod handoff_client;
pub mod handoff_server;
pub mod keyring_store;
pub mod null;
pub mod secret_store;
pub mod url_opener;

use std::io;

use interprocess::local_socket::Name;

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
