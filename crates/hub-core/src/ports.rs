//! Ports: what the core needs from the outside world, as traits.
//!
//! Hides: the HTTP library, where secrets live and how identity is verified. The core and
//! every module are tested with small hand-written fakes of these traits.

use crate::{Credentials, HubError};
use hub_wire::Me;
use serde::de::DeserializeOwned;
use std::io::Read;

/// HTTP verbs the modules use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
}

/// Request body. Kept in memory: uploads are bounded by the device's RAM (see plan, risks).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Body {
    #[default]
    Empty,
    Bytes(Vec<u8>),
}

/// One outgoing request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Body,
}

impl HttpRequest {
    pub fn new(method: Method, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            headers: Vec::new(),
            body: Body::Empty,
        }
    }

    pub fn get(url: impl Into<String>) -> Self {
        Self::new(Method::Get, url)
    }

    pub fn post(url: impl Into<String>) -> Self {
        Self::new(Method::Post, url)
    }

    pub fn header(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.push((name.to_owned(), value.into()));
        self
    }

    pub fn body(mut self, body: Vec<u8>) -> Self {
        self.body = Body::Bytes(body);
        self
    }

    /// `application/x-www-form-urlencoded` body from key/value pairs.
    pub fn form(self, pairs: &[(&str, &str)]) -> Self {
        let encoded = pairs
            .iter()
            .map(|(k, v)| format!("{}={}", percent_encode(k), percent_encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        self.header("Content-Type", "application/x-www-form-urlencoded")
            .body(encoded.into_bytes())
    }
}

/// Percent-encodes everything except RFC 3986 unreserved characters.
pub fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(char::from(byte));
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// One answer. The body streams: read it with [`HttpResponse::bytes`] or hand it to a shell.
pub struct HttpResponse {
    pub status: u16,
    /// Header names are lower-case.
    pub headers: Vec<(String, String)>,
    pub body: Box<dyn Read + Send>,
}

impl HttpResponse {
    /// Answer with an in-memory body (used by fakes and by adapters that already buffered).
    pub fn from_bytes(status: u16, headers: Vec<(String, String)>, body: Vec<u8>) -> Self {
        Self {
            status,
            headers,
            body: Box::new(std::io::Cursor::new(body)),
        }
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// Reads the whole body.
    pub fn bytes(mut self, service: &str) -> Result<Vec<u8>, HubError> {
        let mut out = Vec::new();
        self.body
            .read_to_end(&mut out)
            .map_err(|e| HubError::unavailable(service, e))?;
        Ok(out)
    }

    /// Reads the body as JSON of type `T`.
    pub fn json<T: DeserializeOwned>(self, service: &str) -> Result<T, HubError> {
        let bytes = self.bytes(service)?;
        serde_json::from_slice(&bytes).map_err(|e| HubError::malformed(service, e))
    }

    /// Maps a non-2xx answer to the matching [`HubError`]; passes a 2xx through.
    pub fn ok(self, service: &str, what: &str) -> Result<Self, HubError> {
        match self.status {
            200..=299 => Ok(self),
            401 | 403 => Err(HubError::Denied {
                service: service.to_owned(),
                status: self.status,
            }),
            404 => Err(HubError::NotFound {
                what: what.to_owned(),
            }),
            status => {
                let detail = self.bytes(service).map(|b| excerpt(&b)).unwrap_or_default();
                Err(HubError::Upstream {
                    service: service.to_owned(),
                    status,
                    detail,
                })
            }
        }
    }
}

/// First 200 characters of an error body, for messages.
fn excerpt(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).chars().take(200).collect()
}

/// Sends requests. Network and timeout failures come back as `HubError::Unavailable`.
pub trait HttpPort: Send + Sync {
    fn send(&self, service: &str, request: HttpRequest) -> Result<HttpResponse, HubError>;
}

/// Keeps the credentials between launches.
pub trait SecretStore: Send + Sync {
    fn load(&self) -> Result<Option<Credentials>, HubError>;
    fn save(&self, credentials: &Credentials) -> Result<(), HubError>;
    fn clear(&self) -> Result<(), HubError>;
}

/// Verifies credentials and says which apps the account may open.
pub trait Directory: Send + Sync {
    fn identify(&self, credentials: &Credentials) -> Result<Me, HubError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_body_is_percent_encoded() {
        let request = HttpRequest::post("https://x").form(&[("user", "a b"), ("pw", "p&=é")]);
        assert_eq!(
            request.body,
            Body::Bytes(b"user=a%20b&pw=p%26%3D%C3%A9".to_vec())
        );
    }

    #[test]
    fn status_mapping() {
        let denied = HttpResponse::from_bytes(403, vec![], vec![]).ok("S", "thing");
        assert!(matches!(denied, Err(HubError::Denied { status: 403, .. })));
        let missing = HttpResponse::from_bytes(404, vec![], vec![]).ok("S", "thing");
        assert!(matches!(missing, Err(HubError::NotFound { .. })));
        let broken = HttpResponse::from_bytes(500, vec![], b"boom".to_vec()).ok("S", "thing");
        assert!(
            matches!(broken, Err(HubError::Upstream { status: 500, ref detail, .. }) if detail == "boom")
        );
    }

    #[test]
    fn header_lookup_ignores_case() {
        let response =
            HttpResponse::from_bytes(200, vec![("content-type".into(), "x/y".into())], vec![]);
        assert_eq!(response.header("Content-Type"), Some("x/y"));
    }
}
