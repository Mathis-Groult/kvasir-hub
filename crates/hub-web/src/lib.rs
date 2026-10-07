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
}
