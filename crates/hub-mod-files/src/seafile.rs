//! Seafile REST access with a cached API token.
//!
//! Hides: how a native client authenticates (`/api2/auth-token/`, device id rules, the
//! once-only refresh on 401/403) and which API generation serves which call.

use hub_core::{
    Credentials, HttpPort, HttpRequest, HttpResponse, HubError, Method, percent_encode,
};
use serde::de::DeserializeOwned;
use std::sync::{Arc, Mutex};

pub(crate) const SERVICE: &str = "Files";
/// Seafile wants a 40-hex device id with platform and device name, all or none.
const DEVICE_ID: &[u8; 20] = b"Kvasir-hub-client-00";

pub(crate) struct Seafile {
    http: Arc<dyn HttpPort>,
    base_url: String,
    credentials: Credentials,
    token: Mutex<Option<String>>,
}

fn device_id() -> String {
    DEVICE_ID.iter().map(|b| format!("{b:02x}")).collect()
}

impl Seafile {
    pub(crate) fn new(http: Arc<dyn HttpPort>, base_url: &str, credentials: Credentials) -> Self {
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_owned(),
            credentials,
            token: Mutex::new(None),
        }
    }

    /// Full URL of an API path such as `/api2/account/info/`.
    pub(crate) fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    fn login(&self) -> Result<String, HubError> {
        let request = HttpRequest::post(self.url("/api2/auth-token/")).form(&[
            ("username", &self.credentials.username),
            ("password", &self.credentials.password),
            ("platform", "linux"),
            ("device_id", &device_id()),
            ("device_name", "Kvasir Hub"),
        ]);
        let response = self.http.send(SERVICE, request)?;
        match response.status {
            // The Kvasir gate answers 400 `non_field_errors` for a rejected login.
            400 | 401 | 403 => Err(HubError::Denied {
                service: SERVICE.to_owned(),
                status: response.status,
            }),
            _ => {
                #[derive(serde::Deserialize)]
                struct Token {
                    token: String,
                }
                let token: Token = response.ok(SERVICE, "sign-in")?.json(SERVICE)?;
                Ok(token.token)
            }
        }
    }

    fn token(&self, refresh: bool) -> Result<String, HubError> {
        let mut guard = self.token.lock().unwrap_or_else(|e| e.into_inner());
        if refresh {
            *guard = None;
        }
        if let Some(token) = guard.as_ref() {
            return Ok(token.clone());
        }
        let token = self.login()?;
        *guard = Some(token.clone());
        Ok(token)
    }

    /// Sends an authenticated request; one 401/403 triggers exactly one token refresh.
    pub(crate) fn send(&self, build: &dyn Fn() -> HttpRequest) -> Result<HttpResponse, HubError> {
        for refresh in [false, true] {
            let token = self.token(refresh)?;
            let response = self.http.send(
                SERVICE,
                build().header("Authorization", format!("Token {token}")),
            )?;
            if !matches!(response.status, 401 | 403) || refresh {
                return Ok(response);
            }
        }
        unreachable!("the loop returns on its second pass")
    }

    /// Sends a request to a URL that carries its own one-time token (upload and download links).
    pub(crate) fn send_unauthenticated(
        &self,
        request: HttpRequest,
    ) -> Result<HttpResponse, HubError> {
        self.http.send(SERVICE, request)
    }

    /// GET a JSON document.
    pub(crate) fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        what: &str,
    ) -> Result<T, HubError> {
        let url = self.url(path);
        self.send(&|| HttpRequest::get(url.clone()))?
            .ok(SERVICE, what)?
            .json(SERVICE)
    }

    /// Form POST / DELETE that answers `"success"`; `accept` lists extra OK statuses.
    pub(crate) fn act(
        &self,
        method: Method,
        path: &str,
        form: &[(&str, &str)],
        what: &str,
        accept: &[u16],
    ) -> Result<(), HubError> {
        let url = self.url(path);
        let response = self.send(&|| {
            let mut request = HttpRequest::new(method, url.clone());
            if !form.is_empty() {
                request = request.form(form);
            }
            request
        })?;
        if accept.contains(&response.status) {
            return Ok(());
        }
        response.ok(SERVICE, what).map(drop)
    }

    /// The URL Seafile returns as a bare JSON string (upload link, download link).
    pub(crate) fn link(&self, path: &str, what: &str) -> Result<String, HubError> {
        self.get_json::<String>(path, what)
    }
}

/// `?p=<path>` style query value.
pub(crate) fn q(value: &str) -> String {
    percent_encode(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Hands out numbered tokens; rejects requests that carry a token older than `valid_from`.
    struct FakeSeafile {
        logins: AtomicU32,
        valid_from: AtomicU32,
        reject_login: bool,
    }

    impl HttpPort for FakeSeafile {
        fn send(&self, _: &str, request: HttpRequest) -> Result<HttpResponse, HubError> {
            if request.url.ends_with("/api2/auth-token/") {
                if self.reject_login {
                    return Ok(HttpResponse::from_bytes(
                        400,
                        vec![],
                        br#"{"non_field_errors":["x"]}"#.to_vec(),
                    ));
                }
                let n = self.logins.fetch_add(1, Ordering::SeqCst) + 1;
                return Ok(HttpResponse::from_bytes(
                    200,
                    vec![],
                    format!(r#"{{"token":"tok{n}"}}"#).into_bytes(),
                ));
            }
            let sent = request
                .headers
                .iter()
                .find(|(k, _)| k == "Authorization")
                .map(|(_, v)| v.clone());
            let n: u32 = sent
                .as_deref()
                .and_then(|v| v.strip_prefix("Token tok"))
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
            let status = if n >= self.valid_from.load(Ordering::SeqCst) {
                200
            } else {
                403
            };
            Ok(HttpResponse::from_bytes(
                status,
                vec![],
                br#""ok""#.to_vec(),
            ))
        }
    }

    fn seafile(fake: Arc<FakeSeafile>) -> Seafile {
        Seafile::new(fake, "https://f/", Credentials::new("alice", "pw"))
    }

    fn fake(valid_from: u32, reject_login: bool) -> Arc<FakeSeafile> {
        Arc::new(FakeSeafile {
            logins: AtomicU32::new(0),
            valid_from: AtomicU32::new(valid_from),
            reject_login,
        })
    }

    #[test]
    fn device_id_is_forty_hex_characters() {
        let id = device_id();
        assert_eq!(id.len(), 40);
        assert!(id.bytes().all(|b| b.is_ascii_hexdigit()));
    }

    #[test]
    fn the_token_is_fetched_once_and_reused() {
        let fake = fake(0, false);
        let client = seafile(fake.clone());
        for _ in 0..3 {
            client.get_json::<String>("/api2/x/", "x").unwrap();
        }
        assert_eq!(fake.logins.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn an_expired_token_is_refreshed_exactly_once() {
        let fake = fake(2, false); // tok1 is stale, tok2 works
        let client = seafile(fake.clone());
        assert_eq!(client.get_json::<String>("/api2/x/", "x").unwrap(), "ok");
        assert_eq!(fake.logins.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_still_rejected_token_after_refresh_is_denied_not_looped() {
        let fake = fake(99, false);
        let err = seafile(fake.clone())
            .get_json::<String>("/api2/x/", "x")
            .unwrap_err();
        assert_eq!(err.code(), "denied");
        assert_eq!(fake.logins.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_rejected_login_is_denied() {
        let err = seafile(fake(0, true))
            .get_json::<String>("/api2/x/", "x")
            .unwrap_err();
        assert_eq!(err.code(), "denied");
    }
}
