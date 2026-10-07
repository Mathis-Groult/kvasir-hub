//! The one error taxonomy shared by the core, every module and every shell.
//!
//! Hides: how failures are classified. The UI never sees Rust types; it receives an
//! [`ErrorView`] with a stable `code` it can switch on.

use serde::Serialize;

/// Why a sign-in was refused (the account CLI's outcome names).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refusal {
    /// Unknown user or wrong password.
    Invalid,
    /// The account waits for an admin to approve it.
    Pending,
    /// The account was disabled.
    Disabled,
    /// Too many failed attempts; `until` is unix seconds when known.
    Locked { until: Option<i64> },
}

/// Everything that can go wrong when using the Hub.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum HubError {
    #[error("not signed in")]
    NotSignedIn,
    #[error("sign-in refused: {0:?}")]
    Refused(Refusal),
    #[error("this account may not open {module}")]
    Forbidden { module: String },
    #[error("unknown module '{0}'")]
    UnknownModule(String),
    #[error("module '{module}' has no operation '{op}'")]
    UnknownOp { module: String, op: String },
    #[error("bad arguments for '{op}': {reason}")]
    BadArgs { op: String, reason: String },
    #[error("{service} is unreachable: {detail}")]
    Unavailable { service: String, detail: String },
    #[error("{service} denied the request (HTTP {status})")]
    Denied { service: String, status: u16 },
    #[error("{what} was not found")]
    NotFound { what: String },
    #[error("{service} answered HTTP {status}: {detail}")]
    Upstream {
        service: String,
        status: u16,
        detail: String,
    },
    #[error("{service} sent an unreadable answer: {detail}")]
    Malformed { service: String, detail: String },
    #[error("secret store: {0}")]
    SecretStore(String),
}

/// A [`HubError`] in the shape the UI consumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorView {
    /// Stable machine name, e.g. `unavailable`, `bad_credentials`, `locked`.
    pub code: &'static str,
    /// English sentence for the user.
    pub message: String,
    /// True when trying again later can succeed without any change.
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_until: Option<i64>,
}

impl HubError {
    /// Stable machine name of the error class.
    pub fn code(&self) -> &'static str {
        match self {
            HubError::NotSignedIn => "not_signed_in",
            HubError::Refused(Refusal::Invalid) => "bad_credentials",
            HubError::Refused(Refusal::Pending) => "pending",
            HubError::Refused(Refusal::Disabled) => "disabled",
            HubError::Refused(Refusal::Locked { .. }) => "locked",
            HubError::Forbidden { .. } => "forbidden",
            HubError::UnknownModule(_) => "unknown_module",
            HubError::UnknownOp { .. } => "unknown_op",
            HubError::BadArgs { .. } => "bad_args",
            HubError::Unavailable { .. } => "unavailable",
            HubError::Denied { .. } => "denied",
            HubError::NotFound { .. } => "not_found",
            HubError::Upstream { .. } => "upstream",
            HubError::Malformed { .. } => "malformed",
            HubError::SecretStore(_) => "secret_store",
        }
    }

    /// The error as the UI sees it.
    pub fn view(&self) -> ErrorView {
        let message = match self {
            HubError::Refused(Refusal::Invalid) => "Wrong username or password.".to_owned(),
            HubError::Refused(Refusal::Pending) => {
                "Your account is waiting for an admin to approve it.".to_owned()
            }
            HubError::Refused(Refusal::Disabled) => "This account is disabled.".to_owned(),
            HubError::Refused(Refusal::Locked { .. }) => {
                "Too many failed attempts. Try again later.".to_owned()
            }
            other => other.to_string(),
        };
        let retryable = matches!(
            self,
            HubError::Unavailable { .. }
                | HubError::Upstream {
                    status: 500..=599,
                    ..
                }
        );
        let locked_until = match self {
            HubError::Refused(Refusal::Locked { until }) => *until,
            _ => None,
        };
        ErrorView {
            code: self.code(),
            message,
            retryable,
            locked_until,
        }
    }

    /// Shorthand used by modules for network-level failures.
    pub fn unavailable(service: &str, detail: impl ToString) -> Self {
        HubError::Unavailable {
            service: service.to_owned(),
            detail: detail.to_string(),
        }
    }

    /// Shorthand used by modules for answers they cannot parse.
    pub fn malformed(service: &str, detail: impl ToString) -> Self {
        HubError::Malformed {
            service: service.to_owned(),
            detail: detail.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locked_view_carries_deadline_and_is_not_retryable() {
        let view = HubError::Refused(Refusal::Locked { until: Some(42) }).view();
        assert_eq!(view.code, "locked");
        assert_eq!(view.locked_until, Some(42));
        assert!(!view.retryable);
    }

    #[test]
    fn network_failure_is_retryable_and_names_the_service() {
        let view = HubError::unavailable("Seafile", "timed out").view();
        assert_eq!(view.code, "unavailable");
        assert_eq!(view.message, "Seafile is unreachable: timed out");
        assert!(view.retryable);
    }

    #[test]
    fn server_error_is_retryable_but_client_error_is_not() {
        let server = HubError::Upstream {
            service: "X".into(),
            status: 503,
            detail: String::new(),
        };
        let client = HubError::Upstream {
            service: "X".into(),
            status: 418,
            detail: String::new(),
        };
        assert!(server.view().retryable);
        assert!(!client.view().retryable);
    }
}
