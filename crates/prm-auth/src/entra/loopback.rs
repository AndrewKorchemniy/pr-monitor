//! The loopback redirect listener (RFC 8252): the browser comes back to
//! `http://localhost:{port}/?code=…&state=…` and this hands the code to the app.

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use prm_domain::auth::AuthError;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

use super::token::map_oauth_error;

/// Requests larger than this are not OAuth callbacks.
const MAX_REQUEST_BYTES: usize = 8 * 1024;
/// A connection that sends no full request in this long is dropped. Browsers open spare
/// preconnect sockets that stay silent.
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
const DONE_PAGE: &str =
    "<!doctype html><title>PR Monitor</title><p>Signed in. You can close this tab.";

/// What one connection turned out to be. `None` from a handler means "not the callback".
type Handled = Option<Result<String, AuthError>>;

pub struct Loopback {
    v4: TcpListener,
    /// Browsers may resolve `localhost` to `::1` first.
    v6: Option<TcpListener>,
    port: u16,
}

impl Loopback {
    /// Binds `127.0.0.1` on a free port, then `[::1]` on the same port if possible.
    pub async fn bind() -> std::io::Result<Self> {
        let v4 = TcpListener::bind(("127.0.0.1", 0)).await?;
        let port = v4.local_addr()?.port();
        let v6 = TcpListener::bind(("::1", port)).await.ok();
        Ok(Self { v4, v6, port })
    }

    /// Entra ignores the port when matching `http://localhost` redirect URIs.
    pub fn redirect_uri(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    #[cfg(test)]
    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    /// Waits for the callback and returns the authorization code. Connections are handled
    /// concurrently, so a silent socket never blocks the callback, cancel or the deadline.
    /// Requests whose `state` doesn't match are answered 400 and ignored.
    pub async fn wait_for_code(
        self,
        expected_state: &str,
        cancel: CancellationToken,
        timeout: Duration,
    ) -> Result<String, AuthError> {
        let expected: Arc<str> = Arc::from(expected_state);
        let deadline = tokio::time::sleep(timeout);
        tokio::pin!(deadline);
        let mut connections: JoinSet<Handled> = JoinSet::new();
        loop {
            tokio::select! {
                accepted = self.v4.accept() => {
                    spawn_handler(&mut connections, accepted.map(|(s, _)| s), &expected);
                }
                accepted = accept_optional(self.v6.as_ref()) => {
                    spawn_handler(&mut connections, accepted, &expected);
                }
                Some(joined) = connections.join_next() => {
                    if let Ok(Some(result)) = joined {
                        return result;
                    }
                }
                () = &mut deadline => return Err(AuthError::Timeout),
                () = cancel.cancelled() => return Err(AuthError::Cancelled),
            }
        }
    }
}

/// Handles one accepted connection in the background, bounded by `CONNECTION_TIMEOUT`.
fn spawn_handler(
    connections: &mut JoinSet<Handled>,
    accepted: std::io::Result<TcpStream>,
    expected: &Arc<str>,
) {
    let Ok(stream) = accepted else { return };
    let expected = expected.clone();
    connections.spawn(async move {
        tokio::time::timeout(CONNECTION_TIMEOUT, handle(stream, &expected))
            .await
            .ok()
            .flatten()
    });
}

async fn accept_optional(listener: Option<&TcpListener>) -> std::io::Result<TcpStream> {
    match listener {
        Some(listener) => listener.accept().await.map(|(s, _)| s),
        None => std::future::pending().await,
    }
}

/// `None` for requests that aren't this sign-in's callback (`/favicon.ico`, a wrong `state`).
async fn handle(mut stream: TcpStream, expected_state: &str) -> Handled {
    let request = read_head(&mut stream).await?;
    let first_line = request.lines().next().unwrap_or_default();
    let Some(params) = parse_request_line(first_line) else {
        respond(&mut stream, "404 Not Found", "").await;
        return None;
    };
    // why: only a request carrying this sign-in's `state` may end it; anything else on the
    // port (another page, a local process) is ignored instead of aborting the sign-in.
    if params.get("state").map(String::as_str) != Some(expected_state) {
        respond(&mut stream, "400 Bad Request", "").await;
        return None;
    }
    let result = callback_result(&params);
    respond(&mut stream, "200 OK", DONE_PAGE).await;
    Some(result)
}

/// The code, or the OAuth error, of a callback whose `state` already matched.
fn callback_result(params: &BTreeMap<String, String>) -> Result<String, AuthError> {
    if let Some(error) = params.get("error") {
        let description = params.get("error_description").map_or("", String::as_str);
        return Err(map_oauth_error(error, description, &[]));
    }
    params
        .get("code")
        .cloned()
        .ok_or_else(|| AuthError::Other("Sign-in response had no code.".into()))
}

async fn read_head(stream: &mut TcpStream) -> Option<String> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    while !buffer.windows(4).any(|w| w == b"\r\n\r\n") && buffer.len() < MAX_REQUEST_BYTES {
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(chunk.get(..read)?);
    }
    String::from_utf8(buffer).ok()
}

async fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    if let Err(e) = stream.write_all(response.as_bytes()).await {
        tracing::debug!(error = %e, "loopback response not delivered");
    }
}

/// Parses `GET /?code=…&state=… HTTP/1.1` into query parameters. `None` for other paths or
/// requests without a callback parameter.
pub fn parse_request_line(line: &str) -> Option<BTreeMap<String, String>> {
    let mut parts = line.split_whitespace();
    let (method, target) = (parts.next()?, parts.next()?);
    if method != "GET" {
        return None;
    }
    let (path, query) = target.split_once('?')?;
    if path != "/" {
        return None;
    }
    let params: BTreeMap<String, String> = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();
    (params.contains_key("code") || params.contains_key("error")).then_some(params)
}
