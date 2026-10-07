//! Anki module: the sync-server details a user needs to connect Anki or AnkiDroid.
//!
//! The sync server speaks Anki's own protocol, which the Anki clients use directly, so
//! there is nothing to browse here. The login is the Kvasir account, checked by the Anki
//! gate; Anki lower-cases the name (one collection folder per user).
//!
//! Operation: `setup` -> `{ syncUrl, username }`.

use hub_core::{Credentials, Factory, HubError, Manifest, Module, ModuleDef};
use serde_json::{Value, json};

/// Catalog entry. `sync_url` is e.g. `https://anki.example.org/`.
pub fn definition(sync_url: &str) -> ModuleDef {
    let sync_url = sync_url.to_owned();
    let factory: Factory = Box::new(move |_http, credentials| {
        Box::new(Anki {
            sync_url: sync_url.clone(),
            credentials,
        })
    });
    ModuleDef::new(
        Manifest {
            id: "anki",
            title: "Anki",
            summary: "Sync server details for Anki and AnkiDroid.",
        },
        factory,
    )
}

struct Anki {
    sync_url: String,
    credentials: Credentials,
}

impl Module for Anki {
    fn call(&self, op: &str, _args: &Value) -> Result<Value, HubError> {
        match op {
            "setup" => Ok(json!({
                "syncUrl": self.sync_url,
                "username": self.credentials.username.to_lowercase(),
            })),
            _ => Err(HubError::UnknownOp {
                module: "anki".into(),
                op: op.to_owned(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hub_core::{HttpPort, HttpRequest, HttpResponse};
    use std::sync::Arc;

    /// The module must never touch the network.
    struct Forbidden;
    impl HttpPort for Forbidden {
        fn send(&self, _: &str, _: HttpRequest) -> Result<HttpResponse, HubError> {
            panic!("the Anki module makes no requests");
        }
    }

    fn anki() -> Box<dyn Module> {
        (definition("https://anki.example/").factory)(
            Arc::new(Forbidden),
            Credentials::new("Alice", "pw"),
        )
    }

    #[test]
    fn setup_gives_the_url_and_the_lowercase_login_without_any_request() {
        let out = anki().call("setup", &json!({})).unwrap();
        assert_eq!(
            out,
            json!({"syncUrl": "https://anki.example/", "username": "alice"})
        );
    }

    #[test]
    fn the_password_never_appears_in_an_answer() {
        let out = anki().call("setup", &json!({})).unwrap().to_string();
        assert!(!out.contains("pw"));
    }

    #[test]
    fn other_operations_are_unknown() {
        assert_eq!(
            anki().call("sync", &json!({})).unwrap_err().code(),
            "unknown_op"
        );
    }

    #[test]
    fn manifest_id_is_the_kvasir_app_id() {
        assert_eq!(definition("x").manifest.id, "anki");
    }
}
