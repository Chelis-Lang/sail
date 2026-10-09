//! The pinned corpus (`corpus/corpus.toml`) and its runnable units.
//!
//! The format follows the draft schema in `docs/survey/corpus.md` §4 and is
//! recorded in ADR 0006. Projects are pinned by full commit SHA and fetched
//! once into `<cache>/corpus/<repo>-<commit12>`. A *unit* is one Verus
//! invocation: one crate root of one project (`build = "verus"`), or one
//! `cargo verus` run in one package directory (`build = "cargo-verus"`).

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
    /// `cargo verus <cargo_verus_command>` in each package directory.
    CargoVerus,
    /// A project-specific script (not yet supported by the runner).
    Script,
    /// Verus with a scratch-built libc rlib from `test_libc`.
    LibcVerus,
}

/// The `cargo verus` subcommand a `cargo-verus` project runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CargoCommand {
    /// `cargo verus verify`: verifies the package and every dependency that
    /// opts into verification (including `vstd`), forwarding the Verus
    /// arguments to all of them.
    #[default]
    Verify,
    /// `cargo verus focus`: verifies only the selected package; dependencies
    /// are built with `--no-verify` and receive no forwarded arguments. This
    /// is what verita runs, and partial verification (`--verify-module`)
    /// requires it.
    Focus,
}

impl CargoCommand {
    /// The subcommand word.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verify => "verify",
            Self::Focus => "focus",
        }
    }
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
    /// Crate roots relative to the repository root: `.rs` files for
    /// `verus`, package directories for `cargo-verus`.
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
    /// Arguments passed to `cargo verus <command>` before `--`
    /// (`cargo-verus` only).
    #[serde(default)]
    pub extra_cargo_args: Vec<String>,
    /// Which `cargo verus` subcommand to run (`cargo-verus` only).
    #[serde(default)]
    pub cargo_verus_command: CargoCommand,
    /// Point `vstd`/`verus_builtin`/... dependencies at the pinned Verus
    /// source in a scratch copy of the project (`cargo-verus` only; see
    /// [`crate::patch`]).
    #[serde(default)]
    pub patch_verus_deps: bool,
    /// Keep a local `git bundle` of the pinned commit under
    /// `<cache>/archive/`, and fetch from it if the remote is gone. Required
    /// for fork pins, which can vanish (ADR 0010).
    #[serde(default)]
    pub archive: bool,
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
    /// How the unit is run.
    pub build: Build,
    /// Verus arguments: before the crate root for `verus`, after `--` for
    /// `cargo-verus`.
    pub args: Vec<String>,
    /// `cargo verus` arguments before `--`, starting with the subcommand
    /// (`cargo-verus` only).
    pub cargo_args: Vec<String>,
    /// Whether to patch Verus dependencies in a scratch copy first.
    pub patch_verus_deps: bool,
    /// Cargo's target directory (`cargo-verus` only), set by
    /// [`crate::run::prepare_unit`]. Both sides of a comparison use the same
    /// path, emptied before each run, so that paths Cargo prints (for
    /// example in a failed build script's report) are identical.
    pub target_dir: Option<PathBuf>,
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

/// The repository the pinned Verus source is fetched from.
pub const VERUS_REPO_URL: &str = "https://github.com/verus-lang/verus";

/// Where a repository pinned at `commit` is fetched to under `cache`.
fn checkout_dir(url: &str, commit: &str, cache: &Path) -> PathBuf {
    let repo = url
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .rsplit('/')
        .next()
        .unwrap_or("repo");
    let short = commit.get(..12).unwrap_or(commit);
    cache.join("corpus").join(format!("{repo}-{short}"))
}

/// Fetches `commit` of `url` (shallow) into `dir` unless it is already there.
fn git_in(dir: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
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
}

/// Writes a bundle of `commit` (checked out in `dir`) to `bundle`, unless it
/// already exists.
fn ensure_bundle(dir: &Path, commit: &str, bundle: &Path) -> Result<()> {
    if bundle.is_file() {
        return Ok(());
    }
    if let Some(parent) = bundle.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            Error::new(
                "SAIL3014",
                format!("cannot create {}: {e}", parent.display()),
            )
        })?;
    }
    // Name the commit with a ref so the bundle carries it.
    git_in(dir, &["update-ref", "refs/sail/pinned", commit])?;
    let b = bundle.to_string_lossy().into_owned();
    git_in(dir, &["bundle", "create", "-q", &b, "refs/sail/pinned"])
}

/// Fetches `commit` from `url` into `dir`. With `archive`, a bundle of the
/// commit is kept at that path, and is used when the remote fetch fails.
pub(crate) fn fetch_git(url: &str, commit: &str, dir: &Path, archive: Option<&Path>) -> Result<()> {
    let marker = dir.join(".sail-fetched");
    if std::fs::read_to_string(&marker).is_ok_and(|c| c.trim() == commit) {
        return archive.map_or(Ok(()), |b| ensure_bundle(dir, commit, b));
    }
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir)
        .map_err(|e| Error::new("SAIL3014", format!("cannot create {}: {e}", dir.display())))?;
    git_in(dir, &["init", "-q"])?;
    git_in(dir, &["remote", "add", "origin", url])?;
    match (
        git_in(dir, &["fetch", "-q", "--depth", "1", "origin", commit]),
        archive,
    ) {
        (Ok(()), _) => {}
        (Err(_), Some(bundle)) if bundle.is_file() => {
            eprintln!(
                "{url} unavailable; fetching {commit} from archive {}",
                bundle.display()
            );
            let b = bundle.to_string_lossy().into_owned();
            git_in(dir, &["fetch", "-q", &b, commit])?;
        }
        (Err(e), _) => return Err(e),
    }
    git_in(dir, &["checkout", "-q", "--detach", "FETCH_HEAD"])?;
    if let Some(bundle) = archive {
        ensure_bundle(dir, commit, bundle)?;
    }
    std::fs::write(&marker, commit).map_err(|e| {
        Error::new(
            "SAIL3014",
            format!("cannot write {}: {e}", marker.display()),
        )
    })
}

/// Whether the external requirement `name` is available on this machine,
/// where `rust_toolchain` is Verus's Rust toolchain:
/// - `singular`: `VERUS_SINGULAR_PATH` names a file;
/// - `openssl` (OpenSSL headers and libraries for `openssl-sys`):
///   `OPENSSL_DIR` names a directory, or `pkg-config --exists openssl`
///   succeeds;
/// - `rust-target:<triple>`: rustup lists `<triple>` as installed for
///   `rust_toolchain`;
/// - `program:<name>`: an executable `<name>` is on `PATH`.
///
/// Any other requirement is unknown and therefore unavailable.
#[must_use]
pub fn requirement_available(name: &str, rust_toolchain: &str) -> bool {
    let var = |v: &str| std::env::var_os(v).filter(|p| !p.is_empty());
    if let Some(program) = name.strip_prefix("program:") {
        return var("PATH").is_some_and(|path| {
            std::env::split_paths(&path).any(|dir| dir.join(program).is_file())
        });
    }
    if let Some(triple) = name.strip_prefix("rust-target:") {
        return Command::new("rustup")
            .args([
                "target",
                "list",
                "--installed",
                "--toolchain",
                rust_toolchain,
            ])
            .stderr(std::process::Stdio::null())
            .output()
            .is_ok_and(|o| {
                o.status.success()
                    && String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .any(|l| l.trim() == triple)
            });
    }
    match name {
        "verus-feature:singular" => {
            std::env::var_os("SAIL_VERUS_SOURCE_BUILD").is_some_and(|build| {
                crate::source_build::Manifest::load(Path::new(&build)).is_ok_and(|m| {
                    m.features.iter().any(|f| f == "singular") && m.rust_toolchain == rust_toolchain
                })
            })
        }
        "singular" => var("VERUS_SINGULAR_PATH").is_some_and(|p| Path::new(&p).is_file()),
        "openssl" => {
            var("OPENSSL_DIR").is_some_and(|p| Path::new(&p).is_dir())
                || Command::new("pkg-config")
                    .args(["--exists", "openssl"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .is_ok_and(|s| s.success())
        }
        _ => false,
    }
}

impl Corpus {
    /// Fetches the Verus source at `commit`, the active release's commit
    /// (used by `patch_verus_deps`), and returns its directory.
    ///
    /// # Errors
    ///
    /// `SAIL3014` if git fails.
    pub fn fetch_verus_source(&self, cache: &Path, commit: &str) -> Result<PathBuf> {
        let dir = checkout_dir(VERUS_REPO_URL, commit, cache);
        fetch_git(VERUS_REPO_URL, commit, &dir, None)?;
        Ok(dir)
    }

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
        // A secondary release is a compatibility target: the corpus is run
        // against it as is, and reports record which release ran.
        if pins.is_secondary
            || (self.verus_release == pins.verus.release && self.verus_commit == pins.verus.commit)
        {
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
        checkout_dir(&self.url, &self.commit, cache)
    }

    /// Fetches the pinned commit (shallow) if it is not already present, and
    /// returns the source directory.
    ///
    /// # Errors
    ///
    /// `SAIL3014` if git fails.
    pub fn fetch(&self, cache: &Path) -> Result<PathBuf> {
        let dir = self.source_dir(cache);
        let bundle = self.archive.then(|| self.archive_path(cache));
        fetch_git(&self.url, &self.commit, &dir, bundle.as_deref())?;
        Ok(dir)
    }

    /// Where this project's pinned commit is archived when `archive` is set.
    #[must_use]
    pub fn archive_path(&self, cache: &Path) -> PathBuf {
        let short = self.commit.get(..12).unwrap_or(&self.commit);
        cache
            .join("archive")
            .join(format!("{}-{short}.bundle", self.name))
    }

    /// The entries of `requires` that are not available here (see
    /// [`requirement_available`]).
    #[must_use]
    pub fn missing_requirements(&self, rust_toolchain: &str) -> Vec<String> {
        self.requires
            .iter()
            .filter(|r| !requirement_available(r, rust_toolchain))
            .cloned()
            .collect()
    }

    /// The units of this project, given its fetched source directory.
    /// Examples tagged `ignore` are left out.
    ///
    /// # Errors
    ///
    /// `SAIL3015` for an unsupported build kind, `SAIL3012` if the source
    /// cannot be listed.
    pub fn units(&self, source_dir: &Path) -> Result<Vec<Unit>> {
        let cargo = self.build == Build::CargoVerus;
        if self.build == Build::Script {
            return Err(Error::new(
                "SAIL3015",
                format!(
                    "{}: build {:?} is not supported by the runner yet",
                    self.name, self.build
                ),
            ));
        }
        if !cargo
            && (!self.extra_cargo_args.is_empty()
                || self.patch_verus_deps
                || self.cargo_verus_command != CargoCommand::Verify)
        {
            return Err(Error::new(
                "SAIL3015",
                format!(
                    "{}: extra_cargo_args, cargo_verus_command and patch_verus_deps need build = \"cargo-verus\"",
                    self.name
                ),
            ));
        }
        if cargo && self.crate_roots_dir.is_some() {
            return Err(Error::new(
                "SAIL3015",
                format!("{}: crate_roots_dir needs build = \"verus\"", self.name),
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
            let id = if root == "." {
                self.name.clone()
            } else {
                format!("{}/{root}", self.name)
            };
            let cargo_args = if cargo {
                let mut a = vec![self.cargo_verus_command.as_str().to_owned()];
                a.extend(self.extra_cargo_args.iter().cloned());
                a
            } else {
                Vec::new()
            };
            units.push(Unit {
                id,
                project: self.name.clone(),
                source_dir: source_dir.to_path_buf(),
                root,
                build: self.build,
                args,
                cargo_args,
                patch_verus_deps: self.patch_verus_deps,
                target_dir: None,
                env: self.env.clone(),
                expected,
            });
        }
        Ok(units)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn archived_pins_survive_a_vanished_remote() {
        let root = std::env::temp_dir().join(format!("sail-archive-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let origin = root.join("origin");
        let run = |dir: &Path, args: &[&str]| {
            let ok = std::process::Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
                .args(args)
                .status()
                .is_ok_and(|s| s.success());
            assert!(ok, "git {args:?}");
        };
        assert!(std::fs::create_dir_all(&origin).is_ok());
        run(&origin, &["init", "-q"]);
        assert!(std::fs::write(origin.join("lib.rs"), "fn main() {}").is_ok());
        run(&origin, &["add", "."]);
        run(&origin, &["commit", "-q", "-m", "pin"]);
        let commit = std::process::Command::new("git")
            .arg("-C")
            .arg(&origin)
            .args(["rev-parse", "HEAD"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .unwrap_or_default();
        let url = format!("file://{}", origin.display());
        let bundle = root.join("archive").join("p.bundle");
        assert!(fetch_git(&url, &commit, &root.join("a"), Some(&bundle)).is_ok());
        assert!(bundle.is_file());
        assert!(std::fs::remove_dir_all(&origin).is_ok());
        let again = root.join("b");
        assert!(fetch_git(&url, &commit, &again, Some(&bundle)).is_ok());
        assert!(again.join("lib.rs").is_file());
        assert!(fetch_git(&url, &commit, &root.join("c"), None).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

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

    fn project(extra: &str) -> Result<Project> {
        let text = format!(
            "name = \"p\"\nurl = \"https://example.com/o/p.git\"\ncommit = \"0123456789abcdef\"\nlicense = \"MIT\"\ntier = \"standard\"\nverus_version_compat = \"compatible\"\n{extra}"
        );
        toml::from_str(&text).map_err(|e| Error::new("SAIL3011", e.to_string()))
    }

    #[test]
    fn cargo_verus_units_carry_cargo_and_verus_args() {
        let p = project(
            "build = \"cargo-verus\"\ncrate_roots = [\"b\", \".\"]\ncargo_verus_command = \"focus\"\nextra_cargo_args = [\"--lib\"]\nextra_verus_args = [\"--verify-module\", \"m\"]\npatch_verus_deps = true\n",
        );
        let units = p
            .and_then(|p| p.units(Path::new("/src")))
            .unwrap_or_default();
        let ids: Vec<_> = units.iter().map(|u| u.id.as_str()).collect();
        assert_eq!(ids, ["p", "p/b"]);
        assert!(units.iter().all(|u| u.build == Build::CargoVerus
            && u.patch_verus_deps
            && u.cargo_args == ["focus", "--lib"]
            && u.args == ["--verify-module", "m"]));
    }

    #[test]
    fn cargo_only_fields_are_refused_for_verus_builds() {
        let p = project("build = \"verus\"\ncrate_roots = [\"a.rs\"]\npatch_verus_deps = true\n");
        assert!(p.and_then(|p| p.units(Path::new("/src"))).is_err());
        let p = project("build = \"script\"\n");
        assert!(p.and_then(|p| p.units(Path::new("/src"))).is_err());
        let p = project("build = \"verus\"\nbogus = 1\n");
        assert!(p.is_err(), "unknown fields are rejected");
    }

    #[test]
    fn unknown_requirements_are_missing() {
        let p = project(
            "build = \"verus\"\nrequires = [\"no-such-thing\", \"rust-target:none\", \"program:no-such-program\"]\n",
        );
        assert_eq!(
            p.map(|p| p.missing_requirements("no-such-toolchain")),
            Ok(vec![
                "no-such-thing".to_owned(),
                "rust-target:none".to_owned(),
                "program:no-such-program".to_owned()
            ])
        );
    }

    #[test]
    fn tiers_are_cumulative() {
        assert!(Tier::Fast < Tier::Standard && Tier::Standard < Tier::Deep);
        assert_eq!("standard".parse::<Tier>(), Ok(Tier::Standard));
        assert!("nightly".parse::<Tier>().is_err());
    }
}
