//! Finding captured solver sessions in the shim's trace directories
//! (`SAIL_TRACE_DIR`).
//!
//! Each solver process the shim served left three files, all named by the
//! session id `<pid>-<nanos>`: `<id>.in.smt2` (the exact bytes the solver
//! received), `<id>.out` (the exact bytes it produced) and `<id>.jsonl`
//! (the shim's trace records). A trace directory either holds such files
//! directly, or holds one subdirectory of them per run (for example one per
//! corpus entry); [`discover`] accepts both.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::PortfolioError;

/// The files of one captured solver session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionFiles {
    /// A readable id: `<subdir>/<pid>-<nanos>` or `<pid>-<nanos>`.
    pub id: String,
    /// `<id>.jsonl`.
    pub jsonl: PathBuf,
    /// `<id>.in.smt2`.
    pub input: PathBuf,
    /// `<id>.out`.
    pub output: PathBuf,
}

impl SessionFiles {
    /// The files of a session whose input is at `input` (`<stem>.in.smt2`),
    /// with the output and trace next to it. `id` names it in reports.
    #[must_use]
    pub fn from_input(id: &str, input: &Path) -> Self {
        let name = input
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let stem = name.strip_suffix(".in.smt2").unwrap_or(&name);
        let dir = input.parent().unwrap_or_else(|| Path::new(""));
        Self {
            id: id.to_owned(),
            jsonl: dir.join(format!("{stem}.jsonl")),
            input: input.to_path_buf(),
            output: dir.join(format!("{stem}.out")),
        }
    }
}

/// Lists the sessions under `dir`, sorted by id: the `*.jsonl` files directly
/// in it, or, if there are none, those in its immediate subdirectories.
///
/// # Errors
///
/// [`PortfolioError::Io`] if a directory cannot be read.
pub fn discover(dir: &Path) -> Result<Vec<SessionFiles>, PortfolioError> {
    let mut found = sessions_in(dir, None)?;
    if found.is_empty() {
        let mut subdirs: Vec<PathBuf> = fs::read_dir(dir)
            .map_err(|e| PortfolioError::io(dir, &e))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        subdirs.sort();
        for sub in subdirs {
            let name = sub
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            found.extend(sessions_in(&sub, Some(&name))?);
        }
    }
    Ok(found)
}

fn sessions_in(dir: &Path, prefix: Option<&str>) -> Result<Vec<SessionFiles>, PortfolioError> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| PortfolioError::io(dir, &e))? {
        let path = entry.map_err(|e| PortfolioError::io(dir, &e))?.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(stem) = name.strip_suffix(".jsonl") else {
            continue;
        };
        out.push(SessionFiles {
            id: prefix.map_or_else(|| stem.to_owned(), |p| format!("{p}/{stem}")),
            jsonl: path.clone(),
            input: dir.join(format!("{stem}.in.smt2")),
            output: dir.join(format!("{stem}.out")),
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// The sessions under every directory in `dirs`, in order.
///
/// # Errors
///
/// As [`discover`].
pub fn discover_all(dirs: &[PathBuf]) -> Result<Vec<SessionFiles>, PortfolioError> {
    let mut all = Vec::new();
    for dir in dirs {
        all.extend(discover(dir)?);
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_flat_and_nested_sessions() {
        let root =
            std::env::temp_dir().join(format!("sail-portfolio-sessions-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let nested = root.join("unit-a");
        assert!(fs::create_dir_all(&nested).is_ok());
        for stem in ["2-1", "1-1"] {
            assert!(fs::write(nested.join(format!("{stem}.jsonl")), "").is_ok());
        }
        assert!(fs::write(nested.join("ignored.txt"), "").is_ok());
        let found = discover(&root);
        let Ok(found) = found else {
            unreachable!("{found:?}")
        };
        let ids: Vec<&str> = found.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["unit-a/1-1", "unit-a/2-1"]);
        assert_eq!(found[0].input, nested.join("1-1.in.smt2"));
        let flat = discover(&nested);
        assert_eq!(flat.map(|f| f.len()), Ok(2));
        assert!(discover(&root.join("missing")).is_err());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn from_input_names_the_neighbours() {
        let s = SessionFiles::from_input("toy", Path::new("d/verus-toy.in.smt2"));
        assert_eq!(s.output, Path::new("d/verus-toy.out"));
        assert_eq!(s.jsonl, Path::new("d/verus-toy.jsonl"));
        assert_eq!(s.id, "toy");
    }
}
