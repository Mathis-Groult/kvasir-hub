use super::*;
use hub_core::{
    Credentials, Directory, HttpPort, HttpRequest, HttpResponse, Manifest, MediaRequest,
    MediaResponse, Module, ModuleDef, Ports, SecretStore,
};
use hub_wire::{Me, Role};
use serde_json::Value;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Mutex;

struct NoNet;
impl HttpPort for NoNet {
    fn send(&self, s: &str, _: HttpRequest) -> Result<HttpResponse, HubError> {
        Err(HubError::unavailable(s, "offline"))
    }
}

struct NoSecrets;
impl SecretStore for NoSecrets {
    fn load(&self) -> Result<Option<Credentials>, HubError> {
        Ok(None)
    }
    fn save(&self, _: &Credentials) -> Result<(), HubError> {
        Ok(())
    }
    fn clear(&self) -> Result<(), HubError> {
        Ok(())
    }
}

struct Dir;
impl Directory for Dir {
    fn identify(&self, c: &Credentials) -> Result<Me, HubError> {
        Ok(Me {
            username: c.username.clone(),
            role: Role::User,
            apps: vec!["blob".into()],
        })
    }
}

/// 100 bytes "0123456789" repeated; honours Range like a real server.
struct Blob {
    seen: Mutex<Vec<Option<String>>>,
}
impl Module for Blob {
    fn call(&self, _: &str, _: &Value) -> Result<Value, HubError> {
        Ok(Value::Null)
    }
    fn media(&self, request: &MediaRequest) -> Result<MediaResponse, HubError> {
        self.seen.lock().unwrap().push(request.range.clone());
        let all: Vec<u8> = (0..100u8).map(|i| b'0' + i % 10).collect();
        let (status, body, content_range) = match request.range.as_deref() {
            Some("bytes=10-19") => (
                206,
                all[10..20].to_vec(),
                Some("bytes 10-19/100".to_owned()),
            ),
            _ => (200, all, None),
        };
        Ok(MediaResponse {
            status,
            content_type: "audio/flac".into(),
            content_length: Some(body.len() as u64),
            content_range,
            file_name: (request.op == "download").then(|| "blob é.bin".to_owned()),
            body: Box::new(Cursor::new(body)),
        })
    }
}

fn signed_in_hub() -> Arc<Hub> {
    let def = ModuleDef::new(
        Manifest {
            id: "blob",
            title: "Blob",
            summary: "",
        },
        Box::new(|_, _| {
            Box::new(Blob {
                seen: Mutex::new(vec![]),
            })
        }),
    );
    let hub = Hub::new(
        vec![def],
        Ports {
            http: Arc::new(NoNet),
            secrets: Arc::new(NoSecrets),
            directory: Arc::new(Dir),
        },
    );
    hub.sign_in(Credentials::new("a", "b")).unwrap();
    Arc::new(hub)
}

/// Raw HTTP/1.1 GET returning (status line, headers lowercased, body).
fn get(base: &str, path: &str, host: Option<&str>, extra: &str) -> (String, String, Vec<u8>) {
    let authority = base
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap()
        .to_owned();
    let mut stream = TcpStream::connect(&authority).unwrap();
    let host = host.unwrap_or(&authority);
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {host}\r\n{extra}Connection: close\r\n\r\n"
    )
    .unwrap();
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).unwrap();
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let (status, headers) = head.split_once("\r\n").unwrap_or((&head, ""));
    (
        status.to_owned(),
        headers.to_lowercase(),
        raw[split + 4..].to_vec(),
    )
}

fn path_of(base: &str, rest: &str) -> String {
    let token_path = base.splitn(4, '/').nth(3).unwrap();
    format!("/{token_path}{rest}")
}

#[test]
fn media_streams_with_the_right_headers() {
    let server = spawn(signed_in_hub()).unwrap();
    let (status, headers, body) = get(
        server.base_url(),
        &path_of(server.base_url(), "/blob/stream?id=1"),
        None,
        "",
    );
    assert!(status.contains("200"), "{status}");
    assert!(
        headers.contains("content-type: audio/flac") && headers.contains("accept-ranges: bytes")
    );
    assert!(headers.contains("cache-control: private, no-store"));
    assert_eq!(body.len(), 100);
}

#[test]
fn a_range_request_gets_206_and_exactly_those_bytes() {
    let server = spawn(signed_in_hub()).unwrap();
    let (status, headers, body) = get(
        server.base_url(),
        &path_of(server.base_url(), "/blob/stream?id=1"),
        None,
        "Range: bytes=10-19\r\n",
    );
    assert!(status.contains("206"), "{status}");
    assert!(
        headers.contains("content-range: bytes 10-19/100"),
        "{headers}"
    );
    assert_eq!(body, b"0123456789");
}

#[test]
fn a_wrong_token_is_a_plain_404() {
    let server = spawn(signed_in_hub()).unwrap();
    let authority = server
        .base_url()
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap()
        .to_owned();
    let (status, _, _) = get(
        server.base_url(),
        "/deadbeef/blob/stream?id=1",
        Some(&authority),
        "",
    );
    assert!(status.contains("404"), "{status}");
    let (status, _, _) = get(server.base_url(), "/blob/stream?id=1", None, "");
    assert!(status.contains("404"), "{status}");
}

#[test]
fn a_foreign_host_header_is_refused_even_with_the_right_token() {
    let server = spawn(signed_in_hub()).unwrap();
    let (status, _, _) = get(
        server.base_url(),
        &path_of(server.base_url(), "/blob/stream?id=1"),
        Some("evil.example"),
        "",
    );
    assert!(status.contains("404"), "{status}");
}

#[test]
fn downloads_carry_an_ascii_content_disposition() {
    let server = spawn(signed_in_hub()).unwrap();
    let (_, headers, _) = get(
        server.base_url(),
        &path_of(server.base_url(), "/blob/download?id=1"),
        None,
        "",
    );
    assert!(
        headers.contains("filename*=utf-8''blob%20%c3%a9.bin"),
        "{headers}"
    );
}

#[test]
fn unknown_modules_and_bad_targets_map_to_http_errors() {
    let server = spawn(signed_in_hub()).unwrap();
    let (status, _, _) = get(
        server.base_url(),
        &path_of(server.base_url(), "/nope/stream"),
        None,
        "",
    );
    assert!(status.contains("404"), "{status}");
    let (status, _, _) = get(
        server.base_url(),
        &path_of(server.base_url(), "/blob"),
        None,
        "",
    );
    assert!(status.contains("400"), "{status}");
}

#[test]
fn every_start_gets_a_different_token_of_32_hex_characters() {
    let a = spawn(signed_in_hub()).unwrap();
    let b = spawn(signed_in_hub()).unwrap();
    let token = |s: &MediaServer| s.base_url().rsplit('/').next().unwrap().to_owned();
    assert_eq!(token(&a).len(), 32);
    assert!(token(&a).bytes().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(token(&a), token(&b));
}
