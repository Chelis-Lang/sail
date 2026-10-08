//! The pinned corpus (`corpus/corpus.toml`) and its runnable units.
//!
//! The format follows the draft schema in `docs/survey/corpus.md` §4 and is
//! recorded in ADR 0006. Projects are pinned by full commit SHA and fetched
//! once into `<cache>/corpus/<repo>-<commit12>`. A *unit* is one Verus
//! invocation: one crate root of one project.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::toolchain::Pins;
use crate::{Error, Result};

/// Testing tiers, in increasing size. Running a tier runs every smaller tier
/// too. `Tier2` entries need version work and never run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// Small and quick: suitable for every PR.
    Fast,
    /// The main corpus.
    Standard,
    /// Everything, including long-running entries.
    Deep,
    /// Not yet compatible with the pinned release.
    Tier2,
}

impl std::str::FromStr for Tier {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "fast" => Ok(Self::Fast),
            "standard" => Ok(Self::Standard),
            "deep" => Ok(Self::Deep),
            _ => Err(Error::new(
                "SAIL3010",
                format!("unknown tier `{s}` (fast, standard, deep)"),
            )),
        }
    }
}

/// How a project is verified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Build {
    /// `verus <args> <root>` for each crate root.
    Verus,
    /// `cargo verus verify` (not yet supported by the runner).
    CargoVerus,
    /// A project-specific script (not yet supported by the runner).
    Script,
}

/// One pinned project.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// Unique name, used in unit ids and result paths.
    pub name: String,
    /// Git URL.
    pub url: String,
    /// Full commit SHA.
    pub commit: String,
    /// SPDX licence expression, or `NOASSERTION`.
    pub license: String,
    /// Which tier runs it.
    pub tier: Tier,
    /// Compatibility with the pinned Verus.
    pub verus_version_compat: String,
    /// Evidence for the compatibility claim.
    #[serde(default)]
    pub compat_evidence: String,
    /// How it is verified.
    pub build: Build,
    /// Crate roots relative to the repository root.
    #[serde(default)]
    pub crate_roots: Vec<String>,
    /// A directory whose `.rs` files (recursively) are each a crate root.
    #[serde(default)]
    pub crate_roots_dir: Option<String>,
    /// Path prefixes (relative to the repository root) to leave out.
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Arguments passed to Verus before the crate root. `{out}` is replaced
    /// by the unit's scratch output directory.
    #[serde(default)]
    pub extra_verus_args: Vec<String>,
    /// Extra environment for Verus.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// External requirements, for example `singular`.
    #[serde(default)]
    pub requires: Vec<String>,
    /// Plain Verus's outcome at pin time (metadata only; conformance always
    /// compares against plain Verus on the same run).
    #[serde(default)]
    pub expected: String,
    /// Where per-unit expectations come from, for example
    /// `examples-first-line-tag`.
    #[serde(default)]
    pub expected_from: Option<String>,
}

/// The corpus manifest.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    /// Manifest format version.
    pub schema_version: u32,
    /// The Verus release the corpus is validated against.
    pub verus_release: String,
    /// That release's commit.
    pub verus_commit: String,
    /// The projects.
    #[serde(rename = "project", default)]
    pub projects: Vec<Project>,
}

/// One Verus invocation over one crate root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    /// `<project>/<root>`, unique within the corpus.
    pub id: String,
    /// The project's name.
    pub project: String,
    /// The fetched source tree.
    pub source_dir: PathBuf,
    /// The crate root, relative to `source_dir`.
    pub root: String,
    /// Verus arguments before the crate root.
    pub args: Vec<String>,
    /// Extra environment.
    pub env: BTreeMap<String, String>,
    /// The expected outcome tag, if the corpus records one.
    pub expected: Option<String>,
}

const EXAMPLE_TAG: &str = "// rust_verify/tests/example.rs";

/// The mode from an example's first line: `None` without a tag, otherwise
/// the first word after it (`expect-success` if there is none).
fn example_mode(first_line: &str) -> Option<String> {
    let rest = first_line.strip_prefix(EXAMPLE_TAG)?;
    let mode = rest.split_whitespace().next().unwrap_or("expect-success");
    Some(mode.to_owned())
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| Error::new("SAIL3012", format!("cannot list {}: {e}", dir.display())))?;
    for entry in entries {
        let path = entry
            .map_err(|e| Error::new("SAIL3012", format!("cannot list {}: {e}", dir.display())))?
            .path();
        if path.is_dir() {
            collect_rs(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

impl Corpus {
    /// Reads the manifest from `path`.
    ///
    /// # Errors
    ///
    /// `SAIL3011` if it cannot be read or parsed, or has an unknown schema.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| Error::new("SAIL3011", format!("cannot read {}: {e}", path.display())))?;
        let corpus: Self = toml::from_str(&text)
            .map_err(|e| Error::new("SAIL3011", format!("cannot parse {}: {e}", path.display())))?;
        if corpus.schema_version != 1 {
            return Err(Error::new(
                "SAIL3011",
                format!(
                    "unsupported corpus schema_version {}",
                    corpus.schema_version
                ),
            ));
        }
        Ok(corpus)
    }

    /// Checks that the corpus was validated against the pinned release.
    ///
    /// # Errors
    ///
    /// `SAIL3013` on a mismatch.
    pub fn check_pins(&self, pins: &Pins) -> Result<()> {
        if self.verus_release == pins.verus.release && self.verus_commit == pins.verus.commit {
            Ok(())
        } else {
            Err(Error::new(
                "SAIL3013",
                format!(
                    "corpus is validated against Verus {} ({}), but {} ({}) is pinned",
                    self.verus_release, self.verus_commit, pins.verus.release, pins.verus.commit
                ),
            ))
        }
    }

    /// Projects that run in `tier`, optionally only those named in `only`.
    #[must_use]
    pub fn projects_in(&self, tier: Tier, only: &[String]) -> Vec<&Project> {
        self.projects
            .iter()
            .filter(|p| p.tier <= tier && p.tier != Tier::Tier2)
            .filter(|p| only.is_empty() || only.contains(&p.name))
            .collect()
    }
}

impl Project {
    /// Where this project's source is fetched to under `cache`.
    #[must_use]
    pub fn source_dir(&self, cache: &Path) -> PathBuf {
        let repo = self
            .url
            .trim_end_matches('/')
            .trim_end_matches(".git")
            .rsplit('/')
            .next()
            .unwrap_or("repo");
        let short = self.commit.get(..12).unwrap_or(&self.commit);
        cache.join("corpus").join(format!("{repo}-{short}"))
    }

    /// Fetches the pinned commit (shallow) if it is not already present, and
    /// returns the source directory.
    ///
    /// # Errors
    ///
    /// `SAIL3014` if git fails.
    pub fn fetch(&self, cache: &Path) -> Result<PathBuf> {
        let dir = self.source_dir(cache);
        let marker = dir.join(".sail-fetched");
        if std::fs::read_to_string(&marker).is_ok_and(|c| c.trim() == self.commit) {
            return Ok(dir);
        }
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)
            .map_err(|e| Error::new("SAIL3014", format!("cannot create {}: {e}", dir.display())))?;
        let git = |args: &[&str]| -> Result<()> {
            let status = Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(args)
                .status()
                .map_err(|e| Error::new("SAIL3014", format!("cannot run git: {e}")))?;
            if status.success() {
                Ok(())
            } else {
                Err(Error::new(
                    "SAIL3014",
                    format!("git {} failed in {}", args.join(" "), dir.display()),
                ))
            }
        };
        git(&["init", "-q"])?;
        git(&["remote", "add", "origin", &self.url])?;
        git(&["fetch", "-q", "--depth", "1", "origin", &self.commit])?;
        git(&["checkout", "-q", "--detach", "FETCH_HEAD"])?;
        std::fs::write(&marker, &self.commit).map_err(|e| {
            Error::new(
                "SAIL3014",
                format!("cannot write {}: {e}", marker.display()),
            )
        })?;
        Ok(dir)
    }

    /// The units of this project, given its fetched source directory.
    /// Examples tagged `ignore` are left out.
    ///
    /// # Errors
    ///
    /// `SAIL3015` for an unsupported build kind, `SAIL3012` if the source
    /// cannot be listed.
    pub fn units(&self, source_dir: &Path) -> Result<Vec<Unit>> {
        if self.build != Build::Verus {
            return Err(Error::new(
                "SAIL3015",
                format!(
                    "{}: build {:?} is not supported by the runner yet",
                    self.name, self.build
                ),
            ));
        }
        let mut roots: Vec<String> = self.crate_roots.clone();
        if let Some(dir) = &self.crate_roots_dir {
            let mut files = Vec::new();
            collect_rs(&source_dir.join(dir), &mut files)?;
            roots.extend(
                files
                    .iter()
                    .filter_map(|f| f.strip_prefix(source_dir).ok())
                    .map(|f| f.to_string_lossy().replace('\\', "/")),
            );
        }
        roots.retain(|r| !self.exclude.iter().any(|x| r.starts_with(x.as_str())));
        roots.sort();
        let mut units = Vec::new();
        for root in roots {
            let mut args = self.extra_verus_args.clone();
            let mut expected = (!self.expected.is_empty()).then(|| self.expected.clone());
            if self.expected_from.as_deref() == Some("examples-first-line-tag") {
                let text = std::fs::read_to_string(source_dir.join(&root)).unwrap_or_default();
                let mode = example_mode(text.lines().next().unwrap_or_default())
                    .unwrap_or_else(|| "expect-success".to_owned());
                match mode.as_str() {
                    "ignore" => continue,
                    "expand-errors" => args.push("--expand-errors".to_owned()),
                    _ => {}
                }
                expected = Some(mode);
            }
            units.push(Unit {
                id: format!("{}/{root}", self.name),
                project: self.name.clone(),
                source_dir: source_dir.to_path_buf(),
                root,
                args,
                env: self.env.clone(),
                expected,
            });
        }
        Ok(units)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_example_modes() {
        assert_eq!(
            example_mode("// rust_verify/tests/example.rs").as_deref(),
            Some("expect-success")
        );
        assert_eq!(
            example_mode("// rust_verify/tests/example.rs ignore --- old").as_deref(),
            Some("ignore")
        );
        assert_eq!(
            example_mode("// rust_verify/tests/example.rs expand-errors").as_deref(),
            Some("expand-errors")
        );
        assert_eq!(example_mode("use vstd::prelude::*;"), None);
    }

    #[test]
    fn tiers_are_cumulative() {
        assert!(Tier::Fast < Tier::Standard && Tier::Standard < Tier::Deep);
        assert_eq!("standard".parse::<Tier>(), Ok(Tier::Standard));
        assert!("nightly".parse::<Tier>().is_err());
    }
}
