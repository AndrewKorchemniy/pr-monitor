//! The loopback redirect listener (RFC 8252): the browser comes back to
//! `http://localhost:{port}/?code=…&state=…` and this hands the code to the app.

use std::{collections::BTreeMap, time::Duration};

use prm_domain::auth::AuthError;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_util::sync::CancellationToken;

use super::token::map_oauth_error;

/// Requests larger than this are not OAuth callbacks.
const MAX_REQUEST_BYTES: usize = 8 * 1024;
const DONE_PAGE: &str =
    "<!doctype html><title>PR Monitor</title><p>Signed in. You can close this tab.";

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

    pub fn port(&self) -> u16 {
        self.port
    }

    /// Waits for the callback and returns the authorization code.
    pub async fn wait_for_code(
        self,
        expected_state: &str,
        cancel: CancellationToken,
        timeout: Duration,
    ) -> Result<String, AuthError> {
        let deadline = tokio::time::sleep(timeout);
        tokio::pin!(deadline);
        loop {
            let stream = tokio::select! {
                accepted = self.v4.accept() => accepted.map(|(s, _)| s),
                accepted = accept_optional(self.v6.as_ref()) => accepted,
                () = &mut deadline => return Err(AuthError::Timeout),
                () = cancel.cancelled() => return Err(AuthError::Cancelled),
            };
            let Ok(stream) = stream else { continue };
            if let Some(result) = handle(stream, expected_state).await {
                return result;
            }
        }
    }
}

async fn accept_optional(listener: Option<&TcpListener>) -> std::io::Result<TcpStream> {
    match listener {
        Some(listener) => listener.accept().await.map(|(s, _)| s),
        None => std::future::pending().await,
    }
}

/// `None` for requests that aren't the callback (e.g. `/favicon.ico`).
async fn handle(mut stream: TcpStream, expected_state: &str) -> Option<Result<String, AuthError>> {
    let request = read_head(&mut stream).await?;
    let first_line = request.lines().next().unwrap_or_default();
    let Some(params) = parse_request_line(first_line) else {
        respond(&mut stream, "404 Not Found", "").await;
        return None;
    };
    let result = callback_result(&params, expected_state);
    respond(&mut stream, "200 OK", DONE_PAGE).await;
    Some(result)
}

fn callback_result(
    params: &BTreeMap<String, String>,
    expected_state: &str,
) -> Result<String, AuthError> {
    if let Some(error) = params.get("error") {
        let description = params.get("error_description").map_or("", String::as_str);
        return Err(map_oauth_error(error, description, &[]));
    }
    if params.get("state").map(String::as_str) != Some(expected_state) {
        return Err(AuthError::Other(
            "Sign-in response didn't match this request.".into(),
        ));
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
