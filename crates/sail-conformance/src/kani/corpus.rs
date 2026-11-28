//! The pinned Kani corpus, `corpus/kani-corpus.toml` (ADR 0183).
//!
//! Every project is pinned by full commit and names the package directory
//! whose harnesses are checked. `ci_args` records the project's own CI
//! arguments after `cargo kani`; the arms pass only what the Sail side under
//! test can also express, and the report states which arguments were left out.
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use super::Error;

/// A corpus tier. Only `fast` and `standard` exist for Kani today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tier {
    /// Small projects for every Kani behaviour change (minutes).
    Fast,
    /// The heavy projects (hours, memory-bound).
    Standard,
}

impl std::str::FromStr for Tier {
    type Err = Error;
    fn from_str(text: &str) -> Result<Self, Error> {
        match text {
            "fast" => Ok(Self::Fast),
            "standard" => Ok(Self::Standard),
            _ => Err(Error::corpus(format!(
                "unknown Kani tier `{text}` (fast, standard)"
            ))),
        }
    }
}

/// The whole corpus file.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    /// Format version; 1.
    pub schema_version: u32,
    /// The primary Kani pin every arm runs.
    pub kani_version: String,
    /// The CBMC version that Kani pin bundles.
    pub cbmc_version: String,
    /// The Rust toolchain that Kani pin installs.
    pub rust_toolchain: String,
    /// Secondary Kani pins some projects use in their own CI.
    #[serde(default)]
    pub secondary: Vec<Secondary>,
    /// The projects.
    #[serde(rename = "project")]
    pub projects: Vec<Project>,
}

/// A secondary Kani pin (ADR 0183): recorded, not yet run by any arm.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Secondary {
    /// The Kani release.
    pub kani_version: String,
    /// Projects whose own CI uses it.
    pub projects: Vec<String>,
    /// Current status, in words.
    pub status: String,
}

/// One pinned project package.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    /// Unique name, also the checkout directory name.
    pub name: String,
    /// Git URL.
    pub url: String,
    /// Full 40-hex commit.
    pub commit: String,
    /// Package directory inside the checkout holding the harnesses (`.` for
    /// the root).
    pub package: String,
    /// Tier.
    pub tier: Tier,
    /// SPDX licence of the project.
    pub license: String,
    /// The project's own CI arguments after `cargo kani`.
    #[serde(default)]
    pub ci_args: Vec<String>,
    /// Unwind bound for harnesses without `#[kani::unwind]`, while the Sail
    /// side requires an explicit `--unwind` (ADR 0156). Both arms get it.
    pub default_unwind: u32,
    /// A repository-relative `Cargo.lock` copied into the package's
    /// workspace root when the project commits none.
    #[serde(default)]
    pub lock: Option<String>,
    /// sha256 of that lock file.
    #[serde(default)]
    pub lock_sha256: Option<String>,
    /// Host packages the build needs beyond the CI image, in words.
    #[serde(default)]
    pub host_requires: Vec<String>,
    /// The Kani release the project's own CI pins, when it is not the primary.
    #[serde(default)]
    pub own_kani_pin: Option<String>,
    /// Outcome counts direct Kani gave at pin time, as metadata only.
    #[serde(default)]
    pub expected: Option<String>,
}

impl Corpus {
    /// Loads and validates the corpus.
    ///
    /// # Errors
    /// `SAIL6320` when the file is unreadable or malformed, a commit is not
    /// 40 hex digits, a name repeats or a bound is zero.
    pub fn load(path: &Path) -> Result<Self, Error> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| Error::corpus(format!("{}: {error}", path.display())))?;
        let corpus: Self = toml::from_str(&text)
            .map_err(|error| Error::corpus(format!("{}: {error}", path.display())))?;
        corpus.validate()?;
        Ok(corpus)
    }

    fn validate(&self) -> Result<(), Error> {
        if self.schema_version != 1 {
            return Err(Error::corpus("schema_version must be 1"));
        }
        let mut names = std::collections::BTreeSet::new();
        for project in &self.projects {
            let hex =
                project.commit.len() == 40 && project.commit.bytes().all(|b| b.is_ascii_hexdigit());
            if !hex {
                return Err(Error::corpus(format!(
                    "{}: commit must be a full 40-hex SHA",
                    project.name
                )));
            }
            if !names.insert(project.name.as_str())
                || project.name.is_empty()
                || !project
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            {
                return Err(Error::corpus(format!(
                    "project name `{}` is empty, repeated or not [A-Za-z0-9_-]",
                    project.name
                )));
            }
            if project.default_unwind == 0 {
                return Err(Error::corpus(format!(
                    "{}: default_unwind must be positive",
                    project.name
                )));
            }
            if project.package.split('/').any(|part| part == "..")
                || project.package.starts_with('/')
            {
                return Err(Error::corpus(format!(
                    "{}: package must stay inside the checkout",
                    project.name
                )));
            }
            if project.lock.is_some() != project.lock_sha256.is_some() {
                return Err(Error::corpus(format!(
                    "{}: lock and lock_sha256 go together",
                    project.name
                )));
            }
        }
        Ok(())
    }

    /// Projects in `tier`, or the named ones (which must exist and be in it).
    ///
    /// # Errors
    /// `SAIL6320` for an unknown name or one from another tier.
    pub fn select(&self, tier: Tier, names: &[String]) -> Result<Vec<&Project>, Error> {
        for name in names {
            match self.projects.iter().find(|p| &p.name == name) {
                None => return Err(Error::corpus(format!("unknown Kani project `{name}`"))),
                Some(p) if p.tier != tier => {
                    return Err(Error::corpus(format!(
                        "Kani project `{name}` is in another tier"
                    )));
                }
                Some(_) => {}
            }
        }
        Ok(self
            .projects
            .iter()
            .filter(|p| p.tier == tier && (names.is_empty() || names.contains(&p.name)))
            .collect())
    }
}

impl Project {
    /// Clones (once) and checks out the pinned commit under `cache`, copies the
    /// recorded lock file when there is one, and returns the package directory.
    ///
    /// # Errors
    /// `SAIL6321` when git fails, the checkout is at another commit, or the
    /// lock file is missing or has another hash.
    pub fn fetch(&self, cache: &Path, repo_root: &Path) -> Result<PathBuf, Error> {
        let checkout = cache.join(&self.name);
        if !checkout.join(".git").is_dir() {
            std::fs::create_dir_all(&checkout).map_err(|e| Error::prepare(e.to_string()))?;
            git(&checkout, &["init", "-q"])?;
            git(&checkout, &["remote", "add", "origin", &self.url])?;
        }
        if git_output(&checkout, &["rev-parse", "HEAD"])
            .ok()
            .as_deref()
            != Some(&self.commit)
        {
            git(
                &checkout,
                &["fetch", "-q", "--depth", "1", "origin", &self.commit],
            )?;
            git(&checkout, &["checkout", "-q", "--detach", "FETCH_HEAD"])?;
        }
        let head = git_output(&checkout, &["rev-parse", "HEAD"])?;
        if head != self.commit {
            return Err(Error::prepare(format!(
                "{}: checkout is at {head}, not {}",
                self.name, self.commit
            )));
        }
        let package = checkout.join(&self.package);
        if let (Some(lock), Some(sha)) = (&self.lock, &self.lock_sha256) {
            let bytes = std::fs::read(repo_root.join(lock))
                .map_err(|e| Error::prepare(format!("{lock}: {e}")))?;
            let actual = super::sha256_hex(&bytes);
            if &actual != sha {
                return Err(Error::prepare(format!(
                    "{lock}: sha256 {actual}, corpus records {sha}"
                )));
            }
            let target = workspace_root(&package).join("Cargo.lock");
            let current = std::fs::read(&target).ok();
            if current.as_deref() != Some(bytes.as_slice()) {
                std::fs::write(&target, &bytes).map_err(|e| Error::prepare(e.to_string()))?;
            }
        }
        Ok(package)
    }

    /// The arguments of `ci_args` the current Sail CLI cannot pass, which
    /// both arms therefore omit.
    #[must_use]
    pub fn omitted_ci_args(&self) -> Vec<String> {
        self.ci_args.clone()
    }
}

/// The nearest ancestor of `package` (inclusive) whose `Cargo.toml` declares
/// `[workspace]`, else `package` itself.
#[must_use]
pub fn workspace_root(package: &Path) -> PathBuf {
    let mut dir = Some(package);
    while let Some(current) = dir {
        if std::fs::read_to_string(current.join("Cargo.toml"))
            .is_ok_and(|text| text.lines().any(|l| l.trim() == "[workspace]"))
        {
            return current.to_path_buf();
        }
        dir = current.parent();
    }
    package.to_path_buf()
}

fn git(dir: &Path, args: &[&str]) -> Result<(), Error> {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .map_err(|e| Error::prepare(format!("git: {e}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::prepare(format!(
            "git {} failed: {status}",
            args.join(" ")
        )))
    }
}

fn git_output(dir: &Path, args: &[&str]) -> Result<String, Error> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| Error::prepare(format!("git: {e}")))?;
    if !out.status.success() {
        return Err(Error::prepare(format!("git {} failed", args.join(" "))));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_checked_in_corpus_loads_and_has_both_tiers() -> Result<(), Error> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/kani-corpus.toml");
        let corpus = Corpus::load(&path)?;
        let fast = corpus.select(Tier::Fast, &[])?;
        let names: Vec<_> = fast.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["arrayvec-pilot", "x86_64", "lading_throttle"]);
        assert_eq!(corpus.select(Tier::Standard, &[])?.len(), 4);
        assert!(
            corpus
                .select(Tier::Fast, &["s2n-quic-core".into()])
                .is_err()
        );
        assert!(corpus.select(Tier::Fast, &["nope".into()]).is_err());
        for project in &corpus.projects {
            if let (Some(lock), Some(sha)) = (&project.lock, &project.lock_sha256) {
                let bytes = std::fs::read(
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("../..")
                        .join(lock),
                )
                .map_err(|e| Error::corpus(e.to_string()))?;
                assert_eq!(&crate::kani::sha256_hex(&bytes), sha, "{lock}");
            }
        }
        Ok(())
    }

    #[test]
    fn malformed_entries_are_refused() {
        let base = r#"schema_version = 1
kani_version = "0.68.0"
cbmc_version = "6.11.0"
rust_toolchain = "nightly-2026-08-21"
[[project]]
name = "p"
url = "https://example.invalid/p"
package = "."
tier = "fast"
license = "MIT"
default_unwind = 1
"#;
        for (extra, ok) in [
            (
                "commit = \"0123456789012345678901234567890123456789\"\n",
                true,
            ),
            ("commit = \"abc\"\n", false),
            (
                "commit = \"0123456789012345678901234567890123456789\"\nlock = \"x\"\n",
                false,
            ),
        ] {
            let parsed: Result<Corpus, _> = toml::from_str(&format!("{base}{extra}"));
            let valid = parsed.is_ok_and(|c| c.validate().is_ok());
            assert_eq!(valid, ok, "{extra}");
        }
    }
}
