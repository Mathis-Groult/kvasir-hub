//! The domain the Hub's servers live on (`example.org`): the one place that knows it.
//!
//! Every server address (`music.<domain>`, `files.<domain>`, ...) is built from this value, which
//! the person who runs the servers sets; no address is written in the source.
//!
//! The same rules as `SiteDomain` in the server platform's `platform-core`; the Hub is a separate
//! workspace and takes only the part it uses.

use std::fmt;

const MAX_DOMAIN_LEN: usize = 253;
const MAX_LABEL_LEN: usize = 63;

/// Why a string is not a usable [`SiteDomain`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SiteDomainError {
    #[error("domain is empty")]
    Empty,
    #[error("domain `{0}` is longer than {MAX_DOMAIN_LEN} bytes")]
    TooLong(String),
    #[error(
        "domain `{0}` must be a bare host name such as `example.org` (no scheme, port, path or trailing dot)"
    )]
    NotABareHost(String),
    #[error(
        "domain `{0}` has an invalid label: use letters, digits and `-`, 1 to {MAX_LABEL_LEN} bytes, not starting or ending with `-`"
    )]
    BadLabel(String),
    #[error("domain `{0}` looks like an IP address, not a name")]
    LooksLikeAddress(String),
}

/// A validated, lower-case host name that server addresses hang on.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SiteDomain(String);

impl SiteDomain {
    /// Parses and validates `raw` once, at the boundary. Surrounding whitespace is ignored and
    /// upper case is folded to lower case.
    pub fn parse(raw: &str) -> Result<Self, SiteDomainError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(SiteDomainError::Empty);
        }
        if raw.len() > MAX_DOMAIN_LEN {
            return Err(SiteDomainError::TooLong(raw.to_owned()));
        }
        if raw.contains(['/', ':', '@', '?', '#', ' ', '\\']) || raw.ends_with('.') {
            return Err(SiteDomainError::NotABareHost(raw.to_owned()));
        }
        let lower = raw.to_ascii_lowercase();
        if !lower.split('.').all(is_label) {
            return Err(SiteDomainError::BadLabel(raw.to_owned()));
        }
        let last = lower.rsplit('.').next().unwrap_or_default();
        if last.bytes().all(|b| b.is_ascii_digit()) {
            return Err(SiteDomainError::LooksLikeAddress(raw.to_owned()));
        }
        Ok(Self(lower))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `https://[sub.]domain` followed by `path`. `sub` and `path` are the caller's own constants.
    pub fn https_url(&self, sub: Option<&str>, path: &str) -> String {
        match sub {
            Some(sub) => format!("https://{sub}.{}{path}", self.0),
            None => format!("https://{}{path}", self.0),
        }
    }
}

fn is_label(label: &str) -> bool {
    !label.is_empty()
        && label.len() <= MAX_LABEL_LEN
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

impl fmt::Display for SiteDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_host_name_is_accepted_trimmed_and_folded_to_lower_case() {
        assert_eq!(
            SiteDomain::parse("example.org").unwrap().as_str(),
            "example.org"
        );
        assert_eq!(
            SiteDomain::parse("  Example.ORG\n").unwrap().as_str(),
            "example.org"
        );
    }

    #[test]
    fn anything_that_is_not_a_bare_host_is_refused_with_the_reason() {
        assert_eq!(SiteDomain::parse("  "), Err(SiteDomainError::Empty));
        for raw in [
            "https://example.org",
            "example.org/",
            "example.org:8443",
            "example.org.",
            "u@example.org",
        ] {
            assert!(
                matches!(
                    SiteDomain::parse(raw),
                    Err(SiteDomainError::NotABareHost(_))
                ),
                "{raw}"
            );
        }
        for raw in ["-a.org", "a-.org", "a..org", "a_b.org"] {
            assert!(
                matches!(SiteDomain::parse(raw), Err(SiteDomainError::BadLabel(_))),
                "{raw}"
            );
        }
        for raw in ["10.0.0.5", "127.0.0.1"] {
            assert!(
                matches!(
                    SiteDomain::parse(raw),
                    Err(SiteDomainError::LooksLikeAddress(_))
                ),
                "{raw}"
            );
        }
    }

    #[test]
    fn links_are_built_from_a_sub_domain_and_a_path() {
        let d = SiteDomain::parse("example.org").unwrap();
        assert_eq!(d.https_url(None, "/hub"), "https://example.org/hub");
        assert_eq!(
            d.https_url(Some("music"), "/music/rest"),
            "https://music.example.org/music/rest"
        );
        assert_eq!(d.to_string(), "example.org");
    }
}
