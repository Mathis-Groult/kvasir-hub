//! Module behaviour against a fake Seafile shaped like the real answers captured on 2026-10-07.

use super::*;
use hub_core::{Credentials, HttpPort, HttpResponse};
use std::collections::BTreeMap;
use std::io::Read;
use std::sync::{Arc, Mutex};

const REPO: &str = "262360c8-cdb2-4d92-8c6c-b6ee01aa549c";

#[derive(Clone)]
struct Seen {
    method: Method,
    url: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

/// One canned answer: URL substring, status, headers, body.
type Route = (&'static str, u16, Vec<(String, String)>, Vec<u8>);

/// Routes by URL substring; every request is recorded.
struct FakeSeafile {
    routes: Vec<Route>,
    seen: Mutex<Vec<Seen>>,
}

impl HttpPort for FakeSeafile {
    fn send(&self, _: &str, request: HttpRequest) -> Result<HttpResponse, HubError> {
        let body = match &request.body {
            hub_core::Body::Bytes(b) => b.clone(),
            hub_core::Body::Empty => vec![],
        };
        self.seen.lock().unwrap().push(Seen {
            method: request.method,
            url: request.url.clone(),
            headers: request.headers.clone(),
            body,
        });
        if request.url.ends_with("/api2/auth-token/") {
            return Ok(HttpResponse::from_bytes(
                200,
                vec![],
                br#"{"token":"t0k"}"#.to_vec(),
            ));
        }
        let (_, status, headers, body) = self
            .routes
            .iter()
            .find(|(needle, ..)| request.url.contains(needle))
            .unwrap_or_else(|| panic!("no route for {}", request.url));
        Ok(HttpResponse::from_bytes(
            *status,
            headers.clone(),
            body.clone(),
        ))
    }
}

fn route(needle: &'static str, status: u16, body: &str) -> Route {
    (needle, status, vec![], body.as_bytes().to_vec())
}

fn module(routes: Vec<Route>) -> (Box<dyn Module>, Arc<FakeSeafile>) {
    let fake = Arc::new(FakeSeafile {
        routes,
        seen: Mutex::new(vec![]),
    });
    let built = (definition("https://files.example").factory)(
        fake.clone(),
        Credentials::new("alice", "pw"),
    );
    (built, fake)
}

fn last(fake: &FakeSeafile) -> Seen {
    fake.seen.lock().unwrap().last().cloned().unwrap()
}

#[test]
fn overview_joins_account_quota_and_libraries() {
    let (files, _) = module(vec![
        route(
            "/api2/account/info/",
            200,
            r#"{"name":"hubtest","usage":1200,"total":500000000}"#,
        ),
        route(
            "/api/v2.1/repos/",
            200,
            &format!(
                r#"{{"repos":[{{"type":"mine","repo_id":"{REPO}","repo_name":"My Library","owner_name":"hubtest","size":0,"last_modified":"2026-10-07T15:36:26+00:00","permission":"rw","encrypted":false}}]}}"#
            ),
        ),
    ]);
    let out = files.call("overview", &json!({})).unwrap();
    assert_eq!(
        out["account"],
        json!({"name":"hubtest","used":1200,"quota":500000000})
    );
    assert_eq!(out["libraries"][0]["name"], "My Library");
    assert_eq!(out["libraries"][0]["writable"], true);
}

#[test]
fn a_user_without_a_quota_gets_null() {
    let (files, _) = module(vec![
        route(
            "/api2/account/info/",
            200,
            r#"{"name":"a","usage":0,"total":-2}"#,
        ),
        route("/api/v2.1/repos/", 200, r#"{"repos":[]}"#),
    ]);
    assert_eq!(
        files.call("overview", &json!({})).unwrap()["account"]["quota"],
        Value::Null
    );
}

#[test]
fn ls_requests_the_encoded_path_with_the_token() {
    let (files, fake) = module(vec![route(
        "/dir/?p=",
        200,
        r#"{"dirent_list":[{"type":"dir","name":"Docs","mtime":1,"parent_dir":"/My Files"}]}"#,
    )]);
    let out = files
        .call("ls", &json!({"repo": REPO, "path": "/My Files"}))
        .unwrap();
    assert_eq!(out[0]["path"], "/My Files/Docs");
    let seen = last(&fake);
    assert!(
        seen.url.contains("p=%2FMy%20Files&with_thumbnail=true"),
        "{}",
        seen.url
    );
    assert!(
        seen.headers
            .contains(&("Authorization".to_owned(), "Token t0k".to_owned()))
    );
}

#[test]
fn arguments_that_could_escape_are_refused_before_any_request() {
    let (files, fake) = module(vec![]);
    for args in [
        json!({"repo": "../x", "path": "/"}),
        json!({"repo": REPO, "path": "relative"}),
        json!({"repo": REPO, "path": "/a/../b"}),
    ] {
        assert_eq!(
            files.call("ls", &args).unwrap_err().code(),
            "bad_args",
            "{args}"
        );
    }
    assert!(fake.seen.lock().unwrap().is_empty());
}

#[test]
fn mkdir_posts_the_operation() {
    let (files, fake) = module(vec![route("/dir/?p=", 201, r#""success""#)]);
    files
        .call("mkdir", &json!({"repo": REPO, "path": "/Docs"}))
        .unwrap();
    let seen = last(&fake);
    assert_eq!(
        (seen.method, seen.body.as_slice()),
        (Method::Post, b"operation=mkdir".as_slice())
    );
}

#[test]
fn rename_accepts_seafiles_301_success_and_picks_file_or_dir() {
    let (files, fake) = module(vec![
        route("/file/?p=", 301, r#""success""#),
        route("/dir/?p=", 301, r#""success""#),
    ]);
    files
        .call(
            "rename",
            &json!({"repo": REPO, "path": "/a.txt", "name": "b.txt"}),
        )
        .unwrap();
    assert!(last(&fake).url.contains("/file/?p=%2Fa.txt"));
    assert_eq!(last(&fake).body, b"operation=rename&newname=b.txt");
    files
        .call(
            "rename",
            &json!({"repo": REPO, "path": "/old", "name": "new", "isDir": true}),
        )
        .unwrap();
    assert!(last(&fake).url.contains("/dir/?p=%2Fold"));
}

#[test]
fn rename_refuses_names_with_separators() {
    let (files, _) = module(vec![]);
    let err = files
        .call(
            "rename",
            &json!({"repo": REPO, "path": "/a", "name": "x/y"}),
        )
        .unwrap_err();
    assert_eq!(err.code(), "bad_args");
}

#[test]
fn delete_uses_the_delete_verb_and_never_the_library_root() {
    let (files, fake) = module(vec![route("/file/?p=", 200, r#""success""#)]);
    files
        .call("delete", &json!({"repo": REPO, "path": "/a.txt"}))
        .unwrap();
    assert_eq!(last(&fake).method, Method::Delete);
    let err = files
        .call("delete", &json!({"repo": REPO, "path": "/", "isDir": true}))
        .unwrap_err();
    assert_eq!(err.code(), "bad_args");
}

#[test]
fn upload_fetches_a_link_then_posts_multipart_without_the_token() {
    let (files, fake) = module(vec![
        route(
            "/upload-link/",
            200,
            r#""https://files.example/seafhttp/upload-api/abc""#,
        ),
        route(
            "/seafhttp/upload-api/abc",
            200,
            r#"[{"name":"hello.txt","id":"x","size":10}]"#,
        ),
    ]);
    let out = files
        .upload(
            "put",
            &json!({"repo": REPO, "dir": "/Docs", "name": "hello.txt"}),
            b"hello hub\n".to_vec(),
        )
        .unwrap();
    assert_eq!(out, json!([{"name":"hello.txt","size":10}]));
    let seen = last(&fake);
    assert_eq!(
        seen.url,
        "https://files.example/seafhttp/upload-api/abc?ret-json=1"
    );
    assert!(!seen.headers.iter().any(|(k, _)| k == "Authorization"));
    let body = String::from_utf8(seen.body).unwrap();
    assert!(body.contains("name=\"parent_dir\"\r\n\r\n/Docs") && body.contains("hello hub\n"));
}

#[test]
fn download_resolves_the_link_then_forwards_range() {
    let (files, fake) = module(vec![
        route(
            "/api2/repos/",
            200,
            r#""https://files.example/seafhttp/files/u/hello.txt""#,
        ),
        (
            "/seafhttp/files/u/hello.txt",
            206,
            vec![
                ("content-type".into(), "text/plain".into()),
                ("content-range".into(), "bytes 0-4/10".into()),
                ("content-length".into(), "5".into()),
            ],
            b"hello".to_vec(),
        ),
    ]);
    let request = MediaRequest {
        op: "download".into(),
        params: BTreeMap::from([
            ("repo".into(), REPO.into()),
            ("path".into(), "/hello.txt".into()),
        ]),
        range: Some("bytes=0-4".into()),
    };
    let mut response = files.media(&request).unwrap();
    assert_eq!(
        (response.status, response.file_name.as_deref()),
        (206, Some("hello.txt"))
    );
    assert_eq!(response.content_range.as_deref(), Some("bytes 0-4/10"));
    let mut bytes = Vec::new();
    response.body.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"hello");
    let seen = last(&fake);
    assert!(
        seen.headers
            .contains(&("Range".to_owned(), "bytes=0-4".to_owned()))
    );
    assert!(!seen.headers.iter().any(|(k, _)| k == "Authorization"));
}

#[test]
fn thumbnails_use_the_token_and_a_default_size() {
    let (files, fake) = module(vec![(
        "/thumbnail/",
        200,
        vec![("content-type".into(), "image/png".into())],
        vec![1, 2],
    )]);
    let request = MediaRequest {
        op: "thumb".into(),
        params: BTreeMap::from([
            ("repo".into(), REPO.into()),
            ("path".into(), "/p.png".into()),
        ]),
        range: None,
    };
    let response = files.media(&request).unwrap();
    assert_eq!(response.content_type, "image/png");
    let seen = last(&fake);
    assert!(seen.url.ends_with("/thumbnail/?p=%2Fp.png&size=96"));
    assert!(
        seen.headers
            .contains(&("Authorization".to_owned(), "Token t0k".to_owned()))
    );
}

#[test]
fn a_missing_folder_is_not_found() {
    let (files, _) = module(vec![route("/dir/?p=", 404, "")]);
    assert_eq!(
        files
            .call("ls", &json!({"repo": REPO, "path": "/nope"}))
            .unwrap_err()
            .code(),
        "not_found"
    );
}

#[test]
fn the_manifest_id_is_the_kvasir_app_id() {
    assert_eq!(definition("https://x").manifest.id, "files");
}
