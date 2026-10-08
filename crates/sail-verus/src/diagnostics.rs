//! Verus's diagnostics as data, without rewording them.
//!
//! Verus (through rustc) prints human-readable diagnostics on stderr. This
//! module splits them into blocks, each starting at a top-level
//! `error`/`warning`/`note`/`help` header, and extracts the header's level,
//! code and message and the block's `-->` and `:::` source locations. The
//! block's text is kept verbatim, so nothing Verus reports is lost
//! (charter: "The user experience").

/// One source location in a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    /// The file, as printed.
    pub file: String,
    /// 1-based line.
    pub line: u64,
    /// 1-based column.
    pub column: u64,
    /// Whether this is the primary location (`-->`) rather than a secondary
    /// one (`:::`).
    pub primary: bool,
}

/// One diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// `error`, `warning`, `note` or `help`.
    pub level: String,
    /// The code in brackets after the level, if any (for example `E0308`).
    pub code: Option<String>,
    /// The header's message.
    pub message: String,
    /// Its source locations, in order.
    pub spans: Vec<Span>,
    /// The block's text, verbatim (ANSI colour codes removed).
    pub rendered: String,
}

/// Removes ANSI escape sequences (`ESC [ … letter`).
fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for d in chars.by_ref() {
                if d.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// `(level, code, message)` if `line` is a top-level diagnostic header.
fn header(line: &str) -> Option<(String, Option<String>, String)> {
    for level in ["error", "warning", "note", "help"] {
        let Some(rest) = line.strip_prefix(level) else {
            continue;
        };
        let (code, rest) = match rest.strip_prefix('[') {
            Some(r) => {
                let (code, after) = r.split_once(']')?;
                (Some(code.to_owned()), after)
            }
            None => (None, rest),
        };
        let message = rest.strip_prefix(": ")?;
        return Some((level.to_owned(), code, message.to_owned()));
    }
    None
}

/// A location line: `--> file:line:col` or `::: file:line:col`.
fn location(line: &str) -> Option<Span> {
    let trimmed = line.trim_start();
    let (primary, rest) = if let Some(r) = trimmed.strip_prefix("--> ") {
        (true, r)
    } else {
        (false, trimmed.strip_prefix("::: ")?)
    };
    let mut parts = rest.trim().rsplitn(3, ':');
    let column = parts.next()?.parse().ok()?;
    let line = parts.next()?.parse().ok()?;
    let file = parts.next()?.to_owned();
    Some(Span {
        file,
        line,
        column,
        primary,
    })
}

/// Parses every diagnostic in `stderr`. Lines before the first header (for
/// example Cargo's status lines) are not part of any diagnostic.
#[must_use]
pub fn parse(stderr: &str) -> Vec<Diagnostic> {
    let text = strip_ansi(stderr);
    let mut out: Vec<Diagnostic> = Vec::new();
    let mut lines: Vec<&str> = Vec::new();
    let finish = |out: &mut Vec<Diagnostic>, lines: &mut Vec<&str>| {
        while lines.last().is_some_and(|l| l.trim().is_empty()) {
            lines.pop();
        }
        if let Some(d) = out.last_mut() {
            d.rendered = lines.join("\n");
        }
        lines.clear();
    };
    for line in text.lines() {
        if let Some((level, code, message)) = header(line) {
            finish(&mut out, &mut lines);
            out.push(Diagnostic {
                level,
                code,
                message,
                spans: Vec::new(),
                rendered: String::new(),
            });
            lines.push(line);
        } else if let Some(d) = out.last_mut() {
            if let Some(span) = location(line) {
                d.spans.push(span);
            }
            lines.push(line);
        }
    }
    finish(&mut out, &mut lines);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const STDERR: &str = "    Checking demo v0.1.0 (/tmp/demo)
\u{1b}[1;31merror\u{1b}[0m: postcondition not satisfied
  --> src/lib.rs:10:13
   |
10 |     ensures r > x,
   |             ^^^^^ failed this postcondition
11 | { x }
   |   - at the end of the function body

error[E0308]: mismatched types
 --> src/main.rs:2:5
  ::: src/lib.rs:1:1
  = note: expected u32

warning: unused variable: `y`

error: could not compile `demo` (lib) due to 1 previous error
";

    #[test]
    fn splits_diagnostics_and_keeps_their_text() {
        let d = parse(STDERR);
        assert_eq!(d.len(), 4);
        assert_eq!(d[0].level, "error");
        assert_eq!(d[0].message, "postcondition not satisfied");
        assert_eq!(
            d[0].spans,
            [Span {
                file: "src/lib.rs".to_owned(),
                line: 10,
                column: 13,
                primary: true
            }]
        );
        assert!(
            d[0].rendered
                .starts_with("error: postcondition not satisfied\n  --> src/lib.rs:10:13")
        );
        assert!(d[0].rendered.ends_with("at the end of the function body"));
        assert_eq!(d[1].code.as_deref(), Some("E0308"));
        assert_eq!(d[1].spans.len(), 2);
        assert!(!d[1].spans[1].primary);
        assert_eq!(d[2].level, "warning");
        assert_eq!(d[2].spans, []);
        assert_eq!(
            d[3].message,
            "could not compile `demo` (lib) due to 1 previous error"
        );
    }

    #[test]
    fn handles_windows_style_paths_and_noise() {
        assert_eq!(
            location(" --> C:\\x\\a.rs:3:4").map(|s| s.file),
            Some("C:\\x\\a.rs".to_owned())
        );
        assert_eq!(location("  --> nonsense"), None);
        assert_eq!(parse("just text\nmore\n"), []);
        assert_eq!(header("errors: x"), None);
        assert_eq!(header("error x"), None);
    }
}
