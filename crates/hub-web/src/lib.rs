//! Static file resolution and media header policy for the web shell.
//!
//! Hides: how URL paths map to files (and how traversal is refused) and which cache headers
//! media gets. The listener itself lives in `main.rs`.

use std::path::{Component, Path, PathBuf};

/// Maps a URL path to a file under `root`, or `None` when it would leave `root`.
///
/// `/` serves `index.html`; a path without an extension also serves `index.html` so the
/// client router can own deep links.
pub fn resolve_static(root: &Path, url_path: &str) -> Option<PathBuf> {
    let relative = Path::new(url_path.trim_start_matches('/'));
    let mut resolved = root.to_path_buf();
    for component in relative.components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            // `..`, prefixes and roots could escape `root`.
            _ => return None,
        }
    }
    if resolved == root || resolved.extension().is_none() {
        resolved = root.join("index.html");
    }
    Some(resolved)
}

/// Content type by extension for the files the UI build produces.
pub fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("json") | Some("webmanifest") => "application/json",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    }
}

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

    const ROOT: &str = "/srv/ui";

    #[test]
    fn root_and_extensionless_paths_serve_the_index() {
        for url in ["/", "/files", "/music/album/abc"] {
            assert_eq!(
                resolve_static(Path::new(ROOT), url),
                Some(PathBuf::from("/srv/ui/index.html")),
                "{url}"
            );
        }
    }

    #[test]
    fn assets_resolve_inside_the_root() {
        assert_eq!(
            resolve_static(Path::new(ROOT), "/js/app.js"),
            Some(PathBuf::from("/srv/ui/js/app.js"))
        );
    }

    #[test]
    fn traversal_is_refused() {
        for url in [
            "/../etc/passwd",
            "/js/../../secret.js",
            "//etc/passwd.txt/../..",
        ] {
            assert_eq!(resolve_static(Path::new(ROOT), url), None, "{url}");
        }
    }

    #[test]
    fn content_types() {
        assert_eq!(
            content_type(Path::new("a.js")),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(
            content_type(Path::new("a.unknown")),
            "application/octet-stream"
        );
    }

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
