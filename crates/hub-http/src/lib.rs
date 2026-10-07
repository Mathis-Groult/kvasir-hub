//! `HttpPort` over `ureq` + rustls.
//!
//! Hides: the HTTP library, TLS roots, timeouts and the redirect policy. Replacing the
//! library touches this file only.

use hub_core::{Body, HttpPort, HttpRequest, HttpResponse, HubError, Method};
use std::time::Duration;

/// Time to open a connection and send the request headers.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Time to the first byte of the answer; long enough for a cold Seafile or Navidrome.
const FIRST_BYTE_TIMEOUT: Duration = Duration::from_secs(30);

/// Production HTTP client. Cheap to clone-share behind an `Arc`.
pub struct UreqHttp {
    agent: ureq::Agent,
}

impl UreqHttp {
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            // Statuses are mapped by `HttpResponse::ok`, not by the library.
            .http_status_as_error(false)
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_recv_response(Some(FIRST_BYTE_TIMEOUT))
            .max_redirects(0)
            .user_agent("KvasirHub/0.1")
            .build();
        Self {
            agent: ureq::Agent::new_with_config(config),
        }
    }
}

impl Default for UreqHttp {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpPort for UreqHttp {
    fn send(&self, service: &str, request: HttpRequest) -> Result<HttpResponse, HubError> {
        let mut builder = ureq::http::Request::builder()
            .method(match request.method {
                Method::Get => "GET",
                Method::Post => "POST",
                Method::Put => "PUT",
                Method::Delete => "DELETE",
            })
            .uri(&request.url);
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        let body = match request.body {
            Body::Empty => Vec::new(),
            Body::Bytes(bytes) => bytes,
        };
        let http_request = builder
            .body(body)
            .map_err(|e| HubError::unavailable(service, format!("bad request: {e}")))?;
        let response = self
            .agent
            .run(http_request)
            .map_err(|e| HubError::unavailable(service, e))?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(k, v)| Some((k.as_str().to_owned(), v.to_str().ok()?.to_owned())))
            .collect();
        let reader = response.into_body().into_reader();
        Ok(HttpResponse {
            status,
            headers,
            body: Box::new(reader),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// One-shot local server answering a fixed HTTP response.
    fn serve_once(response: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut buffer = [0u8; 2048];
            let _ = socket.read(&mut buffer);
            socket.write_all(response.as_bytes()).unwrap();
        });
        format!("http://{addr}/")
    }

    #[test]
    fn returns_status_headers_and_body_without_treating_404_as_an_error() {
        let url = serve_once(
            "HTTP/1.1 404 Not Found\r\nContent-Length: 4\r\nX-Test: yes\r\nConnection: close\r\n\r\nnope",
        );
        let response = UreqHttp::new().send("T", HttpRequest::get(url)).unwrap();
        assert_eq!(response.status, 404);
        assert_eq!(response.header("x-test"), Some("yes"));
        assert_eq!(response.bytes("T").unwrap(), b"nope");
    }

    #[test]
    fn refused_connection_is_unavailable() {
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let err = UreqHttp::new()
            .send("T", HttpRequest::get(format!("http://127.0.0.1:{port}/")))
            .err()
            .expect("closed port must fail");
        assert_eq!(err.code(), "unavailable");
    }

    #[test]
    fn redirects_are_surfaced_not_followed() {
        let url = serve_once(
            "HTTP/1.1 302 Found\r\nLocation: https://elsewhere/login\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        );
        let response = UreqHttp::new().send("T", HttpRequest::get(url)).unwrap();
        assert_eq!(response.status, 302);
        assert_eq!(response.header("location"), Some("https://elsewhere/login"));
    }
}
