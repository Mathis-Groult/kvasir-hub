//! The Kvasir account credentials held by the app.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Kvasir username and password. The password only ever goes to the servers of the configured site.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

impl Credentials {
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }
}

// Manual Debug: a derived one would print the password into logs and panics.
impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_prints_the_password() {
        let shown = format!("{:?}", Credentials::new("alice", "hunter2"));
        assert!(shown.contains("alice"));
        assert!(!shown.contains("hunter2"));
    }
}
