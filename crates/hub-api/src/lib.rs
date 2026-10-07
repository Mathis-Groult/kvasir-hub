//! `hub-api`: the server companion of the Hub app. Pure request handling.
//!
//! One endpoint, `GET /v1/me` with HTTP Basic, answers who the account is and which apps it
//! may open. Hides: how credentials are checked ([`Verifier`]) and the 60 s success cache.
//! The shell in `main.rs` supplies the real verifier (`kvasir accounts login`) and the clock.

mod basic;
mod cache;
mod provision;
mod verifier;

pub use basic::{BasicError, parse_basic};
pub use cache::CredentialCache;
pub use provision::Provisioner;
pub use verifier::{Verdict, Verifier, VerifierError, parse_cli_login};

use hub_wire::{Me, Refusal};

/// Success answers are remembered this long (same trade-off as the Flask gates: argon2 is slow,
/// a revoked grant applies within a minute).
pub const CACHE_SECONDS: i64 = 60;

/// The collaborators `handle` needs, bundled so the signature stays short.
pub struct Services<'a> {
    pub verifier: &'a dyn Verifier,
    pub cache: &'a CredentialCache,
    pub provisioner: &'a dyn Provisioner,
}

/// A fully decided HTTP answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    pub body: String,
    /// Header sent with 401 so clients know Basic is expected.
    pub www_authenticate: bool,
}

impl Reply {
    fn json(status: u16, value: &impl serde::Serialize) -> Self {
        // Serialising the plain wire structs cannot fail.
        let body = serde_json::to_string(value).expect("wire types serialise");
        Self {
            status,
            body,
            www_authenticate: false,
        }
    }

    fn plain(status: u16, text: &str) -> Self {
        Self {
            status,
            body: text.to_owned(),
            www_authenticate: false,
        }
    }
}

/// Handles one request. `now` is unix seconds.
pub fn handle(
    method: &str,
    path: &str,
    authorization: Option<&str>,
    services: Services<'_>,
    now: i64,
) -> Reply {
    if path != "/v1/me" {
        return Reply::plain(404, "not found");
    }
    if method != "GET" {
        return Reply::plain(405, "method not allowed");
    }
    let Some(header) = authorization else {
        return unauthorized("invalid");
    };
    let Ok((username, password)) = parse_basic(header) else {
        return unauthorized("invalid");
    };
    let Services {
        verifier,
        cache,
        provisioner,
    } = services;
    if let Some(me) = cache.get(&username, &password, now) {
        return Reply::json(200, &me);
    }
    match verifier.verify(&username, &password) {
        Ok(Verdict::Welcome(me)) => {
            // A failure here must not block sign-in: the app page shows its own error later.
            if let Err(e) = provisioner.ensure(&me) {
                eprintln!("[hub-api] provisioning {} failed: {e}", me.username);
            }
            cache.put(&username, &password, me.clone(), now + CACHE_SECONDS);
            Reply::json(200, &me)
        }
        Ok(Verdict::Refused(refusal)) if refusal.outcome == "locked" => Reply::json(423, &refusal),
        Ok(Verdict::Refused(refusal)) => {
            let mut reply = Reply::json(401, &refusal);
            reply.www_authenticate = true;
            reply
        }
        Err(e) => {
            eprintln!("[hub-api] verifier failed: {e}");
            Reply::plain(502, "account service unavailable")
        }
    }
}

fn unauthorized(outcome: &str) -> Reply {
    let mut reply = Reply::json(
        401,
        &Refusal {
            outcome: outcome.to_owned(),
            locked_until: None,
        },
    );
    reply.www_authenticate = true;
    reply
}

/// Kept for the shell: build a `Me` from literal parts in tests.
#[doc(hidden)]
pub fn me(username: &str, apps: &[&str]) -> Me {
    Me {
        username: username.to_owned(),
        role: hub_wire::Role::User,
        apps: apps.iter().map(|a| (*a).to_owned()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    /// Counts calls so tests can prove the cache spares the verifier.
    struct Scripted {
        verdict: Result<Verdict, ()>,
        calls: Cell<u32>,
    }

    impl Scripted {
        fn new(verdict: Result<Verdict, ()>) -> Self {
            Self {
                verdict,
                calls: Cell::new(0),
            }
        }
    }

    impl Verifier for Scripted {
        fn verify(&self, _: &str, _: &str) -> Result<Verdict, VerifierError> {
            self.calls.set(self.calls.get() + 1);
            self.verdict
                .clone()
                .map_err(|()| VerifierError::Unavailable("down".into()))
        }
    }

    /// Records which accounts were provisioned; optionally fails.
    #[derive(Default)]
    struct Recorder {
        seen: RefCell<Vec<String>>,
        fail: bool,
    }

    impl Provisioner for Recorder {
        fn ensure(&self, me: &Me) -> Result<(), String> {
            self.seen.borrow_mut().push(me.username.clone());
            if self.fail {
                Err("navidrome down".into())
            } else {
                Ok(())
            }
        }
    }

    /// One request against fresh collaborators, unless the caller shares some.
    struct Rig {
        verifier: Scripted,
        cache: CredentialCache,
        provisioner: Recorder,
    }

    impl Rig {
        fn new(verdict: Result<Verdict, ()>) -> Self {
            Self {
                verifier: Scripted::new(verdict),
                cache: CredentialCache::default(),
                provisioner: Recorder::default(),
            }
        }

        fn get(&self, header: Option<&str>, now: i64) -> Reply {
            self.request("GET", "/v1/me", header, now)
        }

        fn request(&self, method: &str, path: &str, header: Option<&str>, now: i64) -> Reply {
            let services = Services {
                verifier: &self.verifier,
                cache: &self.cache,
                provisioner: &self.provisioner,
            };
            handle(method, path, header, services, now)
        }
    }

    // base64("alice:pw")
    const ALICE: &str = "Basic YWxpY2U6cHc=";

    fn welcome() -> Verdict {
        Verdict::Welcome(me("alice", &["files", "music"]))
    }

    fn refused(outcome: &str, until: Option<i64>) -> Verdict {
        Verdict::Refused(Refusal {
            outcome: outcome.to_owned(),
            locked_until: until,
        })
    }

    #[test]
    fn good_credentials_get_the_account() {
        let reply = Rig::new(Ok(welcome())).get(Some(ALICE), 0);
        assert_eq!(reply.status, 200);
        assert_eq!(
            reply.body,
            r#"{"username":"alice","role":"user","apps":["files","music"]}"#
        );
    }

    #[test]
    fn wrong_password_is_401_with_challenge() {
        let reply = Rig::new(Ok(refused("invalid", None))).get(Some(ALICE), 0);
        assert_eq!(reply.status, 401);
        assert!(reply.www_authenticate);
        assert_eq!(reply.body, r#"{"outcome":"invalid"}"#);
    }

    #[test]
    fn pending_and_disabled_are_401_with_their_outcome() {
        for outcome in ["pending", "disabled"] {
            let reply = Rig::new(Ok(refused(outcome, None))).get(Some(ALICE), 0);
            assert_eq!(reply.status, 401);
            assert!(reply.body.contains(outcome));
        }
    }

    #[test]
    fn lockout_is_423_with_deadline() {
        let reply = Rig::new(Ok(refused("locked", Some(1790)))).get(Some(ALICE), 0);
        assert_eq!(reply.status, 423);
        assert_eq!(reply.body, r#"{"outcome":"locked","locked_until":1790}"#);
    }

    #[test]
    fn missing_or_malformed_header_never_reaches_the_verifier() {
        let rig = Rig::new(Ok(welcome()));
        for header in [
            None,
            Some("Bearer abc"),
            Some("Basic !!!"),
            Some("Basic Zm9v"),
        ] {
            assert_eq!(rig.get(header, 0).status, 401);
        }
        assert_eq!(rig.verifier.calls.get(), 0);
    }

    #[test]
    fn success_is_cached_for_sixty_seconds_then_rechecked() {
        let rig = Rig::new(Ok(welcome()));
        rig.get(Some(ALICE), 1000);
        rig.get(Some(ALICE), 1059);
        assert_eq!(rig.verifier.calls.get(), 1);
        rig.get(Some(ALICE), 1060);
        assert_eq!(rig.verifier.calls.get(), 2);
    }

    #[test]
    fn refusals_are_never_cached() {
        let rig = Rig::new(Ok(refused("invalid", None)));
        rig.get(Some(ALICE), 0);
        rig.get(Some(ALICE), 1);
        assert_eq!(rig.verifier.calls.get(), 2);
    }

    #[test]
    fn a_different_password_does_not_hit_the_cache() {
        let rig = Rig::new(Ok(welcome()));
        rig.get(Some(ALICE), 0);
        // base64("alice:other")
        rig.get(Some("Basic YWxpY2U6b3RoZXI="), 1);
        assert_eq!(rig.verifier.calls.get(), 2);
    }

    #[test]
    fn broken_verifier_is_502_not_a_false_refusal() {
        assert_eq!(Rig::new(Err(())).get(Some(ALICE), 0).status, 502);
    }

    #[test]
    fn other_paths_and_methods_are_rejected() {
        let rig = Rig::new(Ok(welcome()));
        assert_eq!(rig.request("GET", "/v1/other", Some(ALICE), 0).status, 404);
        assert_eq!(rig.request("POST", "/v1/me", Some(ALICE), 0).status, 405);
    }

    #[test]
    fn accounts_are_provisioned_once_per_cache_window_not_on_refusals() {
        let rig = Rig::new(Ok(welcome()));
        rig.get(Some(ALICE), 0);
        rig.get(Some(ALICE), 10);
        assert_eq!(*rig.provisioner.seen.borrow(), vec!["alice"]);
        let refused_rig = Rig::new(Ok(refused("invalid", None)));
        refused_rig.get(Some(ALICE), 0);
        assert!(refused_rig.provisioner.seen.borrow().is_empty());
    }

    #[test]
    fn a_provisioning_failure_does_not_block_sign_in() {
        let mut rig = Rig::new(Ok(welcome()));
        rig.provisioner.fail = true;
        assert_eq!(rig.get(Some(ALICE), 0).status, 200);
    }
}
