//! How credentials are checked: a port plus the parser of the account CLI's answer.

use hub_wire::{Me, Refusal, Role};
use serde::Deserialize;

/// Outcome of checking one username/password pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Welcome(Me),
    Refused(Refusal),
}

/// The account service could not give an answer (not a refusal).
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum VerifierError {
    #[error("account service unavailable: {0}")]
    Unavailable(String),
    #[error("unreadable account service answer: {0}")]
    Unreadable(String),
}

/// Checks credentials against the Kvasir account store.
pub trait Verifier {
    fn verify(&self, username: &str, password: &str) -> Result<Verdict, VerifierError>;
}

#[derive(Deserialize)]
struct CliAccount {
    username: String,
    role: String,
    apps: Vec<String>,
}

#[derive(Deserialize)]
struct CliLogin {
    outcome: String,
    #[serde(default)]
    locked_until: Option<i64>,
    #[serde(default)]
    account: Option<CliAccount>,
}

/// Parses the JSON line printed by `kvasir accounts login`.
pub fn parse_cli_login(line: &str) -> Result<Verdict, VerifierError> {
    let login: CliLogin =
        serde_json::from_str(line).map_err(|e| VerifierError::Unreadable(e.to_string()))?;
    match (login.outcome.as_str(), login.account) {
        ("ok", Some(account)) => Ok(Verdict::Welcome(Me {
            username: account.username,
            role: if account.role == "admin" {
                Role::Admin
            } else {
                Role::User
            },
            apps: account.apps,
        })),
        ("ok", None) => Err(VerifierError::Unreadable("ok without account".to_owned())),
        (outcome, _) => Ok(Verdict::Refused(Refusal {
            outcome: outcome.to_owned(),
            locked_until: login.locked_until,
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_successful_login_line() {
        let line = r#"{"ok":true,"outcome":"ok","account":{"username":"alice","role":"admin","status":"active","grants":[],"created_at":1,"last_login":2,"locked_until":null,"apps":["files","kvasir"]}}"#;
        let Verdict::Welcome(me) = parse_cli_login(line).unwrap() else {
            panic!("expected welcome")
        };
        assert_eq!((me.username.as_str(), me.role), ("alice", Role::Admin));
        assert_eq!(me.apps, vec!["files", "kvasir"]);
    }

    #[test]
    fn parses_refusals_with_deadline() {
        let verdict =
            parse_cli_login(r#"{"ok":false,"outcome":"locked","locked_until":1790}"#).unwrap();
        assert_eq!(
            verdict,
            Verdict::Refused(Refusal {
                outcome: "locked".into(),
                locked_until: Some(1790)
            })
        );
        let verdict = parse_cli_login(r#"{"ok":false,"outcome":"invalid"}"#).unwrap();
        assert_eq!(
            verdict,
            Verdict::Refused(Refusal {
                outcome: "invalid".into(),
                locked_until: None
            })
        );
    }

    #[test]
    fn garbage_is_unreadable_not_a_refusal() {
        assert!(matches!(
            parse_cli_login("oops"),
            Err(VerifierError::Unreadable(_))
        ));
        assert!(matches!(
            parse_cli_login(r#"{"ok":true,"outcome":"ok"}"#),
            Err(VerifierError::Unreadable(_))
        ));
    }
}
