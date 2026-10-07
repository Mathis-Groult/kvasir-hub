//! Types that cross the network between the Hub app and the `hub-api` server.
//!
//! Hides: the JSON shape of the sign-in answer. Both sides compile against this
//! crate, so a field cannot drift on one side only.

use serde::{Deserialize, Serialize};

/// What an account may do in Kvasir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Role {
    User,
    Admin,
}

/// The signed-in account as the server sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Me {
    pub username: String,
    pub role: Role,
    /// Kvasir app ids this account may open (`files`, `music`, ...).
    pub apps: Vec<String>,
}

/// Why the server refused to identify an account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    /// `invalid`, `pending`, `disabled`, `locked` (the account CLI's outcome names).
    pub outcome: String,
    /// Unix seconds, present when `outcome == "locked"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked_until: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn me_parses_the_literal_server_answer() {
        let json = r#"{"username":"alice","role":"admin","apps":["files","music"]}"#;
        let me: Me = serde_json::from_str(json).unwrap();
        assert_eq!(me.username, "alice");
        assert_eq!(me.role, Role::Admin);
        assert_eq!(me.apps, vec!["files", "music"]);
    }

    #[test]
    fn refusal_omits_locked_until_when_absent() {
        let refusal = Refusal {
            outcome: "invalid".into(),
            locked_until: None,
        };
        assert_eq!(
            serde_json::to_string(&refusal).unwrap(),
            r#"{"outcome":"invalid"}"#
        );
    }

    #[test]
    fn locked_refusal_carries_the_deadline() {
        let refusal: Refusal =
            serde_json::from_str(r#"{"outcome":"locked","locked_until":1790000000}"#).unwrap();
        assert_eq!(refusal.locked_until, Some(1_790_000_000));
    }
}
