//! R1-R3 of the plan: a module defined OUTSIDE hub-core plugs in through the public API only,
//! the account's grants decide what it may open, and no network is involved.

use hub_core::{
    Args, Credentials, Directory, HttpPort, HttpRequest, HttpResponse, Hub, HubError, Manifest,
    MediaRequest, MediaResponse, Module, ModuleDef, Ports, Refusal, SecretStore,
};
use hub_wire::{Me, Role};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

struct NoNetwork;
impl HttpPort for NoNetwork {
    fn send(&self, service: &str, _: HttpRequest) -> Result<HttpResponse, HubError> {
        Err(HubError::unavailable(service, "test has no network"))
    }
}

#[derive(Default)]
struct MemorySecrets(Mutex<Option<Credentials>>);
impl SecretStore for MemorySecrets {
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

/// Knows two accounts: alice may open `hello` and `other`, bob only `other`.
struct FixedDirectory {
    refuse: Option<Refusal>,
}
impl Directory for FixedDirectory {
    fn identify(&self, c: &Credentials) -> Result<Me, HubError> {
        if let Some(refusal) = self.refuse {
            return Err(HubError::Refused(refusal));
        }
        let apps: &[&str] = match c.username.as_str() {
            "alice" => &["hello", "other"],
            "bob" => &["other"],
            _ => return Err(HubError::Refused(Refusal::Invalid)),
        };
        Ok(Me {
            username: c.username.clone(),
            role: Role::User,
            apps: apps.iter().map(|s| (*s).to_owned()).collect(),
        })
    }
}

/// The module under test: greets the account it was built for.
struct Hello {
    who: String,
}
impl Module for Hello {
    fn call(&self, op: &str, args: &Value) -> Result<Value, HubError> {
        match op {
            "greet" => {
                let name = Args::new(op, args).str("name")?;
                Ok(json!({"text": format!("hello {name}, from {}", self.who)}))
            }
            _ => Err(HubError::UnknownOp {
                module: "hello".into(),
                op: op.into(),
            }),
        }
    }
    fn upload(&self, op: &str, args: &Value, body: Vec<u8>) -> Result<Value, HubError> {
        match op {
            "store" => Ok(json!({"name": Args::new(op, args).str("name")?, "bytes": body.len()})),
            _ => Err(HubError::UnknownOp {
                module: "hello".into(),
                op: op.into(),
            }),
        }
    }
    fn media(&self, _: &MediaRequest) -> Result<MediaResponse, HubError> {
        Ok(MediaResponse {
            status: 200,
            content_type: "text/plain".into(),
            content_length: Some(2),
            content_range: None,
            file_name: None,
            body: Box::new(std::io::Cursor::new(b"hi".to_vec())),
        })
    }
}

struct Other;
impl Module for Other {
    fn call(&self, _: &str, _: &Value) -> Result<Value, HubError> {
        Ok(json!("other"))
    }
}

fn hub_with(refuse: Option<Refusal>, secrets: Arc<MemorySecrets>) -> Hub {
    let defs = vec![
        ModuleDef::new(
            Manifest {
                id: "hello",
                title: "Hello",
                summary: "Says hello.",
            },
            Box::new(|_, creds| {
                Box::new(Hello {
                    who: creds.username,
                })
            }),
        ),
        ModuleDef::new(
            Manifest {
                id: "other",
                title: "Other",
                summary: "Another one.",
            },
            Box::new(|_, _| Box::new(Other)),
        ),
    ];
    Hub::new(
        defs,
        Ports {
            http: Arc::new(NoNetwork),
            secrets,
            directory: Arc::new(FixedDirectory { refuse }),
        },
    )
}

fn alice() -> Credentials {
    Credentials::new("alice", "pw")
}

#[test]
fn a_new_module_works_with_no_change_to_the_core() {
    let hub = hub_with(None, Arc::default());
    hub.sign_in(alice()).unwrap();
    let ids: Vec<_> = hub.manifests().unwrap().iter().map(|m| m.id).collect();
    assert_eq!(ids, ["hello", "other"]);
    let answer = hub
        .call("hello", "greet", &json!({"name": "world"}))
        .unwrap();
    assert_eq!(answer, json!({"text": "hello world, from alice"}));
}

#[test]
fn only_granted_modules_are_listed_and_callable() {
    let hub = hub_with(None, Arc::default());
    hub.sign_in(Credentials::new("bob", "pw")).unwrap();
    let ids: Vec<_> = hub.manifests().unwrap().iter().map(|m| m.id).collect();
    assert_eq!(ids, ["other"]);
    let err = hub
        .call("hello", "greet", &json!({"name": "x"}))
        .unwrap_err();
    assert!(matches!(err, HubError::Forbidden { ref module } if module == "hello"));
}

#[test]
fn unknown_module_is_distinct_from_forbidden() {
    let hub = hub_with(None, Arc::default());
    hub.sign_in(alice()).unwrap();
    let err = hub.call("nope", "x", &json!({})).unwrap_err();
    assert!(matches!(err, HubError::UnknownModule(ref m) if m == "nope"));
}

#[test]
fn nothing_is_callable_before_sign_in() {
    let hub = hub_with(None, Arc::default());
    assert!(matches!(hub.manifests(), Err(HubError::NotSignedIn)));
    assert!(matches!(
        hub.call("hello", "greet", &json!({})),
        Err(HubError::NotSignedIn)
    ));
}

#[test]
fn bad_arguments_name_the_operation() {
    let hub = hub_with(None, Arc::default());
    hub.sign_in(alice()).unwrap();
    let err = hub.call("hello", "greet", &json!({})).unwrap_err();
    assert_eq!(err.code(), "bad_args");
}

#[test]
fn media_goes_through_the_same_grant_check() {
    let hub = hub_with(None, Arc::default());
    hub.sign_in(alice()).unwrap();
    let mut response = hub.media("hello", &MediaRequest::default()).unwrap();
    let mut text = String::new();
    std::io::Read::read_to_string(&mut response.body, &mut text).unwrap();
    assert_eq!(text, "hi");
}

#[test]
fn uploads_carry_a_body_and_obey_the_grants() {
    let hub = hub_with(None, Arc::default());
    hub.sign_in(alice()).unwrap();
    let out = hub
        .upload("hello", "store", &json!({"name": "a.txt"}), vec![1, 2, 3])
        .unwrap();
    assert_eq!(out, json!({"name": "a.txt", "bytes": 3}));
    // A module that does not override `upload` answers UnknownOp, not a panic.
    let err = hub
        .upload("other", "store", &json!({}), vec![])
        .unwrap_err();
    assert_eq!(err.code(), "unknown_op");
    hub.sign_out().unwrap();
    hub.sign_in(Credentials::new("bob", "pw")).unwrap();
    let denied = hub
        .upload("hello", "store", &json!({"name": "x"}), vec![])
        .unwrap_err();
    assert_eq!(denied.code(), "forbidden");
}

#[test]
fn sign_in_saves_credentials_and_restore_reuses_them() {
    let secrets: Arc<MemorySecrets> = Arc::default();
    hub_with(None, Arc::clone(&secrets))
        .sign_in(alice())
        .unwrap();
    let fresh_launch = hub_with(None, Arc::clone(&secrets));
    let me = fresh_launch
        .restore()
        .unwrap()
        .expect("saved credentials restore a session");
    assert_eq!(me.username, "alice");
    assert!(
        fresh_launch
            .call("hello", "greet", &json!({"name": "again"}))
            .is_ok()
    );
}

#[test]
fn restore_with_nothing_saved_is_none() {
    assert!(hub_with(None, Arc::default()).restore().unwrap().is_none());
}

#[test]
fn restore_deletes_credentials_the_server_now_refuses() {
    let secrets: Arc<MemorySecrets> = Arc::default();
    secrets.save(&alice()).unwrap();
    let hub = hub_with(Some(Refusal::Disabled), Arc::clone(&secrets));
    assert!(hub.restore().unwrap().is_none());
    assert!(secrets.load().unwrap().is_none());
}

#[test]
fn refused_sign_in_saves_nothing_and_opens_no_session() {
    let secrets: Arc<MemorySecrets> = Arc::default();
    let hub = hub_with(None, Arc::clone(&secrets));
    let err = hub.sign_in(Credentials::new("mallory", "pw")).unwrap_err();
    assert_eq!(err.code(), "bad_credentials");
    assert!(secrets.load().unwrap().is_none());
    assert!(hub.me().is_none());
}

#[test]
fn sign_out_drops_session_and_secret() {
    let secrets: Arc<MemorySecrets> = Arc::default();
    let hub = hub_with(None, Arc::clone(&secrets));
    hub.sign_in(alice()).unwrap();
    hub.sign_out().unwrap();
    assert!(hub.me().is_none());
    assert!(secrets.load().unwrap().is_none());
    assert!(matches!(
        hub.call("hello", "greet", &json!({})),
        Err(HubError::NotSignedIn)
    ));
}
