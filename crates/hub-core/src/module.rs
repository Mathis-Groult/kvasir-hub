//! The module contract: what an app must provide to appear in the Hub.
//!
//! Hides: how an app is described and called. A module is one crate that implements
//! [`Module`] and is listed once in the catalog; the core never names a concrete app.

use crate::{Credentials, HttpPort, HubError};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::Read;
use std::sync::Arc;

/// Static description of a module, shown on the home page and in navigation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Manifest {
    /// Equals the Kvasir app id used for grants (`files`, `music`, ...).
    pub id: &'static str,
    pub title: &'static str,
    /// One sentence shown under the title on the home grid.
    pub summary: &'static str,
}

/// Request for bytes the webview loads through a URL (audio, covers, downloads).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MediaRequest {
    pub op: String,
    pub params: BTreeMap<String, String>,
    /// Raw `Range` header from the player, forwarded so seeking works.
    pub range: Option<String>,
}

/// Bytes plus the headers a player needs.
pub struct MediaResponse {
    pub status: u16,
    pub content_type: String,
    pub content_length: Option<u64>,
    pub content_range: Option<String>,
    /// Suggested file name for downloads.
    pub file_name: Option<String>,
    pub body: Box<dyn Read + Send>,
}

/// One app, bound to the signed-in account. Dropped on sign-out, so it never holds a stale token.
pub trait Module: Send + Sync {
    /// Runs one operation. `args` is the JSON object the UI sent.
    fn call(&self, op: &str, args: &Value) -> Result<Value, HubError>;

    /// Runs an operation that carries a binary body (file upload). Default: none do.
    fn upload(&self, op: &str, _args: &Value, _body: Vec<u8>) -> Result<Value, HubError> {
        Err(HubError::UnknownOp {
            module: "this module".to_owned(),
            op: op.to_owned(),
        })
    }

    /// Streams bytes for `op`. Default: the module has no media.
    fn media(&self, request: &MediaRequest) -> Result<MediaResponse, HubError> {
        Err(HubError::UnknownOp {
            module: "this module".to_owned(),
            op: request.op.clone(),
        })
    }
}

/// Builds a module for one signed-in account.
pub type Factory = Box<dyn Fn(Arc<dyn HttpPort>, Credentials) -> Box<dyn Module> + Send + Sync>;

/// A catalog entry: static manifest + how to build the live module.
pub struct ModuleDef {
    pub manifest: Manifest,
    pub factory: Factory,
}

impl ModuleDef {
    pub fn new(manifest: Manifest, factory: Factory) -> Self {
        Self { manifest, factory }
    }
}

/// Typed reads over the JSON arguments of one operation; every miss names the operation.
pub struct Args<'a> {
    op: &'a str,
    value: &'a Value,
}

impl<'a> Args<'a> {
    pub fn new(op: &'a str, value: &'a Value) -> Self {
        Self { op, value }
    }

    /// Required string argument.
    pub fn str(&self, key: &str) -> Result<&'a str, HubError> {
        self.value
            .get(key)
            .and_then(Value::as_str)
            .ok_or_else(|| self.missing(key, "a string"))
    }

    /// Optional string argument.
    pub fn opt_str(&self, key: &str) -> Option<&'a str> {
        self.value.get(key).and_then(Value::as_str)
    }

    /// Optional unsigned integer argument.
    pub fn opt_u64(&self, key: &str) -> Option<u64> {
        self.value.get(key).and_then(Value::as_u64)
    }

    /// Optional boolean argument.
    pub fn opt_bool(&self, key: &str) -> Option<bool> {
        self.value.get(key).and_then(Value::as_bool)
    }

    fn missing(&self, key: &str, wanted: &str) -> HubError {
        HubError::BadArgs {
            op: self.op.to_owned(),
            reason: format!("'{key}' must be {wanted}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn required_string_present() {
        let value = json!({"id": "abc"});
        assert_eq!(Args::new("get", &value).str("id").unwrap(), "abc");
    }

    #[test]
    fn missing_argument_names_operation_and_key() {
        let value = json!({"id": 7});
        let err = Args::new("get", &value).str("id").unwrap_err();
        assert_eq!(
            err.to_string(),
            "bad arguments for 'get': 'id' must be a string"
        );
    }

    #[test]
    fn optional_reads_do_not_fail() {
        let value = json!({"limit": 20});
        let args = Args::new("list", &value);
        assert_eq!(args.opt_u64("limit"), Some(20));
        assert_eq!(args.opt_str("path"), None);
        assert_eq!(args.opt_bool("on"), None);
    }
}
