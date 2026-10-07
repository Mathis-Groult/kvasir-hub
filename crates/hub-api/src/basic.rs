//! HTTP Basic header parsing.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;

/// Why an `Authorization` header was not usable.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BasicError {
    #[error("not a Basic header")]
    NotBasic,
    #[error("payload is not base64 text")]
    BadEncoding,
    #[error("payload has no ':' separator")]
    NoSeparator,
}

/// `Basic base64(user:password)` -> `(user, password)`. The password may contain ':'.
pub fn parse_basic(header: &str) -> Result<(String, String), BasicError> {
    let payload = header.strip_prefix("Basic ").ok_or(BasicError::NotBasic)?;
    let bytes = STANDARD
        .decode(payload.trim())
        .map_err(|_| BasicError::BadEncoding)?;
    let text = String::from_utf8(bytes).map_err(|_| BasicError::BadEncoding)?;
    let (user, password) = text.split_once(':').ok_or(BasicError::NoSeparator)?;
    Ok((user.to_owned(), password.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_at_first_colon_only() {
        // base64("bob:pa:ss")
        assert_eq!(
            parse_basic("Basic Ym9iOnBhOnNz").unwrap(),
            ("bob".into(), "pa:ss".into())
        );
    }

    #[test]
    fn rejects_other_schemes_and_garbage() {
        assert_eq!(parse_basic("Bearer x"), Err(BasicError::NotBasic));
        assert_eq!(parse_basic("Basic ***"), Err(BasicError::BadEncoding));
        assert_eq!(parse_basic("Basic Zm9v"), Err(BasicError::NoSeparator));
    }
}
