//! `multipart/form-data` body for Seafile uploads.

/// Builds `(content_type, body)` with plain fields and one file part.
pub(crate) fn upload_body(
    fields: &[(&str, &str)],
    file_name: &str,
    bytes: &[u8],
) -> (String, Vec<u8>) {
    // The boundary only has to be absent from the payload; a fixed random-looking token is
    // enough because file bytes containing it are astronomically unlikely, and checked below.
    let mut boundary = String::from("----KvasirHubBoundary7f3a9c2e51d84b06");
    while contains(bytes, boundary.as_bytes()) {
        boundary.push('x');
    }
    let mut body = Vec::with_capacity(bytes.len() + 512);
    for (name, value) in fields {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\nContent-Type: application/octet-stream\r\n\r\n",
            escape_quotes(file_name)
        )
        .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

fn escape_quotes(name: &str) -> String {
    name.replace('\\', "\\\\")
        .replace('"', "%22")
        .replace(['\r', '\n'], "_")
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_has_fields_then_the_file_between_boundaries() {
        let (content_type, body) = upload_body(&[("parent_dir", "/Docs")], "a.txt", b"hi");
        let boundary = content_type
            .strip_prefix("multipart/form-data; boundary=")
            .unwrap();
        let text = String::from_utf8(body).unwrap();
        let expected = format!(
            "--{b}\r\nContent-Disposition: form-data; name=\"parent_dir\"\r\n\r\n/Docs\r\n--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"a.txt\"\r\nContent-Type: application/octet-stream\r\n\r\nhi\r\n--{b}--\r\n",
            b = boundary
        );
        assert_eq!(text, expected);
    }

    #[test]
    fn a_boundary_found_in_the_payload_is_extended() {
        let payload = b"a----KvasirHubBoundary7f3a9c2e51d84b06-b";
        let (content_type, _) = upload_body(&[], "f", payload);
        assert!(content_type.ends_with("06x"), "{content_type}");
    }

    #[test]
    fn hostile_file_names_cannot_break_the_header() {
        let (_, body) = upload_body(&[], "a\"\r\nX: y", b"");
        let text = String::from_utf8(body).unwrap();
        assert!(text.contains("filename=\"a%22__X: y\""), "{text}");
    }
}
