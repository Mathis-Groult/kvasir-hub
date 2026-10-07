//! Making sure the account exists in the apps that create users lazily.

use hub_wire::Me;

/// Creates the account's downstream users that would otherwise appear only after a first
/// visit in a browser (Navidrome). Idempotent: called on every uncached successful sign-in.
pub trait Provisioner {
    /// `Err` carries a short reason for the log; callers never fail sign-in because of it.
    fn ensure(&self, me: &Me) -> Result<(), String>;
}
