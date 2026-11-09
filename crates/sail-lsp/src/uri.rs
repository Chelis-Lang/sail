//! `file://` URIs for absolute Unix paths.
//!
//! Editors compare document URIs as strings, so the encoding here matches
//! what VS Code, Neovim and Helix produce for ordinary paths: every byte
//! outside RFC 3986's unreserved set and `/` is percent-encoded.

use std::fmt::Write as _;

/// The `file://` URI of an absolute path.
#[must_use]
pub fn from_path(path: &str) -> String {
    let mut uri = String::from("file://");
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'.' | b'_' | b'~') {
            uri.push(char::from(byte));
        } else {
            let _ = write!(uri, "%{byte:02X}");
        }
    }
    uri
}

/// The path of a `file://` URI with an empty or `localhost` authority.
/// `None` for any other scheme or authority, or for a bad escape.
#[must_use]
pub fn to_path(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    if !rest.starts_with('/') {
        return None;
    }
    let mut bytes = Vec::with_capacity(rest.len());
    let mut input = rest.bytes();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let high = char::from(input.next()?).to_digit(16)?;
            let low = char::from(input.next()?).to_digit(16)?;
            bytes.push(u8::try_from(high * 16 + low).ok()?);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).ok()
}

/// Joins `file` to `base` unless it is already absolute, and removes `.`
/// and `..` components lexically (no filesystem access).
#[must_use]
pub fn resolve(base: &str, file: &str) -> String {
    let joined = if file.starts_with('/') {
        file.to_owned()
    } else {
        format!("{base}/{file}")
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in joined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    format!("/{}", parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_round_trip_through_uris() {
        for path in ["/w/src/lib.rs", "/w/a b/ü#?.rs", "/"] {
            assert_eq!(to_path(&from_path(path)).as_deref(), Some(path));
        }
        assert_eq!(from_path("/w/a b.rs"), "file:///w/a%20b.rs");
        assert_eq!(to_path("file://localhost/w").as_deref(), Some("/w"));
        for bad in ["untitled:1", "file://host/w", "file:///%zz", "file:///%4"] {
            assert_eq!(to_path(bad), None);
        }
    }

    #[test]
    fn relative_files_resolve_against_the_workspace() {
        assert_eq!(resolve("/w", "src/lib.rs"), "/w/src/lib.rs");
        assert_eq!(resolve("/w/", "./src/../a.rs"), "/w/a.rs");
        assert_eq!(resolve("/w", "/abs/x.rs"), "/abs/x.rs");
    }
}
