//! Short-lived memory of successful sign-ins.

use hub_wire::Me;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;

/// Key = sha256(user NUL password): the clear password is never kept in memory longer than a request.
#[derive(Default)]
pub struct CredentialCache {
    entries: Mutex<HashMap<[u8; 32], (Me, i64)>>,
}

fn key(username: &str, password: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(username.as_bytes());
    hasher.update([0]);
    hasher.update(password.as_bytes());
    hasher.finalize().into()
}

impl CredentialCache {
    /// The remembered account if the entry is still valid at `now` (unix seconds).
    pub fn get(&self, username: &str, password: &str, now: i64) -> Option<Me> {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries
            .get(&key(username, password))
            .filter(|(_, until)| now < *until)
            .map(|(me, _)| me.clone())
    }

    /// Remembers `me` until `until`, dropping entries that already expired.
    pub fn put(&self, username: &str, password: &str, me: Me, until: i64) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let now = until - crate::CACHE_SECONDS;
        entries.retain(|_, (_, expires)| now < *expires);
        entries.insert(key(username, password), (me, until));
    }
}
