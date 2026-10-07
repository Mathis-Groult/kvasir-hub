//! The Subsonic wire protocol: URL building, auth parameters, envelope and error codes.
//!
//! Hides: everything Subsonic-specific. The rest of the module speaks in methods and DTOs.

use hub_core::{Credentials, HttpPort, HttpRequest, HttpResponse, HubError, percent_encode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;

pub(crate) const SERVICE: &str = "Music";
const API_VERSION: &str = "1.16.1";
const CLIENT_NAME: &str = "KvasirHub";
/// Subsonic error code for bad credentials.
const ERR_WRONG_CREDENTIALS: i64 = 40;
/// Subsonic error code for "data not found".
const ERR_NOT_FOUND: i64 = 70;

/// Authenticated access to one Subsonic server.
pub(crate) struct Subsonic {
    http: Arc<dyn HttpPort>,
    rest_url: String,
    user: String,
    /// `enc:` + hex of the password: the "legacy" form that works with Kvasir's credential gate.
    password: String,
}

impl Subsonic {
    pub(crate) fn new(http: Arc<dyn HttpPort>, rest_url: &str, credentials: &Credentials) -> Self {
        let hex: String = credentials
            .password
            .bytes()
            .map(|b| format!("{b:02x}"))
            .collect();
        Self {
            http,
            rest_url: rest_url.trim_end_matches('/').to_owned(),
            user: credentials.username.clone(),
            password: format!("enc:{hex}"),
        }
    }

    fn url(&self, method: &str, params: &[(&str, &str)]) -> String {
        let mut url = format!(
            "{}/{method}?u={}&p={}&v={API_VERSION}&c={CLIENT_NAME}&f=json",
            self.rest_url,
            percent_encode(&self.user),
            self.password
        );
        for (key, value) in params {
            url.push('&');
            url.push_str(&percent_encode(key));
            url.push('=');
            url.push_str(&percent_encode(value));
        }
        url
    }

    /// Calls a JSON method and deserialises the `subsonic-response` object into `T`.
    pub(crate) fn call<T: DeserializeOwned>(
        &self,
        method: &str,
        params: &[(&str, &str)],
    ) -> Result<T, HubError> {
        let response = self
            .http
            .send(SERVICE, HttpRequest::get(self.url(method, params)))?
            .ok(SERVICE, method)?;
        let envelope: Value = response.json(SERVICE)?;
        let inner = envelope
            .get("subsonic-response")
            .ok_or_else(|| HubError::malformed(SERVICE, "no subsonic-response object"))?;
        if inner.get("status").and_then(Value::as_str) != Some("ok") {
            return Err(error_from(inner, method));
        }
        serde_json::from_value(inner.clone()).map_err(|e| HubError::malformed(SERVICE, e))
    }

    /// Calls a method whose answer is bytes (`stream`, `getCoverArt`), forwarding `Range`.
    pub(crate) fn raw(
        &self,
        method: &str,
        params: &[(&str, &str)],
        range: Option<&str>,
    ) -> Result<HttpResponse, HubError> {
        let mut request = HttpRequest::get(self.url(method, params));
        if let Some(range) = range {
            request = request.header("Range", range);
        }
        self.http.send(SERVICE, request)?.ok(SERVICE, method)
    }
}

/// Maps a failed envelope to the matching [`HubError`].
fn error_from(inner: &Value, method: &str) -> HubError {
    let error = inner.get("error");
    let code = error
        .and_then(|e| e.get("code"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let message = error
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("unknown error")
        .to_owned();
    match code {
        ERR_WRONG_CREDENTIALS => HubError::Denied {
            service: SERVICE.to_owned(),
            status: 401,
        },
        ERR_NOT_FOUND => HubError::NotFound {
            what: method.to_owned(),
        },
        _ => HubError::Upstream {
            service: SERVICE.to_owned(),
            status: 200,
            detail: message,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct Canned {
        body: &'static str,
        seen: Mutex<Vec<HttpRequest>>,
    }

    impl HttpPort for Canned {
        fn send(&self, _: &str, request: HttpRequest) -> Result<HttpResponse, HubError> {
            self.seen.lock().unwrap().push(request);
            Ok(HttpResponse::from_bytes(
                200,
                vec![],
                self.body.as_bytes().to_vec(),
            ))
        }
    }

    fn client(body: &'static str) -> (Subsonic, Arc<Canned>) {
        let http = Arc::new(Canned {
            body,
            seen: Mutex::new(vec![]),
        });
        let creds = Credentials::new("alice", "pw é");
        (
            Subsonic::new(http.clone(), "https://m/music/rest/", &creds),
            http,
        )
    }

    #[test]
    fn url_carries_legacy_auth_and_encoded_params() {
        let (client, _) = client("{}");
        // "pw é" in UTF-8 hex: 70 77 20 c3 a9
        assert_eq!(
            client.url("search3", &[("query", "a b&c")]),
            "https://m/music/rest/search3?u=alice&p=enc:707720c3a9&v=1.16.1&c=KvasirHub&f=json&query=a%20b%26c"
        );
    }

    #[test]
    fn ok_envelope_is_deserialised() {
        #[derive(serde::Deserialize)]
        struct Pong {
            version: String,
        }
        let (client, _) = client(r#"{"subsonic-response":{"status":"ok","version":"1.16.1"}}"#);
        let pong: Pong = client.call("ping", &[]).unwrap();
        assert_eq!(pong.version, "1.16.1");
    }

    #[test]
    fn wrong_credentials_become_denied() {
        let (client, _) = client(
            r#"{"subsonic-response":{"status":"failed","error":{"code":40,"message":"Wrong username or password"}}}"#,
        );
        let err = client.call::<Value>("ping", &[]).unwrap_err();
        assert_eq!(err.code(), "denied");
    }

    #[test]
    fn missing_data_becomes_not_found_and_other_codes_upstream() {
        let (missing, _) = client(
            r#"{"subsonic-response":{"status":"failed","error":{"code":70,"message":"x"}}}"#,
        );
        assert_eq!(
            missing.call::<Value>("getAlbum", &[]).unwrap_err().code(),
            "not_found"
        );
        let (broken, _) = client(
            r#"{"subsonic-response":{"status":"failed","error":{"code":0,"message":"boom"}}}"#,
        );
        let err = broken.call::<Value>("getAlbum", &[]).unwrap_err();
        assert_eq!(err.to_string(), "Music answered HTTP 200: boom");
    }

    #[test]
    fn an_envelope_less_answer_is_malformed() {
        let (client, _) = client("{}");
        assert_eq!(
            client.call::<Value>("ping", &[]).unwrap_err().code(),
            "malformed"
        );
    }

    #[test]
    fn raw_forwards_the_range_header() {
        let (client, http) = client("");
        client
            .raw("stream", &[("id", "s1")], Some("bytes=0-99"))
            .unwrap();
        let seen = http.seen.lock().unwrap();
        assert_eq!(
            seen[0].headers,
            vec![("Range".to_owned(), "bytes=0-99".to_owned())]
        );
    }
}
