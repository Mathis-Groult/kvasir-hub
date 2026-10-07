use super::*;
use hub_core::{
    Directory, HttpPort, HttpRequest, HttpResponse, Manifest, Module, ModuleDef, Ports, SecretStore,
};
use hub_wire::{Me, Role};
use std::sync::{Arc, Mutex};

struct NoNet;
impl HttpPort for NoNet {
    fn send(&self, s: &str, _: HttpRequest) -> Result<HttpResponse, HubError> {
        Err(HubError::unavailable(s, "offline"))
    }
}

#[derive(Default)]
struct Mem(Mutex<Option<Credentials>>);
impl SecretStore for Mem {
    fn load(&self) -> Result<Option<Credentials>, HubError> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, c: &Credentials) -> Result<(), HubError> {
        *self.0.lock().unwrap() = Some(c.clone());
        Ok(())
    }
    fn clear(&self) -> Result<(), HubError> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

struct Dir;
impl Directory for Dir {
    fn identify(&self, c: &Credentials) -> Result<Me, HubError> {
        if c.password == "pw" {
            Ok(Me {
                username: c.username.clone(),
                role: Role::User,
                apps: vec!["echo".into()],
            })
        } else {
            Err(HubError::Refused(hub_core::Refusal::Invalid))
        }
    }
}

struct Echo;
impl Module for Echo {
    fn call(&self, op: &str, args: &Value) -> Result<Value, HubError> {
        Ok(json!({"op": op, "args": args}))
    }
    fn upload(&self, _: &str, args: &Value, body: Vec<u8>) -> Result<Value, HubError> {
        Ok(json!({"args": args, "len": body.len()}))
    }
}

fn hub() -> Hub {
    let def = ModuleDef::new(
        Manifest {
            id: "echo",
            title: "Echo",
            summary: "Echoes.",
        },
        Box::new(|_, _| Box::new(Echo)),
    );
    Hub::new(
        vec![def],
        Ports {
            http: Arc::new(NoNet),
            secrets: Arc::new(Mem::default()),
            directory: Arc::new(Dir),
        },
    )
}

fn text(envelope: Envelope) -> String {
    envelope.to_json()
}

/// Parsed form: JSON object key order carries no meaning, so tests do not depend on it.
fn parsed(envelope: Envelope) -> Value {
    serde_json::from_str(&envelope.to_json()).unwrap()
}

fn signed_in() -> Hub {
    let hub = hub();
    dispatch(&hub, "sign_in", &json!({"username": "a", "password": "pw"}));
    hub
}

#[test]
fn sign_in_answers_with_the_account() {
    let out = parsed(dispatch(
        &hub(),
        "sign_in",
        &json!({"username": "alice", "password": "pw"}),
    ));
    assert_eq!(
        out,
        json!({"ok": true, "data": {"username": "alice", "role": "user", "apps": ["echo"]}})
    );
}

#[test]
fn a_refusal_becomes_a_structured_error() {
    let out = parsed(dispatch(
        &hub(),
        "sign_in",
        &json!({"username": "alice", "password": "bad"}),
    ));
    assert_eq!(
        out,
        json!({"ok": false, "error": {"code": "bad_credentials", "message": "Wrong username or password.", "retryable": false}})
    );
}

#[test]
fn modules_list_after_sign_in_only() {
    assert!(text(dispatch(&hub(), "modules", &json!({}))).contains("not_signed_in"));
    assert_eq!(
        parsed(dispatch(&signed_in(), "modules", &json!({}))),
        json!({"ok": true, "data": [{"id": "echo", "title": "Echo", "summary": "Echoes."}]})
    );
}

#[test]
fn call_routes_module_op_and_args() {
    let out = parsed(dispatch(
        &signed_in(),
        "call",
        &json!({"module": "echo", "op": "hi", "args": {"x": 1}}),
    ));
    assert_eq!(
        out,
        json!({"ok": true, "data": {"op": "hi", "args": {"x": 1}}})
    );
}

#[test]
fn call_without_args_passes_an_empty_object() {
    let out = parsed(dispatch(
        &signed_in(),
        "call",
        &json!({"module": "echo", "op": "hi"}),
    ));
    assert_eq!(out["data"]["args"], json!({}));
}

#[test]
fn restore_with_nothing_saved_is_null_data() {
    assert_eq!(
        parsed(dispatch(&hub(), "restore", &json!({}))),
        json!({"ok": true, "data": null})
    );
}

#[test]
fn unknown_commands_and_missing_fields_are_bad_args() {
    let hub = hub();
    assert!(text(dispatch(&hub, "explode", &json!({}))).contains("bad_args"));
    assert!(
        text(dispatch(&hub, "sign_in", &json!({"username": "a"})))
            .contains("'password' must be a string")
    );
}

#[test]
fn media_target_splits_module_op_params_and_range() {
    let (module, request) =
        parse_media_target("/music/stream?id=s%201&size=300", Some("bytes=0-")).unwrap();
    assert_eq!(module, "music");
    assert_eq!(request.op, "stream");
    assert_eq!(request.params.get("id").map(String::as_str), Some("s 1"));
    assert_eq!(request.range.as_deref(), Some("bytes=0-"));
}

#[test]
fn media_target_rejects_extra_or_missing_segments() {
    for bad in ["/music", "/music/stream/extra", ""] {
        assert!(parse_media_target(bad, None).is_err(), "{bad}");
    }
}

#[test]
fn query_decoding_handles_plus_utf8_and_broken_escapes() {
    let q = parse_query("a=x+y&b=%C3%A9&c=100%&d");
    assert_eq!(q["a"], "x y");
    assert_eq!(q["b"], "é");
    assert_eq!(q["c"], "100%");
    assert_eq!(q["d"], "");
}

#[test]
fn upload_hands_query_arguments_and_body_to_the_module() {
    let out = parsed(upload(
        &signed_in(),
        "/echo/put?repo=r&name=a%20b.txt",
        vec![0; 7],
    ));
    assert_eq!(
        out,
        json!({"ok": true, "data": {"args": {"name": "a b.txt", "repo": "r"}, "len": 7}})
    );
}

const CHUNK: u64 = 1000;

#[test]
fn a_missing_range_becomes_the_first_chunk() {
    assert_eq!(bounded_range(None, CHUNK), "bytes=0-999");
}

#[test]
fn open_ended_ranges_are_closed_at_one_chunk() {
    assert_eq!(bounded_range(Some("bytes=0-"), CHUNK), "bytes=0-999");
    assert_eq!(bounded_range(Some("bytes=5000-"), CHUNK), "bytes=5000-5999");
}

#[test]
fn closed_ranges_shrink_only_when_too_big() {
    assert_eq!(bounded_range(Some("bytes=10-19"), CHUNK), "bytes=10-19");
    assert_eq!(bounded_range(Some("bytes=100-99999"), CHUNK), "bytes=100-1099");
}

#[test]
fn suffix_ranges_are_capped() {
    assert_eq!(bounded_range(Some("bytes=-500"), CHUNK), "bytes=-500");
    assert_eq!(bounded_range(Some("bytes=-90000"), CHUNK), "bytes=-1000");
}

#[test]
fn unreadable_or_multiple_ranges_fall_back_to_the_first_chunk() {
    for bad in ["bytes=a-b", "items=1-2", "bytes=1-2,5-6", "bytes=9-3", "bytes=", "garbage"] {
        assert_eq!(bounded_range(Some(bad), CHUNK), "bytes=0-999", "{bad}");
    }
}

#[test]
fn file_names_lose_separators_dots_and_control_characters() {
    assert_eq!(safe_file_name("../../etc/passwd"), "_.._etc_passwd");
    assert_eq!(safe_file_name(".hidden"), "hidden");
    assert_eq!(safe_file_name("a\nb:c.txt"), "a_b_c.txt");
    assert_eq!(safe_file_name("   "), "download");
    assert_eq!(safe_file_name("blob é.bin"), "blob é.bin");
}

#[test]
fn numbered_names_keep_the_extension() {
    assert_eq!(numbered_name("photo.jpg", 1), "photo.jpg");
    assert_eq!(numbered_name("photo.jpg", 2), "photo (2).jpg");
    assert_eq!(numbered_name("notes", 3), "notes (3)");
    assert_eq!(numbered_name(".bashrc", 2), ".bashrc (2)");
}

#[test]
fn a_prefix_answer_states_what_it_holds_out_of_the_total() {
    assert_eq!(prefix_content_range(1000, 24_716_686), "bytes 0-999/24716686");
}
