//! `Directory` backed by the `hub-api` server (`GET /hub/v1/me`, HTTP Basic).
//!
//! Hides: the sign-in wire format and how refusals map to `HubError`.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hub_core::{Credentials, Directory, HttpPort, HttpRequest, HubError, Refusal};
use hub_wire::{Me, Refusal as WireRefusal};
use std::sync::Arc;

const SERVICE: &str = "Kvasir sign-in";

/// Asks `hub-api` who an account is.
pub struct ApiDirectory {
    http: Arc<dyn HttpPort>,
    base_url: String,
}

impl ApiDirectory {
    /// `base_url` is e.g. `https://example.org/hub` (no trailing slash).
    pub fn new(http: Arc<dyn HttpPort>, base_url: impl Into<String>) -> Self {
        Self {
            http,
            base_url: base_url.into(),
        }
    }
}

/// `Authorization` header value for HTTP Basic.
pub fn basic_header(credentials: &Credentials) -> String {
    let raw = format!("{}:{}", credentials.username, credentials.password);
    format!("Basic {}", STANDARD.encode(raw))
}

impl Directory for ApiDirectory {
    fn identify(&self, credentials: &Credentials) -> Result<Me, HubError> {
        let request = HttpRequest::get(format!("{}/v1/me", self.base_url))
            .header("Authorization", basic_header(credentials));
        let response = self.http.send(SERVICE, request)?;
        match response.status {
            200 => response.json(SERVICE),
            401 | 403 | 423 => {
                let refusal: WireRefusal = response.json(SERVICE)?;
                Err(HubError::Refused(match refusal.outcome.as_str() {
                    "pending" => Refusal::Pending,
                    "disabled" => Refusal::Disabled,
                    "locked" => Refusal::Locked {
                        until: refusal.locked_until,
                    },
                    _ => Refusal::Invalid,
                }))
            }
            status => Err(response
                .ok(SERVICE, "sign-in")
                .err()
                .unwrap_or(HubError::Upstream {
                    service: SERVICE.to_owned(),
                    status,
                    detail: String::new(),
                })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hub_core::HttpResponse;
    use std::sync::Mutex;

    /// Replays one canned answer and remembers the request.
    struct Canned {
        status: u16,
        body: &'static str,
        seen: Mutex<Option<HttpRequest>>,
    }

    impl Canned {
        fn new(status: u16, body: &'static str) -> Arc<Self> {
            Arc::new(Self {
                status,
                body,
                seen: Mutex::new(None),
            })
        }
    }

    impl HttpPort for Canned {
        fn send(&self, _: &str, request: HttpRequest) -> Result<HttpResponse, HubError> {
            *self.seen.lock().unwrap() = Some(request);
            Ok(HttpResponse::from_bytes(
                self.status,
                vec![],
                self.body.as_bytes().to_vec(),
            ))
        }
    }

    fn identify(http: Arc<Canned>) -> Result<Me, HubError> {
        ApiDirectory::new(http, "https://h/hub").identify(&Credentials::new("alice", "pw"))
    }

    #[test]
    fn sends_basic_auth_to_the_me_endpoint() {
        let http = Canned::new(
            200,
            r#"{"username":"alice","role":"user","apps":["files"]}"#,
        );
        let me = identify(Arc::clone(&http)).unwrap();
        assert_eq!(me.apps, vec!["files"]);
        let seen = http.seen.lock().unwrap().clone().unwrap();
        assert_eq!(seen.url, "https://h/hub/v1/me");
        // base64("alice:pw")
        assert_eq!(
            seen.headers,
            vec![("Authorization".to_owned(), "Basic YWxpY2U6cHc=".to_owned())]
        );
    }

    #[test]
    fn maps_every_refusal_outcome() {
        let cases = [
            (401, r#"{"outcome":"invalid"}"#, "bad_credentials"),
            (401, r#"{"outcome":"pending"}"#, "pending"),
            (401, r#"{"outcome":"disabled"}"#, "disabled"),
            (423, r#"{"outcome":"locked","locked_until":99}"#, "locked"),
        ];
        for (status, body, code) in cases {
            let err = identify(Canned::new(status, body)).unwrap_err();
            assert_eq!(err.code(), code, "body {body}");
        }
    }

    #[test]
    fn lockout_deadline_reaches_the_ui() {
        let err = identify(Canned::new(
            423,
            r#"{"outcome":"locked","locked_until":99}"#,
        ))
        .unwrap_err();
        assert_eq!(err.view().locked_until, Some(99));
    }

    #[test]
    fn server_error_is_upstream_and_retryable() {
        let err = identify(Canned::new(502, "bad gateway")).unwrap_err();
        assert_eq!(err.code(), "upstream");
        assert!(err.view().retryable);
    }
}
