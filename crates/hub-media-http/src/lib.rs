//! Media over loopback HTTP: how audio, covers and downloads reach a webview.
//!
//! WebKitGTK cannot stream media from a custom URI scheme (`FormatError`), while every
//! webview plays plain `http://127.0.0.1` media with Range requests. So the app starts this
//! small server on a random loopback port. Hides: the URL layout, the access token, the Host
//! check and the response headers. The bytes themselves come from `Hub::media`.
//!
//! Threat model: another local process or a web page must not read the signed-in user's
//! media. The URL carries a random 128-bit token chosen at start-up, and the `Host` header
//! must be exactly our loopback address (blocks DNS rebinding).

mod policy;

pub use policy::{content_disposition, media_cache_control};

use hub_core::{Hub, HubError};
use hub_rpc::parse_media_target;
use std::io::{Cursor, Read};
use std::sync::Arc;
use tiny_http::{Header, Response, StatusCode};

/// A response body that is either buffered (errors) or streamed (media).
pub type Body = Box<dyn Read + Send>;

fn header(name: &str, value: &str) -> Header {
    // A value an upstream server controls (content type) that is not valid ASCII degrades to
    // a neutral one instead of panicking.
    Header::from_bytes(name, value)
        .or_else(|()| Header::from_bytes(name, "application/octet-stream"))
        .expect("fallback header is valid ASCII")
}

fn text(status: u16, message: &str) -> Response<Body> {
    let body: Body = Box::new(Cursor::new(message.as_bytes().to_vec()));
    Response::new(
        StatusCode(status),
        vec![header("Content-Type", "text/plain; charset=utf-8")],
        body,
        Some(message.len()),
        None,
    )
}

fn status_of(error: &HubError) -> u16 {
    match error {
        HubError::NotSignedIn => 401,
        HubError::Forbidden { .. } => 403,
        HubError::NotFound { .. } | HubError::UnknownModule(_) => 404,
        HubError::BadArgs { .. } => 400,
        _ => 502,
    }
}

/// Answers `<module>/<op>?<params>` (leading slash optional) with the module's media.
pub fn media_response(hub: &Hub, target: &str, range: Option<&str>) -> Response<Body> {
    let result = parse_media_target(target, range).and_then(|(module, request)| {
        hub.media(&module, &request)
            .map(|media| (request.op, media))
    });
    let (op, media) = match result {
        Ok(found) => found,
        Err(error) => return text(status_of(&error), &error.view().message),
    };
    let mut headers = vec![
        header("Content-Type", &media.content_type),
        header("Accept-Ranges", "bytes"),
        header("Cache-Control", media_cache_control(&op)),
    ];
    if let Some(range) = &media.content_range {
        headers.push(header("Content-Range", range));
    }
    if let (true, Some(name)) = (op == "download", &media.file_name) {
        headers.push(header("Content-Disposition", &content_disposition(name)));
    }
    let length = media.content_length.and_then(|n| usize::try_from(n).ok());
    Response::new(StatusCode(media.status), headers, media.body, length, None)
}

/// A running loopback media server. Dropping it does not stop the thread; it lives as long
/// as the app does.
pub struct MediaServer {
    base_url: String,
}

impl MediaServer {
    /// `http://127.0.0.1:<port>/<token>`: append `/<module>/<op>?<params>`.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

fn random_token() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| format!("no system randomness: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Starts the server on a random loopback port.
pub fn spawn(hub: Arc<Hub>) -> std::io::Result<MediaServer> {
    let server = tiny_http::Server::http("127.0.0.1:0").map_err(std::io::Error::other)?;
    let port = server
        .server_addr()
        .to_ip()
        .map(|addr| addr.port())
        .ok_or_else(|| std::io::Error::other("no loopback port"))?;
    let token = random_token().map_err(std::io::Error::other)?;
    let expected_host = format!("127.0.0.1:{port}");
    let prefix = format!("/{token}");
    let base_url = format!("http://127.0.0.1:{port}{prefix}");
    let server = Arc::new(server);
    let handle_hub = hub;
    std::thread::spawn(move || {
        for request in server.incoming_requests() {
            let hub = Arc::clone(&handle_hub);
            let (expected_host, prefix) = (expected_host.clone(), prefix.clone());
            // One thread per request: a playing track holds its connection open.
            std::thread::spawn(move || handle(&hub, &expected_host, &prefix, request));
        }
    });
    Ok(MediaServer { base_url })
}

fn handle(hub: &Hub, expected_host: &str, prefix: &str, request: tiny_http::Request) {
    let host_ok = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Host"))
        .is_some_and(|h| h.value.as_str() == expected_host);
    let range = request
        .headers()
        .iter()
        .find(|h| h.field.equiv("Range"))
        .map(|h| h.value.as_str().to_owned());
    let url = request.url().to_owned();
    let response = match (host_ok, url.strip_prefix(prefix)) {
        (true, Some(target)) if target.starts_with('/') => {
            media_response(hub, target, range.as_deref())
        }
        _ => text(404, "not found"),
    };
    if let Err(e) = request.respond(response) {
        // The player closes connections when it seeks; that is normal.
        eprintln!("[media] connection closed early: {e}");
    }
}

#[cfg(test)]
mod tests;
