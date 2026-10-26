//! `cargo xtask check-error-codes`, also run as a test: every `SAILnnnn`
//! code used in the tools is listed in `docs/errors.md`, and every code
//! listed there is still used.

use std::collections::BTreeSet;
use std::path::Path;

/// Directories whose sources may use error codes.
const SOURCES: [&str; 6] = ["crates", "extensions", "xtask", "ci", "scripts", "bench"];
/// File extensions that hold code rather than recorded data.
const EXTENSIONS: [&str; 3] = ["rs", "py", "sh"];

/// The distinct codes in `text`: `SAIL` followed by exactly four digits.
fn codes(text: &str) -> BTreeSet<String> {
    let bytes = text.as_bytes();
    let mut out = BTreeSet::new();
    let mut start = 0;
    while let Some(found) = text[start..].find("SAIL") {
        let at = start + found;
        let digits = bytes.get(at + 4..at + 8);
        let next = bytes.get(at + 8);
        if digits.is_some_and(|d| d.iter().all(u8::is_ascii_digit))
            && !next.is_some_and(u8::is_ascii_digit)
        {
            out.insert(text[at..at + 8].to_owned());
        }
        start = at + 4;
    }
    out
}

fn collect(dir: &Path, out: &mut BTreeSet<String>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            collect(&path, out)?;
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| EXTENSIONS.contains(&e))
        {
            out.extend(codes(&std::fs::read_to_string(&path)?));
        }
    }
    Ok(())
}

/// Compares the codes used under `root` with `docs/errors.md`.
pub(crate) fn check(root: &Path) -> Result<(), String> {
    let mut used = BTreeSet::new();
    for dir in SOURCES {
        collect(&root.join(dir), &mut used).map_err(|e| format!("{dir}: {e}"))?;
    }
    let registry = std::fs::read_to_string(root.join("docs/errors.md"))
        .map_err(|e| format!("docs/errors.md: {e}"))?;
    let listed = codes(&registry);
    let unlisted: Vec<_> = used.difference(&listed).collect();
    let unused: Vec<_> = listed.difference(&used).collect();
    if unlisted.is_empty() && unused.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "docs/errors.md is out of date. Used but not listed: {unlisted:?}. \
             Listed but not used: {unused:?}."
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{check, codes};

    #[test]
    fn finds_exact_four_digit_codes() {
        // Built from parts so this file does not itself count as a use.
        let (a, b) = (concat!("SAIL", "0002"), concat!("SAIL", "3123"));
        let found = codes(&format!("error[{a}]: x {a}5 SAIL12 `{b}`"));
        assert_eq!(found.into_iter().collect::<Vec<_>>(), [a, b]);
    }

    #[test]
    fn every_used_code_is_registered_and_every_registered_code_is_used() {
        assert_eq!(check(&crate::workspace_root()), Ok(()));
    }
}
