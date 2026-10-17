//! Conservative source-namespace eligibility, not an endpoint consumption proof.
//!
//! A loaded-file checksum cannot attest failed module-candidate lookups (VI-0025).
//! Refuse competing default candidates and workspace path/include syntax. This
//! deliberately accepts less than Rust; every unsupported form is a fresh miss.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;
use std::path::PathBuf;

pub(super) fn admit(
    files: &BTreeMap<PathBuf, String>,
    fixed_provider_roots: &BTreeSet<PathBuf>,
) -> Option<()> {
    for path in files
        .keys()
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
    {
        // All admitted trees, including fixed providers, must be unambiguous.
        if path.file_name()? != "mod.rs"
            && files.contains_key(&path.with_extension("").join("mod.rs"))
        {
            return None;
        }
        if !fixed_provider_roots
            .iter()
            .any(|root| path.starts_with(root))
        {
            let file = super::super::dependency_snapshot::open(path)?;
            let mut raw = Vec::new();
            file.take(16 * 1024 * 1024 + 1).read_to_end(&mut raw).ok()?;
            if raw.len() > 16 * 1024 * 1024
                || files.get(path)? != &super::super::files::digest(&raw)
                || !ordinary(&raw)
            {
                return None;
            }
        }
    }
    Some(())
}

// No expansion-dependent path attributes or include forms, including ones
// assembled by macro_rules fragments. Refuse these identifier tokens anywhere
// in executable source; strings/comments are skipped only by their exact Rust
// lexical delimiters. This is exclusion, not source transformation.
fn ordinary(raw: &[u8]) -> bool {
    let mut i = 0;
    while i < raw.len() {
        if raw[i..].starts_with(b"//") {
            i += 2;
            while i < raw.len() && raw[i] != b'\n' {
                i += 1;
            }
        } else if raw[i..].starts_with(b"/*") {
            let mut depth = 1usize;
            i += 2;
            while i < raw.len() && depth != 0 {
                if raw[i..].starts_with(b"/*") {
                    depth += 1;
                    i += 2;
                } else if raw[i..].starts_with(b"*/") {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if depth != 0 {
                return false;
            }
        } else if raw[i] == b'\'' {
            // A quote inside a character must not turn following source into
            // a string. Lifetimes have no closing quote and remain tokens.
            if let Some(end) = character_end(raw, i) {
                i = end;
            } else {
                i += 1;
            }
        } else if raw[i] == b'"' {
            i += 1;
            loop {
                let Some(&byte) = raw.get(i) else {
                    return false;
                };
                i += 1;
                if byte == b'"' {
                    break;
                }
                if byte == b'\\' {
                    if i == raw.len() {
                        return false;
                    }
                    i += 1;
                }
            }
        } else if raw[i] == b'r' || raw[i..].starts_with(b"br") || raw[i..].starts_with(b"cr") {
            let start = i;
            let mut cursor = i + if raw[i] == b'r' { 1 } else { 2 };
            while raw.get(cursor) == Some(&b'#') {
                cursor += 1;
            }
            if raw.get(cursor) == Some(&b'"') {
                let hashes = cursor - start - if raw[start] == b'r' { 1 } else { 2 };
                i = cursor + 1;
                loop {
                    let Some(&byte) = raw.get(i) else {
                        return false;
                    };
                    i += 1;
                    if byte == b'"'
                        && raw
                            .get(i..i + hashes)
                            .is_some_and(|suffix| suffix.iter().all(|&b| b == b'#'))
                    {
                        i += hashes;
                        break;
                    }
                }
            } else {
                i = identifier(raw, start);
                if forbidden(&raw[start..i]) {
                    return false;
                }
            }
        } else if raw[i].is_ascii_alphabetic() || raw[i] == b'_' {
            let start = i;
            i = identifier(raw, start);
            if forbidden(&raw[start..i]) {
                return false;
            }
        } else {
            i += 1;
        }
    }
    true
}

fn character_end(raw: &[u8], i: usize) -> Option<usize> {
    let first = *raw.get(i + 1)?;
    let close = if first == b'\\' {
        match *raw.get(i + 2)? {
            b'x' => i + 5,
            b'u' => {
                if raw.get(i + 3) != Some(&b'{') {
                    return None;
                }
                let mut end = i + 4;
                while end < i + 12 && *raw.get(end)? != b'}' {
                    end += 1;
                }
                if raw.get(end) != Some(&b'}') {
                    return None;
                }
                end + 1
            }
            _ => i + 3,
        }
    } else {
        let size = if first < 128 {
            1
        } else if first < 224 {
            2
        } else if first < 240 {
            3
        } else {
            4
        };
        i + 1 + size
    };
    (raw.get(close) == Some(&b'\'')).then_some(close + 1)
}

fn identifier(raw: &[u8], mut i: usize) -> usize {
    while raw
        .get(i)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        i += 1;
    }
    i
}

fn forbidden(token: &[u8]) -> bool {
    matches!(
        token,
        b"path" | b"include" | b"include_str" | b"include_bytes"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_namespace_tokens_refuse_even_when_split_across_macro_fragments() {
        for raw in [
            b"#[path=\"other.rs\"] mod a;".as_slice(),
            b"#[ /*c*/ r#path = \"other.rs\"] mod a;",
            b"include!(concat!(\"x\",\".rs\"));",
            b"m!(path);",
            b"let x = '\"'; #[path=\"x.rs\"] mod a;",
            b"include_str!(\"data\");",
            b"include_bytes!(\"data\");",
        ] {
            assert!(!ordinary(raw));
        }
        for raw in [
            b"mod a; // path include\n".as_slice(),
            b"/* path /* include */ */ mod a;",
            b"let x = r###\"/* path */\"###;",
            b"let x = \"// path\"; mod a;",
            b"let x = br#\"include\"#;",
        ] {
            assert!(ordinary(raw));
        }
        assert!(!ordinary(b"let x = r#\"unterminated"));
        assert!(!ordinary(b"/* unterminated"));
    }
    #[test]
    fn eligibility_reads_must_match_the_content_inventory() -> Result<(), Box<dyn std::error::Error>>
    {
        let root =
            std::env::temp_dir().join(format!("sail-retained-namespace-{}", std::process::id()));
        std::fs::create_dir(&root)?;
        let root = root.canonicalize()?;
        let file = root.join("lib.rs");
        std::fs::write(&file, b"mod ordinary; // path in comment\n")?;
        let a = super::super::super::files::digest(&std::fs::read(&file)?);
        let files = BTreeMap::from([(file.clone(), a.clone())]);
        let roots = BTreeSet::new();
        assert!(admit(&files, &roots).is_some());
        std::fs::write(&file, b"#[path=\"other.rs\"] mod ordinary;\n")?;
        assert!(
            admit(&files, &roots).is_none(),
            "a concurrent eligibility read cannot borrow the old content hash"
        );
        let changed = BTreeMap::from([(
            file.clone(),
            super::super::super::files::digest(&std::fs::read(&file)?),
        )]);
        assert!(
            admit(&changed, &roots).is_none(),
            "custom module lookup stays unsupported"
        );
        std::fs::write(&file, b"mod ordinary; // path in comment\n")?;
        assert!(
            admit(&files, &roots).is_some(),
            "literal A returns to the same source eligibility"
        );
        std::fs::remove_dir_all(&root)?;
        Ok(())
    }

    #[test]
    fn competing_default_candidates_are_not_consumption_evidence() {
        let files = BTreeMap::from([
            (PathBuf::from("/source/foo.rs"), String::new()),
            (PathBuf::from("/source/foo/mod.rs"), String::new()),
        ]);
        assert!(admit(&files, &BTreeSet::new()).is_none());
        let single = BTreeMap::from([(PathBuf::from("/source/foo.rs"), String::new())]);
        assert!(admit(&single, &BTreeSet::from([PathBuf::from("/source")])).is_some());
    }
}
