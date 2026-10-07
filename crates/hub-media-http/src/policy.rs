//! Header policy for media responses.

/// `Content-Disposition` value for a download. Header values must be ASCII, so the real name
/// goes in RFC 6266's `filename*` (percent-encoded UTF-8) next to an ASCII fallback.
pub fn content_disposition(file_name: &str) -> String {
    let ascii: String = file_name
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .filter(|c| *c != '"' && *c != '\\')
        .collect();
    let encoded: String = file_name
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                char::from(b).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect();
    format!("attachment; filename=\"{ascii}\"; filename*=UTF-8''{encoded}")
}

/// Cache policy per media operation: covers and thumbnails never change for a given id.
pub fn media_cache_control(op: &str) -> &'static str {
    match op {
        "cover" | "thumb" => "private, max-age=86400",
        _ => "private, no-store",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_names_stay_ascii_with_the_real_name_percent_encoded() {
        let value = content_disposition("blob é.bin");
        assert!(value.is_ascii());
        assert_eq!(
            value,
            "attachment; filename=\"blob _.bin\"; filename*=UTF-8''blob%20%C3%A9.bin"
        );
    }

    #[test]
    fn quotes_and_control_characters_cannot_break_out_of_the_header() {
        let value = content_disposition("a\"b\r\nc.txt");
        assert!(value.is_ascii());
        assert!(!value.contains('\r') && !value.contains('\n'));
        assert_eq!(value.matches('"').count(), 2, "{value}");
    }

    #[test]
    fn only_immutable_media_is_cached() {
        assert_eq!(media_cache_control("cover"), "private, max-age=86400");
        assert_eq!(media_cache_control("stream"), "private, no-store");
    }
}
