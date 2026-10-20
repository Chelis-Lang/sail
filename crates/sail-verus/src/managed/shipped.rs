//! Unselected vstd compiler-boundary reuse experiment (ADR 0048).
//!
//! Native Rust crates, build scripts and proc macros always execute through
//! Cargo's real compiler. Only a full verified vstd dependency invocation can
//! look up a recorded root artifact after those prerequisites complete. Its
//! key binds their actual --extern bytes, every compiler option, the runtime,
//! installed sysroot, solver bytes and the recorded compiler environment.
//!
//! This reuses the original shipped-build serializer and source-path mapping;
//! schema 12 also refuses raw encoded environment-value collisions. It neither
//! reads nor enables the quarantined machine-local v1 store. Explicit recording,
//! machine-local and alternate-bundle modes require `SAIL_VSTD_BOUNDARY_RESEARCH=1`.
//! A valid adjacent release index permits normal serving. Fresh native inputs
//! remain mandatory; staged first-use and its performance remain unqualified.

mod admission;
mod compiler;
pub(crate) mod native;
mod native_reads;
mod release;
mod snapshot;

use super::files::digest;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Layout version of a shipped build and of its keys.
const SCHEMA: u32 = 12;
const KIND: &str = "sail-vstd-compile-boundary";
const MANIFEST: &str = "manifest.json";
const UNITS: &str = "units";
const UNIT: &str = "unit.json";

/// Release tooling: record this run's registry units into the directory named.
pub(crate) const RECORD_ENV: &str = "SAIL_SHIPPED_RECORD";
/// An explicit shipped build directory, instead of the one beside `sail`.
pub(crate) const BUNDLE_ENV: &str = "SAIL_SHIPPED_DEPS";
/// Unselected machine-local boundary cache, bound to the literal registry root.
pub(crate) const LOCAL_ENV: &str = "SAIL_VSTD_BOUNDARY_LOCAL";
/// Sail's Verus: `<virtual base>=<registry source directory>`.
pub(crate) const SOURCE_BASE_ENV: &str = "SAIL_RUSTC_DEV_SOURCE_BASE";
/// Sail's Verus: prefix pairs for imported span strings, one per line.
pub(crate) const SPAN_REMAP_ENV: &str = "SAIL_VIR_SPAN_REMAP";
/// Private plumbing: the run's [`Plan`] for its driver proxies.
pub(crate) const PLAN_ENV: &str = "SAIL_MANAGED_SHIPPED";
/// Diagnosis: a directory that receives, for every registry invocation a
/// serving run looks up, its keyed inputs and the outcome.
pub(crate) const TRACE_ENV: &str = "SAIL_SHIPPED_TRACE";

/// Explicit release-recording, machine-local or alternate-bundle research request.
pub(crate) fn research_enabled() -> bool {
    std::env::var_os("SAIL_VSTD_BOUNDARY_RESEARCH").is_some_and(|value| value == "1")
}

/// Portable source names require ordinary Cargo compiler flags. Unsupported
/// global selectors decline both a new plan and new source-name mapping;
/// unchanged ordinary warm targets still retain their existing naming map.
pub(super) fn portable_context(workspace: &Path, args: &[String]) -> bool {
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")));
    portable_context_inputs(workspace, home.as_deref(), args, std::env::vars_os())
}

fn portable_context_inputs(
    workspace: &Path,
    home: Option<&Path>,
    args: &[String],
    mut environment: impl Iterator<Item = (OsString, OsString)>,
) -> bool {
    if args
        .iter()
        .any(|arg| arg == "--config" || arg.starts_with("--config=") || arg.starts_with("-Z"))
        || environment
            .any(|(name, value)| rust_flag_selector(&name.to_string_lossy()) && !value.is_empty())
    {
        return false;
    }
    super::configuration::paths(workspace, home)
        .into_iter()
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name == "config" || name == "config.toml")
        })
        .all(|path| {
            match std::fs::symlink_metadata(&path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return true,
                Ok(metadata) if metadata.is_file() => {}
                _ => return false,
            }
            let Some(file) = super::dependency_snapshot::open(&path) else {
                return false;
            };
            let mut text = String::new();
            if file
                .take(1024 * 1024 + 1)
                .read_to_string(&mut text)
                .is_err()
                || text.len() > 1024 * 1024
            {
                return false;
            }
            let Ok(config) = toml::from_str::<toml::Value>(&text) else {
                return false;
            };
            config
                .get("env")
                .and_then(toml::Value::as_table)
                .is_none_or(|environment| environment.keys().all(|name| !rust_flag_selector(name)))
                && config
                    .get("build")
                    .and_then(|build| build.get("rustflags"))
                    .is_none()
                && config
                    .get("target")
                    .and_then(toml::Value::as_table)
                    .is_none_or(|targets| {
                        targets
                            .values()
                            .all(|target| target.get("rustflags").is_none())
                    })
        })
}

fn rust_flag_selector(name: &str) -> bool {
    name.ends_with("RUSTFLAGS") || name == "CARGO_ENCODED_RUSTFLAGS"
}

const TARGET: &str = "@@sail-target@@";
const REGISTRY: &str = "@@sail-registry@@";
const PACKAGE: &str = "@@sail-package@@";
const SOLVER_ADAPTER: &str = "@@sail-solver-adapter@@";

/// What a run does with shipped dependencies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case")]
pub(crate) enum Mode {
    /// Serve matching invocations from the shipped build at `bundle`.
    Serve { bundle: PathBuf },
    /// Record every registry invocation into `out` (release tooling).
    Record { out: PathBuf },
    /// Reuse only within this exact registry root, after native compilation.
    MachineLocal { out: PathBuf },
}

/// The run's shipped-dependency plan, read by every driver proxy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Plan {
    pub mode: Mode,
    /// Identity of the shipped build this runtime accepts.
    pub identity: String,
    /// The crates.io registry source directory Cargo uses.
    pub registry: PathBuf,
    /// The run's Cargo target directory.
    pub target: PathBuf,
    /// The pinned toolchain's virtual rustc-dev source base.
    pub virtual_base: String,
    /// Actual solver bytes used by this run, independently of the runtime bundle.
    pub solver_sha256: String,
    /// Exact configured solver adapter, additionally bound for portable entries.
    solver_adapter: Option<SolverAdapter>,
    /// Current full verification, used as provenance when publishing a miss.
    pub run_id: String,
    /// Closed official source/feature policy; independent of stored cache data.
    closure: Option<admission::Closure>,
    /// Per-run successful native compiler observations, never a shared cache.
    native_records: PathBuf,
    /// Actual compiler driver, which embeds rustc and ignores Cargo's hint.
    native_compiler_sha256: String,
    native_launcher: PathBuf,
    native_launcher_sha256: String,
    /// Portable compiler source names, independently rechecked before reuse.
    compiler_source: Option<CompilerSource>,
    /// Per-run admission of a distributed release index, never a proof-key input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    release: Option<release::Admission>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SolverAdapter {
    path: PathBuf,
    sha256: String,
}

/// Imported standard-library locations come from compiled sysroot metadata;
/// these are not additional claims about native `SourceMap` reads. The local
/// source tree also binds diagnostic quoting and the exact added compiler flag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CompilerSource {
    root: PathBuf,
    virtual_root: String,
    tree_sha256: String,
}

impl CompilerSource {
    fn capture(sysroot: &Path, virtual_base: &str, registry: &Path, target: &Path) -> Option<Self> {
        let commit = virtual_base.strip_prefix("/rustc-dev/")?;
        if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let root = sysroot.join("lib/rustlib/src/rust");
        if sysroot.canonicalize().ok()?.as_path() != sysroot
            || root.canonicalize().ok()? != root
            || root.starts_with(registry)
            || registry.starts_with(&root)
            || root.starts_with(target)
            || target.starts_with(&root)
            || root.to_str()?.contains(['=', '@'])
            || root.starts_with("/rustc")
            || root.starts_with("/rustc-dev")
        {
            return None;
        }
        // The pinned complete rust-src tree is larger than a registry crate.
        // Keep that distinct domain bounded without broadening crate admission.
        let files = bounded_source_tree(&root, 128 * 1024 * 1024)?;
        if files.is_empty() {
            return None;
        }
        Some(Self {
            root,
            virtual_root: format!("/rustc/{commit}"),
            tree_sha256: digest(&serde_json::to_vec(&files).ok()?),
        })
    }

    fn current(&self, plan: &Plan) -> Option<()> {
        let sysroot =
            super::runner::verifier_sysroot(&plan.native_launcher, Path::new(".")).ok()?;
        self.current_at(&sysroot, plan)
    }

    fn current_at(&self, sysroot: &Path, plan: &Plan) -> Option<()> {
        let current = Self::capture(sysroot, &plan.virtual_base, &plan.registry, &plan.target)?;
        (current == *self).then_some(())
    }

    fn keyed(&self) -> (&str, &str) {
        (&self.virtual_root, &self.tree_sha256)
    }
}

impl SolverAdapter {
    fn intact(&self) -> bool {
        sha256_file(&self.path).is_some_and(|(hash, _)| hash == self.sha256)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    schema: u32,
    kind: String,
    identity: String,
    /// The runtime variant the build was made with (informational).
    runtime: String,
    virtual_base: String,
    /// Recorded invocations: key to `<crate> <package>`.
    units: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct StoredFile {
    name: String,
    sha256: String,
    size: u64,
    executable: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Unit {
    schema: u32,
    key: String,
    crate_name: String,
    package: String,
    verifying: bool,
    /// The keyed arguments and environment, for inspection.
    args: Vec<String>,
    environment: BTreeMap<String, String>,
    /// Complete vstd package file set observed before compilation.
    source_tree: BTreeMap<String, String>,
    /// Full invocation which originally established this complete proof.
    establishing_run: String,
    /// Separate successful verification and erased-compiler input records.
    compiler_reads: Option<compiler::Document>,
    /// Compiler outputs, stored under `files/`; the dep-info is stored with
    /// placeholders.
    files: Vec<StoredFile>,
    dep_info: String,
    /// Every source file the dep-info names (placeholder path, SHA-256).
    sources: Vec<(String, String)>,
    /// Every environment variable the dep-info declares (name, value), with
    /// placeholders.
    declared: Vec<(String, Option<String>)>,
    stdout: StoredFile,
    stderr: StoredFile,
}

/// The value of [`SOURCE_BASE_ENV`] for a registry directory.
pub(crate) fn source_base(virtual_base: &str, registry: &Path) -> OsString {
    let mut value = OsString::from(virtual_base);
    value.push("=");
    value.push(registry);
    value
}

/// The value of [`SPAN_REMAP_ENV`]: prebuilt VIR's span strings name this
/// machine's sources as a local build would (VI-0031). Standard library spans
/// were recorded under `/rustc/<commit>`, which rustc translates to the
/// toolchain's `rust-src` copy when present; registry sources were recorded
/// under the virtual rustc-dev base. `None` when a path cannot be stated on
/// one line or the toolchain is unpinned.
pub(crate) fn span_remap(registry: &Path, rust_src: Option<&Path>) -> Option<OsString> {
    let mut value = OsString::new();
    let mut pair = |from: &str, to: &Path| -> Option<()> {
        let to = to.as_os_str();
        if to.as_encoded_bytes().contains(&b'\n') || !Path::new(to).is_absolute() {
            return None;
        }
        if !value.is_empty() {
            value.push("\n");
        }
        value.push(from);
        value.push("/=");
        value.push(to);
        value.push("/");
        Some(())
    };
    if let Some(rust_src) = rust_src {
        let commit = crate::toolchain::Pins::embedded().ok()?.verus.rust_commit?;
        pair(&format!("/rustc/{commit}"), rust_src)?;
    }
    pair(&virtual_base()?, registry)?;
    Some(value)
}

/// The pinned Rust toolchain's virtual rustc-dev source base, when pinned.
pub(crate) fn virtual_base() -> Option<String> {
    let pins = crate::toolchain::Pins::embedded().ok()?;
    let commit = pins.verus.rust_commit?;
    (commit.len() == 40 && commit.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| format!("/rustc-dev/{commit}"))
}

/// The crates.io registry source directory Cargo uses for a workspace: the
/// parent of every crates.io package's directory in `cargo metadata`.
pub(crate) fn registry_dir(data: &Value) -> Option<PathBuf> {
    let mut found: Option<PathBuf> = None;
    for package in data["packages"].as_array()? {
        let source = package["source"].as_str().unwrap_or_default();
        if !matches!(
            source,
            "registry+https://github.com/rust-lang/crates.io-index"
                | "sparse+https://index.crates.io/"
        ) {
            continue;
        }
        let directory = Path::new(package["manifest_path"].as_str()?)
            .parent()?
            .parent()?
            .to_owned();
        match &found {
            Some(existing) if *existing != directory => return None,
            _ => found = Some(directory),
        }
    }
    found
}

/// Whether a directory can stand for itself in dep-info, JSON and argument
/// text without escaping, and does not change what the verifier decides from
/// a source path (a path containing `verus_builtin` would).
fn plain_directory(path: &Path) -> bool {
    path.is_absolute()
        && path.to_str().is_some_and(|s| {
            !s.contains("verus_builtin")
                && !s.contains("@@")
                && s.chars().all(|c| {
                    !c.is_whitespace() && !c.is_control() && !matches!(c, '\\' | ':' | '#' | '"')
                })
        })
}

fn loader_variables() -> bool {
    std::env::vars_os().any(|(key, _)| {
        let key = key.as_encoded_bytes();
        key.starts_with(b"LD_") || key.starts_with(b"DYLD_")
    })
}

/// The root of the runtime that `cargo_verus` belongs to
/// (`<root>/verus/source/target-verus/release/cargo-verus`).
fn runtime_root(cargo_verus: &Path) -> Option<PathBuf> {
    let root = cargo_verus
        .canonicalize()
        .ok()?
        .ancestors()
        .nth(5)?
        .to_owned();
    root.join(crate::source_build::MANIFEST)
        .is_file()
        .then_some(root)
}

/// Relevant installed compiler bytes under portable sysroot-relative names.
/// This covers the local receipt input set (compiler/Cargo, root libraries
/// and target libraries), including target libraries' nested linker inputs.
/// Unreadable files and unsupported filesystem entries decline reuse.
fn sysroot_identity(sysroot: &Path) -> Option<String> {
    fn files(directory: &Path, out: &mut Vec<PathBuf>, recursive: bool) -> Option<()> {
        for entry in std::fs::read_dir(directory).ok()? {
            let entry = entry.ok()?;
            let kind = entry.file_type().ok()?;
            if kind.is_file()
                || kind.is_symlink() && std::fs::metadata(entry.path()).ok()?.is_file()
            {
                out.push(entry.path());
            } else if kind.is_dir() {
                if recursive {
                    files(&entry.path(), out, true)?;
                }
            } else {
                return None;
            }
        }
        Some(())
    }
    let mut inputs = vec![sysroot.join("bin/rustc"), sysroot.join("bin/cargo")];
    files(&sysroot.join("lib"), &mut inputs, false)?;
    for entry in std::fs::read_dir(sysroot.join("lib/rustlib")).ok()? {
        let entry = entry.ok()?;
        let kind = entry.file_type().ok()?;
        if kind.is_symlink() {
            return None;
        }
        if !kind.is_dir() {
            continue;
        }
        let library = entry.path().join("lib");
        match std::fs::symlink_metadata(&library) {
            Ok(metadata) if metadata.is_dir() => files(&library, &mut inputs, true)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return None,
        }
    }
    let mut portable = BTreeMap::new();
    for path in inputs {
        let relative = path.strip_prefix(sysroot).ok()?.to_str()?.to_owned();
        portable.insert(relative, sha256_file(&path)?);
    }
    Some(digest(&serde_json::to_vec(&portable).ok()?))
}

/// The rustup components besides the compiler, its standard library, Cargo
/// and `rust-src` that installed libraries into the sysroot directories the
/// identity binds (`rustc-dev`, for example). rustc can load them: the
/// dynamic loader can take `rustc-dev`'s copy of `librustc_driver`, and crate
/// lookup reads every library whose name begins with a sysroot crate's
/// (`liballocator_api2` for `alloc`). So they stay bound, and a toolchain
/// with them misses the shipped proof (#264); this names them for its note.
fn extra_components(sysroot: &Path) -> Vec<String> {
    let rustlib = sysroot.join("lib/rustlib");
    let Ok(entries) = std::fs::read_dir(&rustlib) else {
        return Vec::new();
    };
    let mut extra = Vec::new();
    for entry in entries.flatten() {
        let Some(component) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_prefix("manifest-"))
            .map(str::to_owned)
        else {
            continue;
        };
        if ["rustc-", "rust-std-", "cargo-"]
            .iter()
            .any(|core| component.starts_with(core) && !component.starts_with("rustc-dev"))
            || component == "rust-src"
        {
            continue;
        }
        let installs_libraries = std::fs::read_to_string(entry.path()).is_ok_and(|text| {
            text.lines().any(|line| {
                line.strip_prefix("file:").is_some_and(|file| {
                    let mut parts = file.split('/');
                    // The directories `sysroot_identity` binds: `lib/` itself
                    // and each `lib/rustlib/<target>/lib/`.
                    matches!(
                        (parts.next(), parts.next(), parts.next(), parts.next()),
                        (Some("lib"), Some(_), None, _)
                            | (Some("lib"), Some("rustlib"), Some(_), Some("lib"))
                    )
                })
            })
        });
        if installs_libraries {
            extra.push(component);
        }
    }
    extra.sort();
    extra
}

/// The identity a shipped build must have to be used with a runtime: its
/// build manifest, actual installed compiler/sysroot bytes and virtual base.
fn identity(root: &Path, virtual_base: &str) -> Option<String> {
    // Without rustup the launcher may load another system's compiler
    // libraries. That configuration cannot establish this portable binding.
    if std::env::var_os("VERUS_USE_RUSTUP").is_some_and(|value| value == "0") {
        return None;
    }
    let manifest = std::fs::read(root.join(crate::source_build::MANIFEST)).ok()?;
    let runtime = crate::source_build::artifact_hashes(root).ok()?;
    let wrapper = root.join("verus/source/target-verus/release/verus");
    let sysroot = super::runner::verifier_sysroot(&wrapper, Path::new(".")).ok()?;
    let compiler = sysroot_identity(&sysroot)?;
    Some(digest(
        format!(
            "{KIND}-v{SCHEMA}\n{}\n{}\n{virtual_base}\n{compiler}\n",
            digest(&manifest),
            digest(&serde_json::to_vec(&runtime).ok()?)
        )
        .as_bytes(),
    ))
}

/// The shipped build beside the running `sail` (`<release>/deps/<variant>`)
/// or named by [`BUNDLE_ENV`].
fn adjacent_bundle(variant: &OsStr) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    Some(exe.parent()?.parent()?.join("deps").join(variant))
}

fn bundle_dir(variant: &OsStr) -> Option<PathBuf> {
    if research_enabled()
        && let Some(explicit) = std::env::var_os(BUNDLE_ENV).filter(|v| !v.is_empty())
    {
        return Some(PathBuf::from(explicit));
    }
    adjacent_bundle(variant)
}

/// The SAIL3138 reason when Cargo resolved vstd's closure differently from
/// the shipped build's (#237): what differs and, when that covers every
/// difference, the exact `cargo update` commands and vstd feature edit that
/// align it. Sail never applies them itself: they change the user's lock and
/// manifest, and so what stock Cargo builds too.
fn closure_miss(metadata: &Value) -> Option<String> {
    let difference = admission::closure_difference(metadata)?;
    let list = |items: &[String], last: &str| match items {
        [] => String::new(),
        [only] => only.clone(),
        [init @ .., tail] => format!("{} {last} {tail}", init.join(", ")),
    };
    if !difference.alignable
        && difference.packages.len() == 1
        && difference.packages[0].starts_with("this project uses vstd ")
    {
        return Some(difference.packages[0].clone());
    }
    let mut reason = format!(
        "vstd's dependencies differ from the shipped build's: {}",
        list(&difference.packages, "and")
    );
    if difference.alignable {
        let commands: Vec<String> = difference
            .commands
            .iter()
            .map(|command| format!("`{command}`"))
            .collect();
        let feature = "enabling vstd's default features, that is removing `default-features = false` from its dependency";
        let advice = match (commands.is_empty(), difference.vstd_default_off) {
            (false, false) => format!("align them with {}", list(&commands, "and")),
            (false, true) => format!(
                "align them with {}, and by {feature}",
                list(&commands, "and")
            ),
            (true, _) => format!("align them by {feature}"),
        };
        reason.push_str(" (");
        reason.push_str(&advice);
        reason.push(')');
    }
    Some(reason)
}

/// Cheap source-name configuration for an installed bundle, including warm
/// runs. This grants no proof authority: serving still needs the full identity,
/// source, native-byte and compiler-read checks in the run's owned plan.
pub(crate) fn has_adjacent_bundle(cargo_verus: &Path) -> bool {
    let Some(root) = runtime_root(cargo_verus) else {
        return false;
    };
    let Some(variant) = root.file_name() else {
        return false;
    };
    adjacent_bundle(variant)
        .and_then(|bundle| read_manifest(&bundle))
        .is_some_and(|manifest| {
            manifest.schema == SCHEMA
                && manifest.kind == KIND
                && Some(manifest.runtime.as_str()) == variant.to_str()
                && virtual_base().as_ref() == Some(&manifest.virtual_base)
        })
}

fn read_manifest(bundle: &Path) -> Option<Manifest> {
    release::manifest(bundle)
}

/// The shipped build for the runtime at `root`, if one of its identity
/// exists.
fn accepted_bundle(
    root: &Path,
    identity: &str,
    virtual_base: &str,
) -> Option<(PathBuf, Option<release::Admission>)> {
    let bundle = bundle_dir(root.file_name()?)?;
    let manifest = read_manifest(&bundle)?;
    if manifest.schema != SCHEMA
        || manifest.kind != KIND
        || manifest.identity != identity
        || manifest.virtual_base != virtual_base
    {
        return None;
    }
    let release = if research_enabled() {
        None
    } else {
        Some(release::admit(&bundle, root, &manifest)?)
    };
    Some((bundle, release))
}

/// The package name of the project a shipped build is recorded from; its
/// `Cargo.lock` is shipped with the build.
pub(crate) const RECORD_PACKAGE: &str = "sail-shipped-dependencies";
const LOCKFILE: &str = "Cargo.lock";

/// A `Cargo.lock` for a new project named `name` that, like the project the
/// runtime's shipped build was recorded from, depends on vstd alone: the
/// shipped build's resolution with the root package renamed. Cargo then
/// resolves vstd's closure exactly as the shipped build did, instead of to
/// whatever crates.io holds later. `None` without an accepted shipped build.
#[must_use]
pub fn new_project_lockfile(cargo_verus: &Path, name: &str) -> Option<String> {
    if super::research::selected().ok()? == super::research::Mode::RetainedContent {
        return None;
    }
    let virtual_base = virtual_base()?;
    let root = runtime_root(cargo_verus)?;
    // An ordinary install without a bundle needs no shipped-input probe.
    read_manifest(&bundle_dir(root.file_name()?)?)?;
    let identity = identity(&root, &virtual_base)?;
    let (bundle, _) = accepted_bundle(&root, &identity, &virtual_base)?;
    let lock = std::fs::read_to_string(bundle.join(LOCKFILE)).ok()?;
    rename_root(&lock, RECORD_PACKAGE, name)
}

/// `lock` with its package `from` renamed to `to`, the packages in Cargo's
/// order (by name, then version). `None` unless exactly one package is
/// named `from` and none `to`.
fn rename_root(lock: &str, from: &str, to: &str) -> Option<String> {
    const SEPARATOR: &str = "\n[[package]]\n";
    let mut parts = lock.split(SEPARATOR);
    let header = parts.next()?;
    let mut packages: Vec<String> = parts.map(str::to_owned).collect();
    let name_line = |name: &str| format!("name = \"{name}\"");
    let named = |block: &str, name: &str| block.lines().next() == Some(name_line(name).as_str());
    if packages.iter().filter(|b| named(b, from)).count() != 1
        || packages.iter().any(|b| named(b, to))
    {
        return None;
    }
    for block in &mut packages {
        if named(block, from) {
            *block = block.replacen(&name_line(from), &name_line(to), 1);
        }
    }
    let field = |block: &str, field: &str| {
        block
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{field} = \"")))
            .map(|rest| rest.trim_end_matches('"').to_owned())
            .unwrap_or_default()
    };
    packages.sort_by_key(|block| (field(block, "name"), field(block, "version")));
    let mut out = header.to_owned();
    for block in packages {
        out.push_str(SEPARATOR);
        out.push_str(&block);
    }
    Some(out)
}

/// The run's plan: recording when [`RECORD_ENV`] is set, serving when the
/// run starts a new Cargo target and a shipped build of this runtime's
/// identity exists, nothing otherwise.
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "Keep the complete sequential plan admission together; solver and adapter are distinct byte-bound inputs."
)]
pub(crate) fn plan(
    cargo_verus: &Path,
    target: &Path,
    registry: Option<&Path>,
    new_target: bool,
    solver: &Path,
    shim: &Path,
    run_id: &str,
    workspace: &Path,
    cargo_args: &[String],
    metadata: &Value,
) -> Option<Plan> {
    let research = research_enabled();
    if !research && !has_adjacent_bundle(cargo_verus) {
        return None;
    }
    if [
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "VERUS_CONTRIB_HOOKS_FILE",
    ]
    .iter()
    .any(|name| std::env::var_os(name).is_some())
    {
        return None;
    }
    let recording = research
        .then(|| std::env::var_os(RECORD_ENV).filter(|v| !v.is_empty()))
        .flatten();
    let local = research
        .then(|| std::env::var_os(LOCAL_ENV).filter(|value| !value.is_empty()))
        .flatten();
    // Do not silently reinterpret conflicting research modes.
    if recording.is_some() && local.is_some() {
        return None;
    }
    if local.is_none() && !portable_context(workspace, cargo_args) {
        return None;
    }
    // Ordinary unchanged runs do not use a shipped build and must not probe
    // or hash a toolchain solely for an unused shipped-dependency identity.
    if recording.is_none() && local.is_none() && !new_target {
        return None;
    }
    let virtual_base = virtual_base()?;
    let registry = registry?;
    if !plain_directory(registry) || !plain_directory(target) || loader_variables() {
        return None;
    }
    let root = runtime_root(cargo_verus)?;
    if recording.is_none() && local.is_none() {
        read_manifest(&bundle_dir(root.file_name()?)?)?;
    }
    let identity = identity(&root, &virtual_base)?;
    let (mode, release) = if let Some(out) = local {
        (
            Mode::MachineLocal {
                out: std::path::absolute(out).ok()?,
            },
            None,
        )
    } else if let Some(out) = recording {
        (
            Mode::Record {
                out: std::path::absolute(out).ok()?,
            },
            None,
        )
    } else {
        let (bundle, release) = accepted_bundle(&root, &identity, &virtual_base)?;
        (Mode::Serve { bundle }, release)
    };
    let solver_adapter = if matches!(mode, Mode::MachineLocal { .. }) {
        None
    } else {
        Some(SolverAdapter {
            path: shim.to_owned(),
            sha256: sha256_file(shim)?.0,
        })
    };
    let compiler_source = if matches!(mode, Mode::MachineLocal { .. }) {
        None
    } else {
        let sysroot = super::runner::verifier_sysroot(
            &root.join("verus/source/target-verus/release/verus"),
            Path::new("."),
        )
        .ok()?;
        Some(CompilerSource::capture(
            &sysroot,
            &virtual_base,
            registry,
            target,
        )?)
    };
    Some(Plan {
        mode,
        release,
        identity,
        registry: registry.to_owned(),
        target: target.to_owned(),
        virtual_base,
        solver_sha256: sha256_file(solver)?.0,
        solver_adapter,
        run_id: run_id.to_owned(),
        closure: Some(admission::Closure::resolve(metadata, registry)?),
        native_records: target.join(".sail-boundary-native").join(run_id),
        native_compiler_sha256: sha256_file(
            &root.join("verus/source/target-verus/release/rust_verify"),
        )?
        .0,
        native_launcher: root.join("verus/source/target-verus/release/verus"),
        native_launcher_sha256: sha256_file(&root.join("verus/source/target-verus/release/verus"))?
            .0,
        compiler_source,
    })
}

/// Why an installed release's shipped vstd proof does not apply to this run's
/// new target, when [`plan`] declined it (#238). It names the first of `plan`'s
/// cheap refusals that holds. Otherwise the installed compiler, runtime or
/// bundle identity differs from the one the proof was built with, which only
/// a complete hash could tell apart, and this does not repeat it.
pub(crate) fn plan_miss(
    cargo_verus: &Path,
    workspace: &Path,
    cargo_args: &[String],
    registry: Option<&Path>,
    target: &Path,
    metadata: &Value,
) -> String {
    let wrapped = [
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "VERUS_CONTRIB_HOOKS_FILE",
    ]
    .iter()
    .any(|name| std::env::var_os(name).is_some());
    let reason = miss_reason(
        wrapped,
        portable_context(workspace, cargo_args),
        registry,
        loader_variables(),
        target,
    );
    if reason == TOOLCHAIN_MISS
        && let Ok(sysroot) = super::runner::verifier_sysroot(
            &runtime_root(cargo_verus)
                .unwrap_or_default()
                .join("verus/source/target-verus/release/verus"),
            Path::new("."),
        )
    {
        let extra = extra_components(&sysroot);
        if !extra.is_empty() {
            return components_miss(&extra);
        }
    }
    // The shipped closure's admission is cheap to repeat; the compiler and
    // runtime identity is not, so it is the reason left when this holds.
    match registry {
        Some(registry)
            if reason == TOOLCHAIN_MISS
                && admission::Closure::resolve(metadata, registry).is_none() =>
        {
            closure_miss(metadata).unwrap_or_else(|| CLOSURE_MISS.to_owned())
        }
        _ => reason.to_owned(),
    }
}

fn components_miss(extra: &[String]) -> String {
    format!(
        "the pinned Rust toolchain also has {}, whose libraries rustc can load (install a separate toolchain without them, or `rustup component remove` them)",
        extra.join(", ")
    )
}

const TOOLCHAIN_MISS: &str =
    "the installed Rust toolchain or Sail's Verus differs from the one the proof was built with";

/// Also the reason when Cargo resolved vstd's closure differently.
pub(crate) const CLOSURE_MISS: &str =
    "vstd's dependency versions or build settings differ from the shipped build's";

fn miss_reason(
    wrapped: bool,
    portable: bool,
    registry: Option<&Path>,
    loader: bool,
    target: &Path,
) -> &'static str {
    if wrapped {
        return "RUSTC_WRAPPER, RUSTC_WORKSPACE_WRAPPER or VERUS_CONTRIB_HOOKS_FILE is set";
    }
    if !portable {
        return "a Cargo configuration file, `--config`, `-Z` or a RUSTFLAGS-style variable can change how dependencies compile";
    }
    let Some(registry) = registry else {
        return "this project's vstd does not come from crates.io";
    };
    if loader {
        return "an LD_ or DYLD_ variable is set";
    }
    if !plain_directory(registry) || !plain_directory(target) {
        return "the Cargo home or target path contains characters the proof cannot name";
    }
    TOOLCHAIN_MISS
}

/// Completes a recording run: the manifest lists every recorded invocation.
/// It fails unless a verified invocation (vstd) was recorded.
pub(crate) fn finish_record(plan: &Plan, cargo_verus: &Path, workspace: &Path) -> Result<()> {
    let Mode::Record { out } = &plan.mode else {
        return Ok(());
    };
    let failed = |m: String| Error::new("SAIL3134", m);
    // The resolution new projects start from (`sail new`).
    let source_lock = workspace.join(LOCKFILE);
    let destination_lock = out.join(LOCKFILE);
    std::fs::copy(&source_lock, &destination_lock).map_err(|error| {
        failed(format!(
            "copy {} -> {} (source_exists={:?}, destination_parent_exists={:?}): {error}",
            source_lock.display(),
            destination_lock.display(),
            source_lock.try_exists(),
            out.try_exists(),
        ))
    })?;
    let mut units = BTreeMap::new();
    let mut verified = false;
    for entry in std::fs::read_dir(out.join(UNITS))
        .map_err(|e| failed(format!("{}: {e}", out.display())))?
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let unit: Unit = serde_json::from_slice(
            &std::fs::read(entry.path().join(UNIT)).map_err(|e| failed(format!("{name}: {e}")))?,
        )
        .map_err(|e| failed(format!("{name}: {e}")))?;
        verified |= unit.verifying;
        units.insert(name, format!("{} {}", unit.crate_name, unit.package));
    }
    if !verified {
        return Err(failed(
            "no verified registry dependency was recorded".to_owned(),
        ));
    }
    let runtime = runtime_root(cargo_verus)
        .and_then(|root| root.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default();
    let manifest = Manifest {
        schema: SCHEMA,
        kind: KIND.to_owned(),
        identity: plan.identity.clone(),
        runtime,
        virtual_base: plan.virtual_base.clone(),
        units,
    };
    std::fs::write(
        out.join(MANIFEST),
        serde_json::to_vec_pretty(&manifest).map_err(|e| failed(e.to_string()))?,
    )
    .map_err(|e| failed(e.to_string()))
}

/// The plan of the run this driver proxy belongs to.
pub(crate) fn active() -> Option<Plan> {
    let path = PathBuf::from(std::env::var_os(PLAN_ENV)?);
    let plan: Plan = serde_json::from_slice(&std::fs::read(&path).ok()?).ok()?;
    let runtime_path = PathBuf::from(std::env::var_os("SAIL_MANAGED_RUNTIME")?);
    let runtime: super::receipts::Runtime =
        serde_json::from_slice(&std::fs::read(runtime_path).ok()?).ok()?;
    active_for(&path, plan, &runtime, research_enabled())
}

fn active_for(
    path: &Path,
    plan: Plan,
    runtime: &super::receipts::Runtime,
    research: bool,
) -> Option<Plan> {
    let run = runtime.sidecars.parent()?;
    if path != run.join("shipped.json")
        || run.file_name()? != OsStr::new(&runtime.run_id)
        || plan.run_id != runtime.run_id
        || runtime.retained.is_some()
            && (!matches!(plan.mode, Mode::Serve { .. }) || plan.release.is_none())
        || runtime.engine.is_some()
        || runtime.z3_sha256 != plan.solver_sha256
        || runtime.original_wrapper != plan.native_launcher
    {
        return None;
    }
    if !research {
        let Mode::Serve { .. } = &plan.mode else {
            return None;
        };
        let root = runtime_root(&plan.native_launcher.with_file_name("cargo-verus"))?;
        let executable = std::env::current_exe().ok()?;
        if !ordinary_child(&plan, &root, &executable) {
            return None;
        }
    }
    Some(plan)
}

// A compiler child runs the same Sail bytes under a cache hardlink/copy name.
// canonicalize cannot recover an installed path from either kind of alias.
fn ordinary_child(plan: &Plan, runtime: &Path, executable: &Path) -> bool {
    let check = || {
        let root = runtime.canonicalize().ok()?;
        let engine = root.parent()?;
        if engine.file_name()? != OsStr::new("engine")
            || !root.join(crate::source_build::MANIFEST).is_file()
        {
            return None;
        }
        let adjacent = engine.parent()?.join("deps").join(root.file_name()?);
        let Mode::Serve { bundle } = &plan.mode else {
            return None;
        };
        let manifest = read_manifest(bundle)?;
        let release = plan.release.as_ref()?;
        (ordinary_serve(plan, &adjacent, &manifest)
            && release.intact(bundle)
            && release.executable_matches(bundle, executable))
        .then_some(())
    };
    check().is_some()
}

fn ordinary_serve(plan: &Plan, adjacent: &Path, manifest: &Manifest) -> bool {
    matches!(&plan.mode, Mode::Serve { bundle } if bundle == adjacent)
        && manifest.schema == SCHEMA
        && manifest.kind == KIND
        && manifest.identity == plan.identity
        && manifest.virtual_base == plan.virtual_base
}

/// Replaces `from` by `to` wherever `from` occurs as a whole path prefix:
/// preceded by the start or a separator, and followed by the end, `/` or a
/// separator.
fn replace_path(text: &str, from: &str, to: &str) -> String {
    let separator = |c: char| matches!(c, '=' | ' ' | '"' | '\n' | '\t' | ':' | ',' | '\'');
    let mut out = String::with_capacity(text.len());
    let (mut copied, mut search) = (0, 0);
    while let Some(found) = text[search..].find(from) {
        let index = search + found;
        let end = index + from.len();
        let before = text[..index].chars().next_back();
        let after = text[end..].chars().next();
        if before.is_none_or(separator) && after.is_none_or(|c| c == '/' || separator(c)) {
            out.push_str(&text[copied..index]);
            out.push_str(to);
            copied = end;
            search = end;
        } else {
            search = index + from.chars().next().map_or(1, char::len_utf8);
        }
    }
    out.push_str(&text[copied..]);
    out
}

/// Text with this machine's directories and package id as placeholders, and
/// back.
struct Normalizer {
    pairs: Vec<(String, &'static str)>,
}

impl Normalizer {
    fn new(plan: &Plan, package: &str) -> Option<Self> {
        let mut pairs = vec![
            (plan.target.to_str()?.to_owned(), TARGET),
            (plan.registry.to_str()?.to_owned(), REGISTRY),
        ];
        // The longer directory first, in case one contains the other.
        pairs.sort_by_key(|(path, _)| std::cmp::Reverse(path.len()));
        pairs.push((package.to_owned(), PACKAGE));
        Some(Self { pairs })
    }

    fn environment_name(&self, name: &str, localize: bool) -> String {
        let Some((package, _)) = self.pairs.iter().find(|(_, token)| *token == PACKAGE) else {
            return name.to_owned();
        };
        for prefix in VERUS_OWN {
            if name.strip_prefix(prefix) == Some(if localize { PACKAGE } else { package }) {
                return format!("{prefix}{}", if localize { package } else { PACKAGE });
            }
        }
        name.to_owned()
    }

    fn normalize(&self, text: &str) -> String {
        let mut text = text.to_owned();
        for (actual, token) in &self.pairs {
            text = if *token == PACKAGE {
                text.replace(actual.as_str(), token)
            } else {
                replace_path(&text, actual, token)
            };
        }
        text
    }

    fn localize(&self, text: &str) -> String {
        let mut text = text.to_owned();
        for (actual, token) in &self.pairs {
            text = text.replace(token, actual);
        }
        text
    }
}

/// The package id Cargo-Verus gives the package being compiled
/// (`cargo-verus` `make_package_id`).
fn package_id(lookup: &impl Fn(&str) -> Option<String>) -> Option<String> {
    let name = lookup("CARGO_PKG_NAME")?;
    let version = lookup("CARGO_PKG_VERSION")?;
    let manifest = lookup("CARGO_MANIFEST_DIR")?;
    let hash = digest(
        Path::new(&manifest)
            .join("Cargo.toml")
            .to_string_lossy()
            .as_bytes(),
    );
    Some(format!("{name}-{version}-{}", hash.get(..12)?))
}

/// Variables Cargo sets for one compiler invocation, which the compiler and
/// the crate's macros may read.
const CARGO_SET: &[&str] = &[
    "CARGO_CRATE_NAME",
    "CARGO_MANIFEST_DIR",
    "CARGO_MANIFEST_PATH",
    "CARGO_MANIFEST_LINKS",
    "CARGO_PRIMARY_PACKAGE",
    "CARGO_RUSTC_CURRENT_DIR",
    "CARGO_BIN_NAME",
    "CARGO_TARGET_TMPDIR",
    "OUT_DIR",
    "__CARGO_DEFAULT_LIB_METADATA",
];
/// `RUST*` variables that name this machine's files or only affect a crash.
/// `RUSTUP_*` (below) are unbound too: rustup's proxies set
/// `RUSTUP_TOOLCHAIN` to the user's own default, and the Verus launcher runs
/// its driver with its own pinned toolchain (`rustup run <toolchain>`).
const RUST_UNBOUND: &[&str] = &[
    "RUSTC",
    "RUSTDOC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUST_RECURSION_COUNT",
    "RUST_BACKTRACE",
    "RUST_LIB_BACKTRACE",
];
/// Single variables the compiler, the Verus driver or a dependency's macros
/// read: `VSTD_KIND` (Verus's `builtin_macros`) and Sail's solver mode.
/// Apple's link settings remain bound too: an invalid SDK or deployment
/// target may fail a compilation that succeeded during release recording.
const NAMES: &[&str] = &[
    "VSTD_KIND",
    "SAIL_SOLVER_MODE",
    "SDKROOT",
    "MACOSX_DEPLOYMENT_TARGET",
];
const VERUS_OWN: &[&str] = &[
    " __VERUS_DRIVER_ARGS_FOR_",
    "__VERUS_DRIVER_VERIFY_",
    " __VERUS_DRIVER_IS_BUILTIN_",
    " __VERUS_DRIVER_IS_BUILTIN_MACROS_",
];

/// The name under which a variable is bound, if it is: the invocation's own
/// Cargo variables, the compiler's and Verus's variables, and the Verus
/// driver's variables for this package (with its id as a placeholder).
fn bound_name(name: &str, package: &str) -> Option<String> {
    if name.starts_with("CARGO_PKG_") || CARGO_SET.contains(&name) || NAMES.contains(&name) {
        return Some(name.to_owned());
    }
    if name.starts_with("RUST") {
        return (!name.starts_with("RUSTUP_") && !RUST_UNBOUND.contains(&name))
            .then(|| name.to_owned());
    }
    if name.starts_with("VERUS_") {
        return (name != "VERUS_Z3_PATH").then(|| name.to_owned());
    }
    if name.starts_with("VARGO_") || name.starts_with("Z3_") {
        return Some(name.to_owned());
    }
    if name == "__VERUS_DRIVER_VIA_CARGO__" || name == " __VERUS_DRIVER_ARGS__" {
        return Some(name.to_owned());
    }
    for prefix in VERUS_OWN {
        if let Some(rest) = name.strip_prefix(prefix) {
            return (rest == package).then(|| format!("{prefix}{PACKAGE}"));
        }
    }
    None
}

/// cargo-verus's separator before each argument it packs into one variable.
const ARGS_SEP: &str = "__VERUS_DRIVER_ARGS_SEP__";
/// The rlimit witness the release recorder stores beside a unit (ADR 0060).
pub(super) const RLIMIT_WITNESS: &str = "rlimit.json";

/// Verus's default `--rlimit`, the one the release records the shipped proof
/// at (its witness's `recorded_rlimit`).
const DEFAULT_RLIMIT: f64 = 10.0;

/// `items` without their one `--rlimit N` (or `--rlimit=N`), and N, when they
/// hold exactly one such flag with a finite positive decimal N (ADR 0060).
/// Anything else returns `None`.
fn split_rlimit<'a>(items: &[&'a str]) -> Option<(Vec<&'a str>, f64)> {
    let mut found = None;
    let mut kept = Vec::with_capacity(items.len());
    let mut rest = items.iter();
    while let Some(&item) = rest.next() {
        let value = if item == "--rlimit" {
            Some(*rest.next()?)
        } else {
            item.strip_prefix("--rlimit=")
        };
        match value {
            Some(value) => {
                if found.is_some()
                    || value.is_empty()
                    || !value.bytes().all(|b| b.is_ascii_digit() || b == b'.')
                {
                    return None;
                }
                let n: f64 = value.parse().ok()?;
                if !n.is_finite() || n <= 0.0 {
                    return None;
                }
                found = Some(n);
            }
            None => kept.push(item),
        }
    }
    Some((kept, found?))
}

/// A package's packed forwarded Verus arguments without its one `--rlimit N`
/// (or `--rlimit=N`), and N, when they hold exactly one such flag with a
/// finite positive decimal N (ADR 0060). Anything else returns `None`.
fn without_rlimit(packed: &str) -> Option<(String, f64)> {
    let items: Vec<&str> = packed.strip_prefix(ARGS_SEP)?.split(ARGS_SEP).collect();
    let (kept, n) = split_rlimit(&items)?;
    if kept.is_empty() {
        return None;
    }
    let packed = kept.iter().fold(String::new(), |mut packed, item| {
        packed.push_str(ARGS_SEP);
        packed.push_str(item);
        packed
    });
    Some((packed, n))
}

/// Why the shipped vstd proof missed because of the Verus flags this run's
/// Cargo arguments forward to vstd (`cargo verus verify` forwards everything
/// after `--` to every verified package unless `--fwd-verus-args-to` says
/// otherwise). Exact keys bind those flags; only one raised `--rlimit` is
/// served under the proof's rlimit witness (ADR 0060), so that case returns
/// `None`, as does a run that forwards nothing to vstd.
pub(crate) fn forwarded_miss(cargo_args: &[String]) -> Option<String> {
    let split = cargo_args.iter().position(|arg| arg == "--")?;
    let (cargo, verus) = cargo_args.split_at(split);
    let verus: Vec<&str> = verus.iter().skip(1).map(String::as_str).collect();
    let mut to = None;
    let mut rest = cargo.iter();
    while let Some(arg) = rest.next() {
        if arg == "--fwd-verus-args-to" {
            to = rest.next().map(String::as_str);
        } else if let Some(value) = arg.strip_prefix("--fwd-verus-args-to=") {
            to = Some(value);
        }
    }
    if verus.is_empty() || to.is_some_and(|to| to != "all") {
        return None;
    }
    let keep_off = "pass `--fwd-verus-args-to roots` before `--` to keep";
    match split_rlimit(&verus) {
        Some((kept, n)) if kept.is_empty() => (n < DEFAULT_RLIMIT).then(|| {
            format!(
                "`{}` reaches vstd and is below the default rlimit {DEFAULT_RLIMIT} the proof was recorded at, which can change vstd's results ({keep_off} it off vstd)",
                verus.join(" ")
            )
        }),
        _ => Some(format!(
            "the Verus flags after `--` (`{}`) reach vstd too and the proof binds them; only a raised `--rlimit` is exempt ({keep_off} them off vstd)",
            verus.join(" ")
        )),
    }
}

/// The record a raised `--rlimit` is served under (ADR 0060): the package's
/// forwarded-arguments variable, its value without the flag (as recorded) and
/// as this invocation has it, and the requested rlimit.
struct Subsumed {
    name: String,
    recorded: String,
    current: String,
    requested: f64,
}

/// Which stored unit a lookup serves: the invocation's own key, or the key of
/// the same invocation without its `--rlimit`.
struct Lookup<'k> {
    key: &'k str,
    rlimit: Option<Subsumed>,
}

/// What `units/<key>/rlimit.json` records: the recorder verified the same
/// recipe as recorded and with a raised `--rlimit`, and compared every solver
/// process (ADR 0060).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RlimitWitness {
    schema: u32,
    kind: String,
    key: String,
    recorded_rlimit: f64,
    witness_rlimit: f64,
    recorded_solver_limit: u64,
    witness_solver_limit: u64,
    max_query: u64,
    max_query_limit: u64,
    over_limit_queries: u64,
    queries: u64,
    unknown_answers: u64,
    solver_processes: u64,
    inputs_equal: bool,
    answers_and_counts_equal: bool,
    normalized_inputs_sha256: String,
}

impl RlimitWitness {
    /// Whether this witness lets a unit recorded at `recorded_rlimit` serve
    /// an invocation that asks for `requested`.
    fn admits(&self, key: &str, requested: f64) -> bool {
        self.schema == 1
            && self.kind == "sail-rlimit-witness"
            && self.key == key
            && self.inputs_equal
            && self.answers_and_counts_equal
            && self.unknown_answers == 0
            && self.queries > 0
            && self.solver_processes > 0
            && self.normalized_inputs_sha256.len() == 64
            && self.over_limit_queries == 0
            && self.max_query < self.max_query_limit
            && self.witness_rlimit > self.recorded_rlimit
            && self.witness_solver_limit > self.recorded_solver_limit
            && self.recorded_rlimit > 0.0
            && requested >= self.recorded_rlimit
    }
}

/// One compiler invocation of a registry package under a plan.
pub(crate) struct Call<'a> {
    plan: &'a Plan,
    arguments: &'a [OsString],
    normalizer: Normalizer,
    key: String,
    /// The invocation's whole environment (UTF-8 variables).
    vars: BTreeMap<String, String>,
    keyed_args: Vec<String>,
    environment: BTreeMap<String, String>,
    crate_name: String,
    package: String,
    out_dir: PathBuf,
    stem: String,
    verifying: bool,
    externs: Vec<(String, String, Option<String>)>,
    source_tree: Option<BTreeMap<String, String>>,
    compiler_observation: Option<compiler::Observation>,
    compiler_reads: Option<compiler::Document>,
    native_snapshot: Option<snapshot::Snapshot>,
}

/// Observe every package file, including files that dep-info does not yet
/// name. Added/removed modules must also invalidate a recorded compilation.
fn package_tree(root: &Path) -> Option<BTreeMap<String, String>> {
    bounded_source_tree(root, 64 * 1024 * 1024)
}

fn bounded_source_tree(root: &Path, byte_limit: u64) -> Option<BTreeMap<String, String>> {
    fn visit(
        root: &Path,
        directory: &Path,
        files: &mut BTreeMap<String, String>,
        bytes: &mut u64,
        byte_limit: u64,
    ) -> Option<()> {
        for entry in std::fs::read_dir(directory).ok()? {
            let entry = entry.ok()?;
            let kind = entry.file_type().ok()?;
            if kind.is_dir() {
                visit(root, &entry.path(), files, bytes, byte_limit)?;
            } else if kind.is_file() {
                *bytes = bytes.checked_add(entry.metadata().ok()?.len())?;
                if *bytes > byte_limit || files.len() >= 16_384 {
                    return None;
                }
                files.insert(
                    entry.path().strip_prefix(root).ok()?.to_str()?.to_owned(),
                    sha256_file(&entry.path())?.0,
                );
            } else {
                return None;
            }
        }
        Some(())
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files, &mut 0, byte_limit)?;
    Some(files)
}

/// The keyed arguments of an invocation (this machine's directories as
/// placeholders) and the
/// digests of the crates it links.
#[allow(
    clippy::type_complexity,
    reason = "One (argument, library digest, VIR digest) triple per linked crate."
)]
fn keyed_arguments(
    text: &[String],
    normalizer: &Normalizer,
) -> std::result::Result<(Vec<String>, Vec<(String, String, Option<String>)>), String> {
    // The rustc Cargo names is first; only its name reaches the driver.
    let mut keyed_args = vec![
        text.first()
            .and_then(|rustc| Path::new(rustc).file_name())
            .ok_or("no compiler argument")?
            .to_string_lossy()
            .into_owned(),
    ];
    let mut externs = Vec::new();
    for (index, arg) in text.iter().enumerate().skip(1) {
        // Keep every compiler option exactly: even rendering and capped lint
        // options can produce argument errors, and forced warnings bypass
        // --cap-lints. Different Cargo versions may therefore safely miss.
        let external = arg
            .strip_prefix("--extern=")
            .or_else(|| (text[index - 1] == "--extern").then_some(arg.as_str()));
        if external.is_some_and(|external| !external.contains('=') && external != "proc_macro") {
            return Err("an --extern lookup has no explicit artifact path".to_owned());
        }
        if let Some(external) = external
            && let Some((_, path)) = external.split_once('=')
        {
            let path = Path::new(path);
            let library = sha256_file(path)
                .ok_or_else(|| format!("unreadable --extern {}", path.display()))?
                .0;
            let vir = path.with_extension("vir");
            let vir = match std::fs::symlink_metadata(&vir) {
                Ok(_) => Some(
                    sha256_file(&vir)
                        .ok_or_else(|| format!("unreadable --extern VIR {}", vir.display()))?
                        .0,
                ),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(format!("--extern VIR {}: {error}", vir.display())),
            };
            externs.push((normalizer.normalize(arg), library, vir));
        }
        keyed_args.push(normalizer.normalize(arg));
    }
    Ok((keyed_args, externs))
}

fn argument_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| w[1].clone())
        .or_else(|| {
            args.iter()
                .find_map(|a| a.strip_prefix(&format!("{flag}=")).map(str::to_owned))
        })
}

fn codegen_value(args: &[String], option: &str) -> Option<String> {
    let prefix = format!("{option}=");
    args.windows(2)
        .find_map(|w| (w[0] == "-C").then(|| w[1].strip_prefix(&prefix).map(str::to_owned))?)
        .or_else(|| {
            args.iter()
                .find_map(|a| a.strip_prefix(&format!("-C{prefix}")).map(str::to_owned))
        })
}

fn sha256_file(path: &Path) -> Option<(String, u64)> {
    let mut input = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    let mut size = 0u64;
    loop {
        let n = input.read(&mut buffer).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        size += n as u64;
    }
    Some((format!("{:x}", hasher.finalize()), size))
}

impl<'a> Call<'a> {
    /// The only production entry into the prototype. Cargo has already
    /// compiled and run the native dependency units before this invocation;
    /// their actual library bytes remain inputs to the lookup. No build
    /// script, proc macro, ordinary Rust crate or requested project is served.
    pub(crate) fn vstd_boundary(
        plan: &'a Plan,
        args: &'a [OsString],
        vars: impl Iterator<Item = (OsString, OsString)>,
    ) -> Option<Self> {
        if !plan.closure.as_ref()?.check_sources() {
            return None;
        }
        let mut call = Self::vstd_invocation(plan, args, vars)?;
        if !plan.closure.as_ref()?.vstd_unit(&call.vars, args) {
            call.trace("actual vstd compiler features differ from the admitted source closure");
            return None;
        }
        if !native::compiler_matches(&call) || !native::externs_are_fresh(&call) {
            call.trace("native dependencies lack this run's successful compiler provenance");
            return None;
        }
        let Some(snapshot) = snapshot::Snapshot::capture(&call) else {
            call.trace("native input bytes could not be privately captured");
            return None;
        };
        call.native_snapshot = Some(snapshot);
        Some(call)
    }

    fn vstd_invocation(
        plan: &'a Plan,
        args: &'a [OsString],
        vars: impl Iterator<Item = (OsString, OsString)>,
    ) -> Option<Self> {
        let vars: Vec<_> = vars.collect();
        let lookup = |key: &str| {
            vars.iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value)
        };
        if lookup("CARGO_PKG_NAME")? != "vstd" {
            return None;
        }
        let version = crate::toolchain::Pins::embedded().ok()?.verus.vstd?;
        if lookup("CARGO_PKG_VERSION")? != version.as_str()
            || lookup("CARGO_PRIMARY_PACKAGE").is_some()
            || Path::new(lookup("CARGO_MANIFEST_DIR")?)
                != plan.registry.join(format!("vstd-{version}"))
        {
            return None;
        }
        let manifest: toml::Value = toml::from_str(
            &std::fs::read_to_string(Path::new(lookup("CARGO_MANIFEST_DIR")?).join("Cargo.toml"))
                .ok()?,
        )
        .ok()?;
        let metadata = manifest.get("package")?.get("metadata")?.get("verus")?;
        if metadata.get("verify")?.as_bool() != Some(true)
            || metadata.get("is-vstd")?.as_bool() != Some(true)
        {
            return None;
        }
        let root = PathBuf::from(lookup("CARGO_MANIFEST_DIR")?);
        let source_tree = package_tree(&root)?;
        let mut call = Self::new(plan, args, true, vars.into_iter())?;
        call.source_tree = Some(source_tree);
        (call.crate_name == "vstd"
            && argument_value(&call.keyed_args, "--crate-type")
                .is_some_and(|kind| matches!(kind.as_str(), "lib" | "rlib")))
        .then(|| {
            call.trace("native-prerequisites-observed");
            call
        })
    }

    /// The invocation, if it compiles a crates.io package under `plan` and
    /// can be keyed; `vars` is the invocation's environment. When recording,
    /// a registry invocation that cannot be keyed is listed under `skipped/`
    /// with the reason.
    pub(crate) fn new(
        plan: &'a Plan,
        args: &'a [OsString],
        verifying: bool,
        vars: impl Iterator<Item = (OsString, OsString)>,
    ) -> Option<Self> {
        // An undecodable input must miss, rather than masquerade as absent.
        let vars: BTreeMap<String, String> = vars
            .map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
            .collect::<Option<_>>()?;
        let manifest = PathBuf::from(vars.get("CARGO_MANIFEST_DIR")?);
        if manifest.parent()? != plan.registry {
            return None;
        }
        match Self::keyed(plan, args, verifying, vars) {
            Ok(call) => Some(call),
            Err(reason) => {
                if let Mode::Record { out } | Mode::MachineLocal { out } = &plan.mode {
                    let skipped = out.join("skipped");
                    let _ = std::fs::create_dir_all(&skipped);
                    let name = manifest.file_name().unwrap_or_default().to_string_lossy();
                    let _ = std::fs::write(
                        skipped.join(format!("unkeyed-{name}-{}", std::process::id())),
                        reason,
                    );
                }
                None
            }
        }
    }

    fn keyed(
        plan: &'a Plan,
        args: &'a [OsString],
        verifying: bool,
        vars: BTreeMap<String, String>,
    ) -> std::result::Result<Self, String> {
        if plan.closure.is_some()
            && !matches!(plan.mode, Mode::MachineLocal { .. })
            && plan.solver_adapter.is_none()
        {
            return Err("portable solver adapter binding is missing".to_owned());
        }
        if let Some(adapter) = &plan.solver_adapter
            && (!adapter.intact()
                || vars.get("VERUS_Z3_PATH").map(String::as_str) != adapter.path.to_str())
        {
            return Err("configured solver adapter path or bytes differ".to_owned());
        }
        let lookup = |name: &str| vars.get(name).cloned();
        let package = package_id(&lookup).ok_or("no Cargo package identity")?;
        let normalizer = Normalizer::new(plan, &package).ok_or("non-UTF-8 directories")?;
        let text: Vec<String> = args
            .iter()
            .map(|a| a.to_str().map(str::to_owned))
            .collect::<Option<_>>()
            .ok_or("a non-UTF-8 argument")?;
        let crate_name = argument_value(&text, "--crate-name").ok_or("no --crate-name")?;
        let out_dir = PathBuf::from(argument_value(&text, "--out-dir").ok_or("no --out-dir")?);
        let stem = format!(
            "{crate_name}{}",
            codegen_value(&text, "extra-filename").unwrap_or_default()
        );
        let (keyed_args, externs) = keyed_arguments(&text, &normalizer)?;
        let mut environment = BTreeMap::new();
        for (name, value) in &vars {
            if let Some(bound) = bound_name(name, &package) {
                environment.insert(bound, normalizer.normalize(value));
            }
        }
        let registry_scope = if matches!(plan.mode, Mode::MachineLocal { .. }) {
            Some(registry_forms(&plan.registry))
        } else {
            None
        };
        let mut identity = serde_json::json!({
            "kind": KIND,
            "schema": SCHEMA,
            "identity": plan.identity,
            "solver_sha256": plan.solver_sha256,
            "literal_registry_scope": registry_scope,
            "source_policy": plan.closure.as_ref().map(admission::Closure::policy_sha256),
            "verifying": verifying,
            "args": &keyed_args,
            "environment": &environment,
            "externs": &externs,
        });
        if let Some(adapter) = &plan.solver_adapter {
            identity["solver_adapter_sha256"] = serde_json::json!(adapter.sha256);
        }
        if let Some(source) = &plan.compiler_source {
            identity["compiler_source"] = serde_json::json!(source.keyed());
        }
        let key = digest(&serde_json::to_vec(&identity).map_err(|e| e.to_string())?);
        let package = format!(
            "{}-{}",
            lookup("CARGO_PKG_NAME").unwrap_or_default(),
            lookup("CARGO_PKG_VERSION").unwrap_or_default()
        );
        Ok(Self {
            plan,
            arguments: args,
            normalizer,
            key,
            vars,
            keyed_args,
            environment,
            crate_name,
            package,
            out_dir,
            stem,
            verifying,
            externs,
            source_tree: None,
            compiler_observation: None,
            compiler_reads: None,
            native_snapshot: None,
        })
    }

    fn adapter_intact(&self) -> bool {
        self.plan.solver_adapter.as_ref().is_none_or(|adapter| {
            adapter.intact()
                && self.vars.get("VERUS_Z3_PATH").map(String::as_str) == adapter.path.to_str()
        })
    }

    fn environment_name(&self, name: &str, localize: bool) -> String {
        if matches!(self.plan.mode, Mode::MachineLocal { .. }) {
            name.to_owned()
        } else {
            self.normalizer.environment_name(name, localize)
        }
    }

    fn environment_value(&self, name: &str, value: &str, localize: bool) -> String {
        // These are Cargo's documented path-valued variables, not families:
        // an unknown VERUS_*, RUST*, Z3_*, VARGO_* or CARGO_PKG_* read remains
        // literal even if the lookup key's older environment view maps it.
        const PATH_VALUES: &[&str] = &[
            "CARGO_MANIFEST_DIR",
            "CARGO_MANIFEST_PATH",
            "CARGO_RUSTC_CURRENT_DIR",
            "CARGO_TARGET_TMPDIR",
            "OUT_DIR",
        ];
        if name == "VERUS_Z3_PATH"
            && let Some(adapter) = &self.plan.solver_adapter
            && let Some(path) = adapter.path.to_str()
            && value == if localize { SOLVER_ADAPTER } else { path }
        {
            return if localize { path } else { SOLVER_ADAPTER }.to_owned();
        }
        if !matches!(self.plan.mode, Mode::MachineLocal { .. }) && !PATH_VALUES.contains(&name) {
            return value.to_owned();
        }
        if localize {
            self.normalizer.localize(value)
        } else {
            self.normalizer.normalize(value)
        }
    }

    fn mapped_environment(
        &self,
        values: &BTreeMap<String, Option<String>>,
        localize: bool,
    ) -> Option<BTreeMap<String, Option<String>>> {
        let mut mapped = BTreeMap::new();
        for (name, value) in values {
            // A literal input must not masquerade as our encoded known name.
            if !localize && self.environment_name(name, true) != *name {
                return None;
            }
            let key = self.environment_name(name, localize);
            let value = match value {
                Some(value) => Some(self.mapped_environment_value(
                    if localize { &key } else { name },
                    value,
                    localize,
                )?),
                None => None,
            };
            if mapped.insert(key, value).is_some() {
                return None;
            }
        }
        Some(mapped)
    }

    fn mapped_environment_value(&self, name: &str, value: &str, localize: bool) -> Option<String> {
        // A raw literal must not impersonate an encoded relocatable value.
        // Unknown variables stay literal, and MachineLocal keeps its old map.
        if !localize
            && !matches!(self.plan.mode, Mode::MachineLocal { .. })
            && self.environment_value(name, value, true) != value
        {
            return None;
        }
        Some(self.environment_value(name, value, localize))
    }

    fn dep_info_declarations(&self, text: &str) -> Option<BTreeMap<String, Option<String>>> {
        if matches!(self.plan.mode, Mode::MachineLocal { .. }) {
            Some(
                dep_info_environment(&self.normalizer.normalize(text))
                    .into_iter()
                    .collect(),
            )
        } else {
            self.mapped_environment(&checked_dep_info_environment(text)?, false)
        }
    }

    fn mapped_dep_info(&self, text: &str, localize: bool) -> Option<String> {
        let map_paths = |text: &str| {
            if localize {
                self.normalizer.localize(text)
            } else {
                self.normalizer.normalize(text)
            }
        };
        if matches!(self.plan.mode, Mode::MachineLocal { .. }) {
            return Some(map_paths(text));
        }
        let mut original_names = std::collections::BTreeSet::new();
        let mut mapped_names = std::collections::BTreeSet::new();
        let mut output = String::with_capacity(text.len());
        for line in text.split_inclusive('\n') {
            let body = line.strip_suffix('\n').unwrap_or(line);
            let body = body.strip_suffix('\r').unwrap_or(body);
            let ending = &line[body.len()..];
            if body.trim_start().starts_with("# env-dep") {
                let (name, value) = checked_environment_line(body)?;
                if !localize && self.environment_name(&name, true) != name {
                    return None;
                }
                let mapped_name = self.environment_name(&name, localize);
                if !original_names.insert(name.clone()) || !mapped_names.insert(mapped_name.clone())
                {
                    return None;
                }
                let mapped_value = match &value {
                    Some(value) => Some(self.mapped_environment_value(
                        if localize { &mapped_name } else { &name },
                        value,
                        localize,
                    )?),
                    None => None,
                };
                if mapped_name == name && mapped_value == value {
                    output.push_str(body);
                } else {
                    output.push_str("# env-dep:");
                    output.push_str(&escape_environment(&mapped_name));
                    if let Some(value) = mapped_value {
                        output.push('=');
                        output.push_str(&escape_environment(&value));
                    }
                }
            } else {
                output.push_str(&map_paths(body));
            }
            output.push_str(ending);
        }
        Some(output)
    }

    /// Actual compiler arguments use the already verified private input bytes.
    pub(crate) fn compiler_arguments(&self) -> &[OsString] {
        self.native_snapshot
            .as_ref()
            .map_or(self.arguments, snapshot::Snapshot::arguments)
    }

    fn snapshot_intact(&self) -> bool {
        self.compiler_source_intact()
            && (self.plan.closure.is_none()
                || self
                    .native_snapshot
                    .as_ref()
                    .is_some_and(snapshot::Snapshot::intact))
    }

    fn compiler_source_intact(&self) -> bool {
        self.plan
            .compiler_source
            .as_ref()
            .is_none_or(|source| source.current(self.plan).is_some())
    }

    /// Request observations only for this actual admitted compiler invocation.
    pub(crate) fn observe_compiler(&mut self, command: &mut std::process::Command) {
        self.compiler_observation = compiler::Observation::begin(self);
        if let Some(observation) = &self.compiler_observation {
            observation.configure(command);
        }
    }

    /// Bind observations to the PID returned by the actual compiler spawn.
    pub(crate) fn compiler_started(&mut self, pid: u32) {
        if let Some(observation) = &mut self.compiler_observation {
            observation.started(pid);
        }
    }

    /// Read the fork's completion only after the real child has returned.
    pub(crate) fn compiler_finished(&mut self) {
        self.compiler_reads = self
            .compiler_observation
            .as_ref()
            .and_then(|observation| observation.read(self));
    }

    /// Extra compiler arguments when recording: name registry sources by the
    /// virtual base, under the directory Cargo names and under its canonical
    /// form (rustc's working directory for a registry package). Standard
    /// library files keep rustc's virtual `/rustc/<commit>` names, as a
    /// toolchain without `rust-src` records them, rather than the recording
    /// host's `rust-src` path (VI-0031).
    pub(crate) fn record_args(&self) -> Vec<OsString> {
        if !matches!(self.plan.mode, Mode::Record { .. }) {
            return Vec::new();
        }
        let mut args = Vec::new();
        for directory in registry_forms(&self.plan.registry) {
            let mut mapping = directory.into_os_string();
            mapping.push("=");
            mapping.push(&self.plan.virtual_base);
            args.extend([OsString::from("--remap-path-prefix"), mapping]);
        }
        if let Some(source) = &self.plan.compiler_source {
            let mut mapping = source.root.clone().into_os_string();
            mapping.push("=");
            mapping.push(&source.virtual_root);
            args.extend([OsString::from("--remap-path-prefix"), mapping]);
        }
        args
    }

    fn outputs(&self) -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = std::fs::read_dir(&self.out_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .filter(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                !name.starts_with('.')
                    && (name.starts_with(&self.stem)
                        || name
                            .strip_prefix("lib")
                            .is_some_and(|n| n.starts_with(&self.stem)))
            })
            .map(|e| e.path())
            .collect();
        found.sort();
        found
    }

    /// Records this invocation's outputs and replayable output into the
    /// recording directory. Units that printed diagnostics, wrote anything
    /// unexpected, or whose outputs still name this machine's registry are
    /// not recorded; the reason is kept beside the entries.
    pub(crate) fn record(&self, stdout: &[u8], stderr: &[u8], exit: Option<i32>) {
        let (Mode::Record { out } | Mode::MachineLocal { out }) = &self.plan.mode else {
            return;
        };
        if let Err(reason) = self.try_record(out, stdout, stderr, exit) {
            let skipped = out.join("skipped");
            let _ = std::fs::create_dir_all(&skipped);
            let _ = std::fs::write(
                skipped.join(format!("{}-{}", self.stem, &self.key[..12])),
                reason,
            );
        }
    }

    fn try_record(
        &self,
        out: &Path,
        stdout: &[u8],
        stderr: &[u8],
        exit: Option<i32>,
    ) -> std::result::Result<(), String> {
        if exit != Some(0) {
            return Err(format!("exit {exit:?}"));
        }
        if !quiet(stderr) {
            return Err("the compiler printed diagnostics".to_owned());
        }
        if !self.verifying && !stdout.is_empty() {
            return Err("the compiler printed to stdout".to_owned());
        }
        let stdout = std::str::from_utf8(stdout).map_err(|e| e.to_string())?;
        let stderr = std::str::from_utf8(stderr).map_err(|e| e.to_string())?;
        let staging = out
            .join(UNITS)
            .join(format!(".{}-{}", self.key, std::process::id()));
        let _ = std::fs::remove_dir_all(&staging);
        let staged = std::fs::create_dir_all(staging.join("files"))
            .map_err(|e| e.to_string())
            .and_then(|()| self.stage(&staging, stdout, stderr));
        if let Err(reason) = staged {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(reason);
        }
        if !self.compiler_source_intact() {
            let _ = std::fs::remove_dir_all(&staging);
            return Err("compiler source tree changed before publication".to_owned());
        }
        let entry = out.join(UNITS).join(&self.key);
        if entry.exists() {
            let _ = std::fs::remove_dir_all(&staging);
            return Ok(());
        }
        std::fs::rename(&staging, &entry).map_err(|e| e.to_string())
    }

    /// Copies the invocation's outputs into `staging/files` and returns them
    /// with the dep-info's name and text.
    fn stage_outputs(
        &self,
        staging: &Path,
    ) -> std::result::Result<(Vec<StoredFile>, String, String), String> {
        let registries: Vec<String> = registry_forms(&self.plan.registry)
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        let target = self.plan.target.to_string_lossy().into_owned();
        let mut files = Vec::new();
        let mut dep_info = None;
        for path in self.outputs() {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .ok_or("output without a name")?;
            let bytes = std::fs::read(&path).map_err(|e| format!("{name}: {e}"))?;
            let stored = if name == format!("{}.d", self.stem) {
                let text = String::from_utf8(bytes).map_err(|e| format!("{name}: {e}"))?;
                dep_info = Some((name.clone(), text.clone()));
                self.mapped_dep_info(&text, false)
                    .ok_or("unsupported or ambiguous dep-info environment")?
                    .into_bytes()
            } else {
                if !matches!(self.plan.mode, Mode::MachineLocal { .. })
                    && registries
                        .iter()
                        .any(|registry| contains(&bytes, registry.as_bytes()))
                {
                    return Err(format!("{name} names the registry directory"));
                }
                let library = Path::new(&name)
                    .extension()
                    .is_some_and(|e| e == "dylib" || e == "so");
                if !library && contains(&bytes, target.as_bytes()) {
                    return Err(format!("{name} names the target directory"));
                }
                bytes
            };
            std::fs::write(staging.join("files").join(&name), &stored)
                .map_err(|e| format!("{name}: {e}"))?;
            files.push(StoredFile {
                name: name.clone(),
                sha256: digest(&stored),
                size: stored.len() as u64,
                executable: executable(&path),
            });
        }
        let (dep_info, text) = dep_info.ok_or("no dep-info")?;
        Ok((files, dep_info, text))
    }

    /// Writes the entry into `staging`.
    fn stage(&self, staging: &Path, stdout: &str, stderr: &str) -> std::result::Result<(), String> {
        if !self.snapshot_intact() {
            return Err("private native inputs changed before publication".to_owned());
        }
        if self.plan.closure.is_some() && !native::externs_are_fresh(self) {
            return Err("native compiler provenance changed before publication".to_owned());
        }
        if self
            .plan
            .closure
            .as_ref()
            .is_some_and(|closure| !closure.check_sources())
        {
            return Err(
                "official dependency source closure changed during verification".to_owned(),
            );
        }
        if let Some(before) = &self.source_tree {
            let root = Path::new(self.vars.get("CARGO_MANIFEST_DIR").ok_or("no manifest")?);
            if package_tree(root).as_ref() != Some(before) {
                return Err("vstd source tree changed during compilation".to_owned());
            }
            let current = Self::new(
                self.plan,
                self.arguments,
                self.verifying,
                self.vars
                    .iter()
                    .map(|(key, value)| (key.into(), value.into())),
            )
            .ok_or("cannot recheck compiler inputs")?;
            if current.key != self.key || current.externs != self.externs {
                return Err("native dependency bytes changed during compilation".to_owned());
            }
        }
        let (files, dep_info, text) = self.stage_outputs(staging)?;
        if self
            .plan
            .closure
            .as_ref()
            .is_some_and(|closure| !closure.compiler_sources(&text))
        {
            return Err("compiler-loaded vstd source checksums are not admitted".to_owned());
        }
        let mut sources = Vec::new();
        let manifest =
            Path::new(self.vars.get("CARGO_MANIFEST_DIR").ok_or("no manifest")?).join("Cargo.toml");
        for source in dep_info_sources(&text)
            .into_iter()
            .chain(std::iter::once(manifest.to_string_lossy().into_owned()))
        {
            let normalized = self.normalizer.normalize(&source);
            if !(normalized.starts_with(REGISTRY) || normalized.starts_with(TARGET)) {
                return Err(format!("source outside the registry and target: {source}"));
            }
            let (sha256, _) = sha256_file(Path::new(&source)).ok_or("unreadable source")?;
            sources.push((normalized, sha256));
        }
        let mut declared = self
            .dep_info_declarations(&text)
            .ok_or("unsupported or ambiguous dep-info environment")?;
        let compiler_reads = compiler::publication(self, &mut declared)?;
        let text_file = |name: &str, text: &str| -> std::result::Result<StoredFile, String> {
            let bytes = self.normalizer.normalize(text).into_bytes();
            std::fs::write(staging.join(name), &bytes).map_err(|e| e.to_string())?;
            Ok(StoredFile {
                name: name.to_owned(),
                sha256: digest(&bytes),
                size: bytes.len() as u64,
                executable: false,
            })
        };
        let unit = Unit {
            schema: SCHEMA,
            key: self.key.clone(),
            crate_name: self.crate_name.clone(),
            package: self.package.clone(),
            verifying: self.verifying,
            args: self.keyed_args.clone(),
            environment: self.environment.clone(),
            source_tree: self.source_tree.clone().unwrap_or_default(),
            establishing_run: self.plan.run_id.clone(),
            compiler_reads,
            files,
            dep_info,
            sources,
            declared: declared.into_iter().collect(),
            stdout: text_file("stdout", stdout)?,
            stderr: text_file("stderr", stderr)?,
        };
        std::fs::write(
            staging.join(UNIT),
            serde_json::to_vec_pretty(&unit).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }

    /// Serves this invocation from the shipped build. Every refusal, including
    /// damaged data, is a silent miss followed by normal compilation. The
    /// optional research trace retains the reason without changing diagnostics.
    pub(crate) fn serve(&self) -> Option<Replay> {
        if !self.snapshot_intact() {
            self.trace("private native inputs unavailable");
            return None;
        }
        let bundle = match &self.plan.mode {
            Mode::Serve { bundle } => bundle,
            Mode::MachineLocal { out } => out,
            Mode::Record { .. } => return None,
        };
        let entry = bundle.join(UNITS).join(&self.key);
        let exact = Lookup {
            key: &self.key,
            rlimit: None,
        };
        let mut outcome = self.try_serve(&entry, &exact);
        let mut served = "served".to_owned();
        // Only when nothing is stored under the exact key: a unit recorded
        // without the invocation's one `--rlimit N` (ADR 0060).
        if !entry.exists()
            && let Some((key, subsumed)) = self.rlimit_subsumption()
        {
            let entry = bundle.join(UNITS).join(&key);
            let lookup = Lookup {
                key: &key,
                rlimit: Some(subsumed),
            };
            outcome = self
                .rlimit_witness(bundle, &lookup)
                .and_then(|()| self.try_serve(&entry, &lookup))
                .map_err(|refusal| match refusal {
                    Refusal::Miss(reason) => Refusal::Miss(format!("rlimit-subsumed: {reason}")),
                    Refusal::Damaged(reason) => {
                        Refusal::Damaged(format!("rlimit-subsumed: {reason}"))
                    }
                });
            served = format!("served rlimit-subsumed {key}");
        }
        self.trace(match &outcome {
            Ok(_) => &served,
            Err(Refusal::Miss(reason) | Refusal::Damaged(reason)) => reason,
        });
        outcome.ok()
    }

    /// The key of this vstd invocation without its forwarded `--rlimit N`, when
    /// its forwarded arguments hold exactly one (ADR 0060). Every other argument
    /// and variable is keyed as before.
    fn rlimit_subsumption(&self) -> Option<(String, Subsumed)> {
        if !self.verifying {
            return None;
        }
        let package = package_id(&|name: &str| self.vars.get(name).cloned())?;
        let name = format!("{}{package}", VERUS_OWN[0]);
        let current = self.vars.get(&name)?.clone();
        let (recorded, requested) = without_rlimit(&current)?;
        let mut vars = self.vars.clone();
        vars.insert(name.clone(), recorded.clone());
        let alternative = Self::keyed(self.plan, self.arguments, self.verifying, vars).ok()?;
        (alternative.key != self.key).then_some((
            alternative.key,
            Subsumed {
                name,
                recorded,
                current,
                requested,
            },
        ))
    }

    /// The unit's rlimit witness admits the requested rlimit.
    fn rlimit_witness(
        &self,
        bundle: &Path,
        lookup: &Lookup<'_>,
    ) -> std::result::Result<(), Refusal> {
        let Some(subsumed) = &lookup.rlimit else {
            return Ok(());
        };
        let raw = match &self.plan.release {
            Some(release) => release.read_unit_file(bundle, lookup.key, RLIMIT_WITNESS),
            None => std::fs::read(bundle.join(UNITS).join(lookup.key).join(RLIMIT_WITNESS)).ok(),
        }
        .ok_or_else(|| Refusal::Miss("no rlimit witness".to_owned()))?;
        let witness: RlimitWitness = serde_json::from_slice(&raw)
            .map_err(|e| Refusal::Damaged(format!("unreadable rlimit witness: {e}")))?;
        if witness.admits(lookup.key, subsumed.requested) {
            Ok(())
        } else {
            Err(Refusal::Miss(
                "the rlimit witness does not admit this rlimit".to_owned(),
            ))
        }
    }

    /// Writes this lookup's keyed inputs and outcome under [`TRACE_ENV`].
    fn trace(&self, outcome: &str) {
        let Some(directory) = self.vars.get(TRACE_ENV) else {
            return;
        };
        let directory = Path::new(directory);
        let _ = std::fs::create_dir_all(directory);
        let _ = std::fs::write(
            directory.join(format!("{}-{}.json", self.stem, &self.key[..12])),
            serde_json::to_vec_pretty(&serde_json::json!({
                "crate": self.crate_name,
                "package": self.package,
                "key": self.key,
                "outcome": outcome,
                "args": self.keyed_args,
                "environment": self.environment,
                "externs": self.externs,
            }))
            .unwrap_or_default(),
        );
    }

    fn proof_origin(
        &self,
        unit: &Unit,
        key: &str,
    ) -> std::result::Result<sail_api::project::DependencyBuildOrigin, Refusal> {
        let local = matches!(self.plan.mode, Mode::MachineLocal { .. });
        if local && unit.establishing_run.is_empty() {
            return Err(Refusal::Damaged(
                "missing establishing invocation".to_owned(),
            ));
        }
        Ok(sail_api::project::DependencyBuildOrigin {
            kind: if local {
                sail_api::project::DependencyBuildKind::MachineLocal
            } else {
                sail_api::project::DependencyBuildKind::Release
            },
            identity: format!("{}/{key}", self.plan.identity),
            establishing_run: if local {
                unit.establishing_run.clone()
            } else {
                format!("shipped:{}/{key}", self.plan.identity)
            },
        })
    }

    fn check_declared_environment(
        &self,
        unit: &Unit,
        lookup: &Lookup<'_>,
    ) -> std::result::Result<(), Refusal> {
        for (name, value) in &unit.declared {
            let name = if matches!(self.plan.mode, Mode::MachineLocal { .. }) {
                self.normalizer.localize(name)
            } else {
                self.environment_name(name, true)
            };
            // Under a raised rlimit the unit records the arguments without it.
            let current = match &lookup.rlimit {
                Some(subsumed) if subsumed.name == name => Some(&subsumed.recorded),
                _ => self.vars.get(&name),
            }
            .map(|v| self.environment_value(&name, v, false));
            if current != *value {
                return Err(Refusal::Miss(format!("declared variable {name} differs")));
            }
        }
        Ok(())
    }

    fn read_unit_bytes(&self, entry: &Path, key: &str) -> std::result::Result<Vec<u8>, Refusal> {
        if let Some(release) = &self.plan.release {
            let Mode::Serve { bundle } = &self.plan.mode else {
                return Err(Refusal::Miss(
                    "release admission outside serving mode".to_owned(),
                ));
            };
            release
                .read_unit(bundle, key)
                .ok_or_else(|| Refusal::Miss("release index or unit changed".to_owned()))
        } else {
            std::fs::read(entry.join(UNIT))
                .map_err(|_| Refusal::Miss("no entry for this key".to_owned()))
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "Keep fresh compiler/source, unit, artifact and release admission in one ordered restore boundary."
    )]
    fn try_serve(&self, entry: &Path, lookup: &Lookup<'_>) -> std::result::Result<Replay, Refusal> {
        let bytes = self.read_unit_bytes(entry, lookup.key)?;
        let unit: Unit = serde_json::from_slice(&bytes)
            .map_err(|e| Refusal::Damaged(format!("unreadable entry: {e}")))?;
        if unit.schema != SCHEMA || unit.key != lookup.key || unit.verifying != self.verifying {
            return Err(Refusal::Damaged("entry of another key".to_owned()));
        }
        if self
            .source_tree
            .as_ref()
            .is_some_and(|tree| *tree != unit.source_tree)
        {
            return Err(Refusal::Miss(
                "vstd package file set or bytes differ".to_owned(),
            ));
        }
        let recorded = lookup
            .rlimit
            .as_ref()
            .map(|subsumed| (subsumed.name.as_str(), subsumed.recorded.as_str()));
        if !compiler::stored_for(self, &unit, lookup.key, recorded) {
            return Err(Refusal::Miss(
                "compiler-read verification and compilation observations are unavailable"
                    .to_owned(),
            ));
        }
        let origin = self.proof_origin(&unit, lookup.key)?;
        // Validate every destination before placing any output. A malformed
        // later filename must not leave earlier copied artifacts behind.
        if let Some(file) = unit
            .files
            .iter()
            .find(|file| file.name.contains('/') || file.name.starts_with('.'))
        {
            return Err(Refusal::Damaged(format!("unsafe file name {}", file.name)));
        }
        // This machine's sources and declared variables must be the ones the
        // entry was compiled from; otherwise compiling here is the answer.
        for (source, sha256) in &unit.sources {
            let path = self.normalizer.localize(source);
            // Hash equality does not preserve the filename stock records if
            // a registry file is now redirected by an interior symlink.
            let root_and_relative = source
                .strip_prefix(REGISTRY)
                .map(|rest| (&self.plan.registry, rest))
                .or_else(|| {
                    source
                        .strip_prefix(TARGET)
                        .map(|rest| (&self.plan.target, rest))
                });
            let canonical_name = root_and_relative.and_then(|(root, rest)| {
                Some(root.canonicalize().ok()?.join(rest.strip_prefix('/')?))
            });
            if canonical_name.is_none() || Path::new(&path).canonicalize().ok() != canonical_name {
                return Err(Refusal::Miss(format!("source {path} is redirected")));
            }
            if sha256_file(Path::new(&path)).is_none_or(|(actual, _)| actual != *sha256) {
                return Err(Refusal::Miss(format!("source {path} differs")));
            }
        }
        self.check_declared_environment(&unit, lookup)?;
        let read_text = |path: &Path, file: &StoredFile| -> std::result::Result<String, Refusal> {
            let bytes =
                std::fs::read(path).map_err(|e| Refusal::Damaged(format!("{}: {e}", file.name)))?;
            if digest(&bytes) != file.sha256 {
                return Err(Refusal::Damaged(format!(
                    "{} has another digest",
                    file.name
                )));
            }
            String::from_utf8(bytes).map_err(|e| Refusal::Damaged(e.to_string()))
        };
        let stdout = self
            .normalizer
            .localize(&read_text(&entry.join("stdout"), &unit.stdout)?);
        let stderr = self
            .normalizer
            .localize(&read_text(&entry.join("stderr"), &unit.stderr)?);
        let mut placed = Vec::new();
        for file in &unit.files {
            let from = entry.join("files").join(&file.name);
            let to = self.out_dir.join(&file.name);
            let result = if file.name == unit.dep_info {
                read_text(&from, file).and_then(|text| {
                    let text = self.mapped_dep_info(&text, true).ok_or_else(|| {
                        Refusal::Damaged("unsupported or ambiguous dep-info environment".to_owned())
                    })?;
                    // Name the arguments as this invocation has them, as a
                    // compilation with its `--rlimit` would record them.
                    let text = match &lookup.rlimit {
                        Some(subsumed) => {
                            let recorded = format!(
                                "# env-dep:{}={}",
                                subsumed.name,
                                escape_environment(&subsumed.recorded)
                            );
                            // vstd's dep-info does not declare them today; a
                            // repeated declaration is not a recorded shape.
                            let lines = text.lines().filter(|line| *line == recorded).count();
                            if lines > 1 || (lines == 1 && text.contains('\r')) {
                                return Err(Refusal::Damaged(
                                    "dep-info declares the forwarded arguments twice".to_owned(),
                                ));
                            }
                            let current = format!(
                                "# env-dep:{}={}",
                                subsumed.name,
                                escape_environment(&subsumed.current)
                            );
                            text.split_inclusive('\n')
                                .map(|line| match line.strip_suffix('\n') {
                                    Some(body) if body == recorded => format!("{current}\n"),
                                    None if line == recorded => current.clone(),
                                    _ => line.to_owned(),
                                })
                                .collect::<String>()
                        }
                        None => text,
                    };
                    write_new(&to, text.as_bytes(), false)
                        .map_err(|e| Refusal::Damaged(e.to_string()))
                })
            } else {
                copy_checked(&from, &to, file)
            };
            if let Err(refusal) = result {
                for path in &placed {
                    let _ = std::fs::remove_file(path);
                }
                return Err(refusal);
            }
            placed.push(to);
        }
        if !self.compiler_source_intact() {
            for path in &placed {
                let _ = std::fs::remove_file(path);
            }
            return Err(Refusal::Miss(
                "compiler source tree changed during restore".to_owned(),
            ));
        }
        Ok(Replay {
            stdout: stdout.into_bytes(),
            stderr: stderr.into_bytes(),
            origin,
            retained: RetainedProof::new(self, lookup.key, &bytes, &unit),
        })
    }
}

/// A served invocation's recorded output, with this machine's directories.
pub(crate) struct Replay {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// Scope and establishing invocation, preserved through later reuse.
    pub origin: sail_api::project::DependencyBuildOrigin,
    /// A distinct release-proof producer, never an actual local compiler run.
    pub retained: Option<RetainedProof>,
}

/// Minted only after the complete fresh-native checked Serve boundary succeeds.
/// The retained arena may preserve it, but must recheck the release index/unit,
/// closed sources, compiler and exact placed output on every use.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RetainedProof {
    plan: Plan,
    key: String,
    unit_sha256: String,
    outputs: BTreeMap<PathBuf, super::dependency_snapshot::Content>,
}

impl RetainedProof {
    pub(crate) fn for_run(&self, run: &str, compiler: &str, target: &Path) -> bool {
        self.plan.run_id == run
            && self.plan.native_compiler_sha256 == compiler
            && self.plan.target == target
    }

    pub(crate) fn compiler_identity(&self) -> &str {
        &self.plan.native_compiler_sha256
    }

    fn new(call: &Call<'_>, key: &str, raw: &[u8], unit: &Unit) -> Option<Self> {
        if call.plan.release.is_none()
            || !matches!(call.plan.mode, Mode::Serve { .. })
            || !unit.verifying
            || call.crate_name != "vstd"
            || !call.snapshot_intact()
        {
            return None;
        }
        let outputs = unit
            .files
            .iter()
            .filter(|file| file.name != unit.dep_info)
            .map(|file| {
                let path = call.out_dir.join(&file.name);
                let content = super::dependency_snapshot::read_content(&path, 256 * 1024 * 1024)?;
                (content.sha256 == file.sha256 && content.size == file.size)
                    .then_some((path, content))
            })
            .collect::<Option<_>>()?;
        Some(Self {
            plan: call.plan.clone(),
            key: key.to_owned(),
            unit_sha256: digest(raw),
            outputs,
        })
    }

    pub(crate) fn document(&self) -> Option<PathBuf> {
        let Mode::Serve { bundle } = &self.plan.mode else {
            return None;
        };
        Some(bundle.join(UNITS).join(&self.key).join(UNIT))
    }

    pub(in crate::managed) fn outputs(
        &self,
    ) -> &BTreeMap<PathBuf, super::dependency_snapshot::Content> {
        &self.outputs
    }

    pub(in crate::managed) fn intact(
        &self,
        output: &Path,
        body: &super::dependency_snapshot::Content,
    ) -> bool {
        let check = || {
            (self.plan.closure.as_ref()?.check_sources()
                && self
                    .plan
                    .compiler_source
                    .as_ref()?
                    .current(&self.plan)
                    .is_some()
                && native::launcher_matches(&self.plan, &self.plan.native_launcher)
                && self.release_output_intact(output, body))
            .then_some(())
        };
        check().is_some()
    }

    fn release_output_intact(
        &self,
        output: &Path,
        body: &super::dependency_snapshot::Content,
    ) -> bool {
        let check = || {
            let Mode::Serve { bundle } = &self.plan.mode else {
                return None;
            };
            let raw = self.plan.release.as_ref()?.read_unit(bundle, &self.key)?;
            let unit: Unit = serde_json::from_slice(&raw).ok()?;
            (digest(&raw) == self.unit_sha256
                && unit.schema == SCHEMA
                && unit.key == self.key
                && unit.verifying
                && unit.crate_name == "vstd"
                && self.outputs.get(output) == Some(body)
                && output
                    .parent()
                    .is_some_and(|parent| parent.starts_with(&self.plan.target))
                && unit.files.iter().any(|file| {
                    output.file_name() == Some(OsStr::new(&file.name))
                        && file.sha256 == body.sha256
                        && file.size == body.size
                })
                && super::dependency_snapshot::read_content(output, 256 * 1024 * 1024).as_ref()
                    == Some(body))
            .then_some(())
        };
        check().is_some()
    }
}

enum Refusal {
    Miss(String),
    Damaged(String),
}

/// The registry directory as Cargo names it and, if different, canonical.
fn registry_forms(registry: &Path) -> Vec<PathBuf> {
    let mut forms = vec![registry.to_owned()];
    if let Ok(canonical) = registry.canonicalize()
        && canonical != registry
    {
        forms.push(canonical);
    }
    forms
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

#[cfg(unix)]
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn executable(_: &Path) -> bool {
    false
}

/// Whether a compiler's stderr holds only Cargo's artifact notifications.
fn quiet(stderr: &[u8]) -> bool {
    stderr
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .all(|line| {
            serde_json::from_slice::<Value>(line).is_ok_and(|v| {
                v.get("artifact").is_some_and(Value::is_string)
                    && v.as_object().is_some_and(|o| {
                        o.keys()
                            .all(|k| matches!(k.as_str(), "$message_type" | "artifact" | "emit"))
                    })
            })
        })
}

fn unescape(path: &str) -> String {
    path.replace("\\ ", " ")
}

/// The files a rustc dep-info lists (`<path>:` lines).
fn dep_info_sources(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.strip_suffix(':'))
        .filter(|path| !path.is_empty() && !path.contains(": "))
        .map(unescape)
        .collect()
}

/// The `# env-dep:` declarations of a dep-info: name and value, if any.
fn dep_info_environment(text: &str) -> Vec<(String, Option<String>)> {
    text.lines()
        .filter_map(|line| line.strip_prefix("# env-dep:"))
        .map(|rest| match rest.split_once('=') {
            Some((name, value)) => (name.to_owned(), Some(unescape_env(value))),
            None => (rest.to_owned(), None),
        })
        .collect()
}

// Match pinned rustc's escape_dep_env for names AND values. Unknown escapes
// cannot silently turn into another observed variable or value.
fn checked_environment_line(line: &str) -> Option<(String, Option<String>)> {
    let rest = line.strip_prefix("# env-dep:")?;
    let (name, value) = rest
        .split_once('=')
        .map_or((rest, None), |(name, value)| (name, Some(value)));
    let name = checked_environment_text(name)?;
    if name.is_empty() || name.contains(['=', '\0']) {
        return None;
    }
    let value = match value {
        Some(value) => Some(checked_environment_text(value)?),
        None => None,
    };
    Some((name, value))
}

fn checked_environment_text(text: &str) -> Option<String> {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        result.push(if c == '\\' {
            match chars.next()? {
                'n' => '\n',
                'r' => '\r',
                '\\' => '\\',
                _ => return None,
            }
        } else {
            if matches!(c, '\0' | '\n' | '\r') {
                return None;
            }
            c
        });
    }
    Some(result)
}

fn escape_environment(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn checked_dep_info_environment(text: &str) -> Option<BTreeMap<String, Option<String>>> {
    let mut result = BTreeMap::new();
    for line in text
        .lines()
        .filter(|line| line.trim_start().starts_with("# env-dep"))
    {
        let (name, value) = checked_environment_line(line)?;
        if result.insert(name, value).is_some() {
            return None;
        }
    }
    Some(result)
}

/// rustc escapes `\` and newlines in dep-info environment values.
fn unescape_env(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn write_new(to: &Path, bytes: &[u8], executable: bool) -> std::io::Result<()> {
    let temporary = to.with_file_name(format!(
        ".{}.sail-shipped-{}",
        to.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));
    let _ = std::fs::remove_file(&temporary);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(if executable { 0o755 } else { 0o644 });
    }
    #[cfg(not(unix))]
    let _ = executable;
    let outcome = (|| {
        let mut output = options.open(&temporary)?;
        output.write_all(bytes)?;
        drop(output);
        std::fs::rename(&temporary, to)
    })();
    if outcome.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    outcome
}

/// Copies a stored file into place, checking its digest and size on the
/// bytes read; the destination is replaced only by a complete, checked copy.
fn copy_checked(from: &Path, to: &Path, file: &StoredFile) -> std::result::Result<(), Refusal> {
    let bytes = std::fs::read(from).map_err(|e| Refusal::Damaged(format!("{}: {e}", file.name)))?;
    if bytes.len() as u64 != file.size || digest(&bytes) != file.sha256 {
        return Err(Refusal::Damaged(format!(
            "{} has another digest",
            file.name
        )));
    }
    write_new(to, &bytes, file.executable).map_err(|e| Refusal::Damaged(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_components_name_only_those_installing_bound_libraries()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-components-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("lib/rustlib"))?;
        let manifest = |component: &str, files: &[&str]| {
            let text = files
                .iter()
                .fold(String::new(), |text, file| text + "file:" + file + "\n");
            std::fs::write(root.join(format!("lib/rustlib/manifest-{component}")), text)
        };
        let target = "x86_64-unknown-linux-gnu";
        manifest(
            &format!("rustc-{target}"),
            &["bin/rustc", "lib/librustc_driver-1.so"],
        )?;
        manifest(
            &format!("rust-std-{target}"),
            &["lib/rustlib/x86_64-unknown-linux-gnu/lib/libstd-1.rlib"],
        )?;
        manifest(
            "rust-src",
            &["lib/rustlib/src/rust/library/core/src/lib.rs"],
        )?;
        manifest(&format!("clippy-{target}"), &["bin/cargo-clippy"])?;
        manifest(
            &format!("llvm-tools-{target}"),
            &["lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-objcopy"],
        )?;
        assert_eq!(extra_components(&root), Vec::<String>::new());
        manifest(
            &format!("rustc-dev-{target}"),
            &["lib/rustlib/x86_64-unknown-linux-gnu/lib/librustc_driver-1.so"],
        )?;
        assert_eq!(extra_components(&root), [format!("rustc-dev-{target}")]);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn closure_misses_name_differences_and_the_commands_that_align_them()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let shipped = admission::policy_metadata(Path::new("/r"))?;
        assert_eq!(closure_miss(&shipped), None);
        let shipped_version = |name: &str| -> std::result::Result<String, &str> {
            let mut copy = shipped.clone();
            admission::test_entry(&mut copy, name, "packages")?["version"]
                .as_str()
                .map(str::to_owned)
                .ok_or("no version")
        };
        let unicode = shipped_version("unicode-ident")?;
        let mut vest = shipped.clone();
        admission::test_version(&mut vest, "unicode-ident", "1.0.24")?;
        admission::test_features(&mut vest, "vstd", &["alloc", "std"])?;
        assert_eq!(
            closure_miss(&vest).ok_or("no reason")?,
            format!(
                "vstd's dependencies differ from the shipped build's: unicode-ident is 1.0.24 instead of {unicode} and vstd's default features are off (align them with `cargo update -p unicode-ident --precise {unicode}`, and by enabling vstd's default features, that is removing `default-features = false` from its dependency)"
            )
        );
        let mut defaults = shipped.clone();
        admission::test_features(&mut defaults, "vstd", &["alloc", "std"])?;
        assert!(closure_miss(&defaults).ok_or("no reason")?.ends_with(
            "vstd's default features are off (align them by enabling vstd's default features, that is removing `default-features = false` from its dependency)"
        ));
        let mut verismo = shipped.clone();
        admission::test_features(&mut verismo, "vstd", &["alloc", "allow_panic"])?;
        let reason = closure_miss(&verismo).ok_or("no reason")?;
        assert!(
            !reason.contains("cargo update") && !reason.contains("align"),
            "{reason}"
        );
        let mut release = shipped;
        admission::test_version(&mut release, "vstd", "0.0.0-2026-09-06-0133")?;
        assert!(closure_miss(&release).ok_or("no reason")?.starts_with(
            "this project uses vstd 0.0.0-2026-09-06-0133 and the proof is for vstd "
        ));
        Ok(())
    }

    #[test]
    fn shipped_misses_name_the_first_refusal_that_holds() {
        let registry = Path::new("/home/user/.cargo/registry/src/index.crates.io-1");
        let target = Path::new("/tmp/target");
        let reason = |wrapped, portable, registry, loader, target| {
            miss_reason(wrapped, portable, registry, loader, target)
        };
        assert!(reason(true, false, None, true, target).starts_with("RUSTC_WRAPPER"));
        assert!(reason(false, false, None, true, target).starts_with("a Cargo configuration"));
        assert!(reason(false, true, None, true, target).contains("crates.io"));
        assert!(reason(false, true, Some(registry), true, target).contains("LD_"));
        assert!(
            reason(false, true, Some(registry), false, Path::new("/tmp/a b"))
                .contains("characters")
        );
        assert!(reason(false, true, Some(registry), false, target).contains("toolchain"));
    }

    #[test]
    fn span_remap_names_local_standard_library_and_registry_sources() {
        let commit = crate::toolchain::Pins::embedded()
            .ok()
            .and_then(|pins| pins.verus.rust_commit)
            .unwrap_or_default();
        let registry = Path::new("/home/u/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f");
        let rust_src = Path::new("/home/u/.rustup/toolchains/1.99.1/lib/rustlib/src/rust");
        let base = virtual_base().unwrap_or_default();
        assert_eq!(
            span_remap(registry, None),
            Some(OsString::from(format!("{base}/={}/", registry.display())))
        );
        assert_eq!(
            span_remap(registry, Some(rust_src)),
            Some(OsString::from(format!(
                "/rustc/{commit}/={}/\n{base}/={}/",
                rust_src.display(),
                registry.display()
            )))
        );
        // A path that cannot be stated on one line, or a relative one, selects nothing.
        assert_eq!(span_remap(Path::new("/a\nb"), None), None);
        assert_eq!(span_remap(Path::new("relative"), None), None);
    }

    #[test]
    fn portable_context_refuses_rustflag_selectors_inline_overrides_and_config_aliases()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let machine = Machine::new("portable-context-guard", |root| Mode::MachineLocal {
            out: root.join("local"),
        })?;
        let workspace = machine.root.join("workspace");
        let home = machine.root.join("cargo-home");
        std::fs::create_dir_all(workspace.join(".cargo"))?;
        std::fs::create_dir_all(&home)?;
        let check = |args: &[&str], vars: &[(&str, &str)]| {
            portable_context_inputs(
                &workspace,
                Some(&home),
                &args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>(),
                vars.iter()
                    .map(|(name, value)| ((*name).into(), (*value).into())),
            )
        };
        assert!(check(&[], &[]));
        assert!(check(&[], &[("RUSTFLAGS", "")]));
        for name in [
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "CARGO_BUILD_RUSTFLAGS",
            "CARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS",
        ] {
            assert!(!check(&[], &[(name, "--force-warn=missing_docs")]));
        }
        for args in [
            vec!["--config", "build.rustflags=[]"],
            vec!["--config=build.rustflags=[]"],
            vec!["-Z", "config-include"],
            vec!["-Zconfig-include"],
        ] {
            assert!(!check(&args, &[]));
        }
        let config = workspace.join(".cargo/config.toml");
        std::fs::write(&config, "[build]\njobs=2\n")?;
        assert!(check(&[], &[]));
        for text in [
            "[build]\nrustflags=[]\n",
            "[target.aarch64-apple-darwin]\nrustflags=['--cfg=custom']\n",
            "[env]\nRUSTFLAGS='--force-warn=missing_docs'\n",
            "[env]\nCARGO_ENCODED_RUSTFLAGS={value='--cfg=custom',force=true}\n",
            "[env]\nCARGO_BUILD_RUSTFLAGS={value='--cfg=custom',force=true}\n",
            "[env]\nCARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS='--cfg=custom'\n",
            "[build\n",
        ] {
            std::fs::write(&config, text)?;
            assert!(!check(&[], &[]));
        }
        std::fs::File::create(&config)?.set_len(1024 * 1024 + 1)?;
        assert!(!check(&[], &[]));
        std::fs::remove_file(&config)?;
        #[cfg(unix)]
        {
            let target = machine.root.join("ordinary-config");
            std::fs::write(&target, "[build]\njobs=2\n")?;
            std::os::unix::fs::symlink(target, &config)?;
            assert!(!check(&[], &[]));
        }
        Ok(())
    }

    fn plan(root: &Path, mode: Mode) -> Plan {
        Plan {
            mode,
            identity: "identity".into(),
            registry: root.join("home/registry/src/index.crates.io-0"),
            target: root.join("target"),
            virtual_base: "/rustc-dev/0123".into(),
            solver_sha256: "solver-bytes".into(),
            solver_adapter: None,
            run_id: "original-verification".into(),
            closure: None,
            native_records: root.join("native-observations"),
            native_compiler_sha256: "compiler-bytes".into(),
            native_launcher: root.join("verus"),
            native_launcher_sha256: "launcher-bytes".into(),
            compiler_source: None,
            release: None,
        }
    }

    #[test]
    fn ordinary_activation_accepts_only_matching_adjacent_serve() {
        let root = Path::new("/owned-release");
        let adjacent = root.join("deps/variant");
        let mut call = plan(
            root,
            Mode::Serve {
                bundle: adjacent.clone(),
            },
        );
        let mut manifest = Manifest {
            schema: SCHEMA,
            kind: KIND.into(),
            identity: call.identity.clone(),
            runtime: "variant".into(),
            virtual_base: call.virtual_base.clone(),
            units: BTreeMap::new(),
        };
        assert!(ordinary_serve(&call, &adjacent, &manifest));
        assert!(!ordinary_serve(&call, &root.join("other"), &manifest));
        manifest.schema -= 1;
        assert!(!ordinary_serve(&call, &adjacent, &manifest));
        manifest.schema = SCHEMA;
        manifest.identity.push('x');
        assert!(!ordinary_serve(&call, &adjacent, &manifest));
        manifest.identity = call.identity.clone();
        call.mode = Mode::Record {
            out: adjacent.clone(),
        };
        assert!(!ordinary_serve(&call, &adjacent, &manifest));
        call.mode = Mode::MachineLocal {
            out: adjacent.clone(),
        };
        assert!(!ordinary_serve(&call, &adjacent, &manifest));
    }

    #[test]
    fn child_plan_is_bound_to_original_run_solver_and_single_authority() {
        let root = Path::new("/owned-family");
        let call = plan(
            root,
            Mode::Record {
                out: root.join("record"),
            },
        );
        let run = root.join("runs").join(&call.run_id);
        let path = run.join("shipped.json");
        let mut runtime = super::super::receipts::Runtime {
            run_id: call.run_id.clone(),
            args: Vec::new(),
            z3: root.join("z3"),
            z3_sha256: call.solver_sha256.clone(),
            daemon: false,
            sidecars: run.join("sidecars"),
            invocations: run.join("invocations"),
            original_wrapper: call.native_launcher.clone(),
            receipts: root.join("receipts"),
            context_sha256: "context".into(),
            input_revision: "source".into(),
            packages: BTreeMap::new(),
            target_kinds: BTreeMap::new(),
            engine: None,
            retained: None,
            attribution_logs: false,
        };
        assert!(active_for(&path, call.clone(), &runtime, true).is_some());
        assert!(active_for(&run.join("foreign.json"), call.clone(), &runtime, true).is_none());
        assert!(active_for(&path, call.clone(), &runtime, false).is_none());
        runtime.run_id.push('x');
        assert!(active_for(&path, call.clone(), &runtime, true).is_none());
        runtime.run_id = call.run_id.clone();
        runtime.z3_sha256.push('x');
        assert!(active_for(&path, call.clone(), &runtime, true).is_none());
        runtime.z3_sha256 = call.solver_sha256.clone();
        runtime.original_wrapper = root.join("foreign-verus");
        assert!(active_for(&path, call, &runtime, true).is_none());
    }

    fn vars(root: &Path, extra: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        let manifest = root.join("home/registry/src/index.crates.io-0/demo-0.1.0");
        let mut vars = vec![
            ("CARGO_PKG_NAME".into(), "demo".into()),
            ("CARGO_PKG_VERSION".into(), "0.1.0".into()),
            ("CARGO_MANIFEST_DIR".into(), manifest.into_os_string()),
            ("HOME".into(), root.into()),
            ("PATH".into(), "/bin".into()),
            ("VERUS_Z3_PATH".into(), root.join("z3").into_os_string()),
            ("RUSTC_BOOTSTRAP".into(), "1".into()),
        ];
        for (k, v) in extra {
            vars.push(((*k).into(), (*v).into()));
        }
        vars
    }

    fn args(root: &Path) -> Vec<OsString> {
        let target = root.join("target/debug/deps");
        let source = root.join("home/registry/src/index.crates.io-0/demo-0.1.0/src/lib.rs");
        vec![
            root.join("toolchains/bin/rustc").into_os_string(),
            "--crate-name".into(),
            "demo".into(),
            source.into_os_string(),
            "--json=diagnostic-rendered-ansi,artifacts".into(),
            "--color=always".into(),
            "-C".into(),
            "extra-filename=-abc".into(),
            "--out-dir".into(),
            target.clone().into_os_string(),
            "-L".into(),
            format!("dependency={}", target.display()).into(),
        ]
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "One owned filesystem pair exercises the same stored compiler certificate through relocation and every refusal."
    )]
    fn portable_observed_environment_binds_identity_bytes_and_unknown_reads() -> TestResult {
        fn make_call(machine: &Machine, values: BTreeMap<String, String>) -> Option<Call<'_>> {
            Call::new(
                &machine.plan,
                &machine.args,
                true,
                values
                    .into_iter()
                    .map(|(name, value)| (name.into(), value.into())),
            )
        }
        let mut a = Machine::new("environment-a", |root| Mode::Record {
            out: root.join("bundle"),
        })?;
        let mut b = Machine::new("environment-b", |root| Mode::Serve {
            bundle: root.join("bundle"),
        })?;
        let policy: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../engine/verus/vstd-boundary-inputs.json"
        )))?;
        for machine in [&mut a, &mut b] {
            let path = machine.root.join("z3");
            std::fs::write(&path, b"actual solver adapter A")?;
            machine.plan.solver_adapter = Some(SolverAdapter {
                path,
                sha256: digest(b"actual solver adapter A"),
            });
            machine.plan.closure = Some(serde_json::from_value(serde_json::json!({
                "policy_sha256": "same-pinned-policy", "registry": machine.plan.registry,
                "packages": [policy["vstd"]],
            }))?);
        }
        let observations = |machine: &Machine| -> Option<BTreeMap<String, String>> {
            let mut values: BTreeMap<_, _> = vars(&machine.root, &[])
                .into_iter()
                .map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
                .collect::<Option<_>>()?;
            let package = package_id(&|name| values.get(name).cloned())?;
            values.insert(
                format!(" __VERUS_DRIVER_ARGS_FOR_{package}"),
                "--is-vstd".into(),
            );
            values.insert(format!("__VERUS_DRIVER_VERIFY_{package}"), "1".into());
            values.insert("UNKNOWN_READ".into(), "literal value".into());
            Some(values)
        };
        let va = observations(&a).ok_or("A variables")?;
        let vb = observations(&b).ok_or("B variables")?;
        let ca = make_call(&a, va.clone()).ok_or("A call")?;
        let cb = make_call(&b, vb.clone()).ok_or("B call")?;
        assert_eq!(
            ca.key, cb.key,
            "relocated paths and identical adapter bytes"
        );
        let checksums =
            policy["packages"][policy["vstd"].as_str().ok_or("vstd")?]["compiler_checksums"]
                .as_object()
                .ok_or("checksums")?;
        let (source, expected) = checksums
            .iter()
            .find(|(name, _)| {
                Path::new(name)
                    .extension()
                    .is_some_and(|extension| extension == "rs")
            })
            .ok_or("source")?;
        let source = a
            .plan
            .registry
            .join(policy["vstd"].as_str().ok_or("vstd")?)
            .join(source);
        let phase = serde_json::json!({
            "sources": {source.to_str().ok_or("source path")?: {
                "checksum":format!("blake3={}", expected["blake3"].as_str().ok_or("checksum")?),
                "size":expected["size"]}},
            "extra_tracked_files":[],
            "tracked_environment":va.iter().filter(|(name, _)| name.as_str() != "HOME" && name.as_str() != "PATH")
                .map(|(name, value)| (name.clone(), Some(value.clone()))).collect::<BTreeMap<_, _>>(),
        });
        let actual: compiler::Document = serde_json::from_value(serde_json::json!({
            "kind":"sail-compiler-read-inputs", "schema":1, "complete":true,
            "pid":123, "binding":digest(format!("{}\0{}", a.plan.run_id, ca.key).as_bytes()),
            "phases":{"verification":phase, "compilation":phase},
        }))?;
        assert!(actual.valid(&ca, &a.plan.run_id));
        let saved = actual.mapped(&ca, false).ok_or("saved observations")?;
        let relocated = saved.mapped(&cb, true).ok_or("relocated observations")?;
        assert!(relocated.valid(&cb, &a.plan.run_id));
        let stored: Unit = serde_json::from_value(serde_json::json!({
            "schema":SCHEMA, "key":ca.key, "crate_name":"demo", "package":"demo-0.1.0",
            "verifying":true, "args":[], "environment":{}, "source_tree":{},
            "establishing_run":a.plan.run_id, "compiler_reads":saved,
            "files":[], "dep_info":"demo.d", "sources":[], "declared":[],
            "stdout":{"name":"stdout", "sha256":digest(b""), "size":0, "executable":false},
            "stderr":{"name":"stderr", "sha256":digest(b""), "size":0, "executable":false},
        }))?;
        assert!(compiler::stored(&cb, &stored));
        for name in [
            "VERUS_UNKNOWN_PATH",
            "RUST_UNKNOWN_PATH",
            "Z3_UNKNOWN_PATH",
            "VARGO_UNKNOWN_PATH",
            "CARGO_PKG_UNKNOWN_PATH",
        ] {
            let mut a_values = va.clone();
            let mut b_values = vb.clone();
            a_values.insert(
                name.into(),
                a.plan
                    .registry
                    .join("opaque")
                    .to_string_lossy()
                    .into_owned(),
            );
            b_values.insert(
                name.into(),
                b.plan
                    .registry
                    .join("opaque")
                    .to_string_lossy()
                    .into_owned(),
            );
            let a_call = make_call(&a, a_values).ok_or("unknown A call")?;
            let b_call = make_call(&b, b_values).ok_or("unknown B call")?;
            assert_eq!(a_call.key, b_call.key, "same older key view for {name}");
            let mut certificate = serde_json::to_value(&actual)?;
            certificate["binding"] = serde_json::json!(digest(
                format!("{}\0{}", a.plan.run_id, a_call.key).as_bytes()
            ));
            for phase in ["verification", "compilation"] {
                certificate["phases"][phase]["tracked_environment"][name] =
                    serde_json::json!(a.plan.registry.join("opaque"));
            }
            let certificate: compiler::Document = serde_json::from_value(certificate)?;
            assert!(certificate.valid(&a_call, &a.plan.run_id));
            let saved = certificate.mapped(&a_call, false).ok_or("unknown saved")?;
            let mut entry = serde_json::to_value(&stored)?;
            entry["key"] = serde_json::json!(a_call.key);
            entry["compiler_reads"] = serde_json::to_value(saved)?;
            let entry: Unit = serde_json::from_value(entry)?;
            assert!(
                !compiler::stored(&b_call, &entry),
                "changed unknown {name} must refuse"
            );
        }

        let mut accepted_collisions = Vec::new();
        for name in ["CARGO_RUSTC_CURRENT_DIR", "OUT_DIR"] {
            let mut a_values = va.clone();
            let mut b_values = vb.clone();
            a_values.insert(name.into(), "@@sail-target@@/opaque".into());
            b_values.insert(
                name.into(),
                b.plan.target.join("opaque").to_string_lossy().into_owned(),
            );
            let a_call = make_call(&a, a_values).ok_or("token A call")?;
            let b_call = make_call(&b, b_values).ok_or("physical B call")?;
            assert_eq!(a_call.key, b_call.key, "same lookup key for {name}");
            let mut certificate = serde_json::to_value(&actual)?;
            certificate["binding"] = serde_json::json!(digest(
                format!("{}\0{}", a.plan.run_id, a_call.key).as_bytes()
            ));
            for phase in ["verification", "compilation"] {
                certificate["phases"][phase]["tracked_environment"][name] =
                    serde_json::json!("@@sail-target@@/opaque");
            }
            let certificate: compiler::Document = serde_json::from_value(certificate)?;
            assert!(certificate.valid(&a_call, &a.plan.run_id));
            if let Some(encoded) = certificate.mapped(&a_call, false) {
                let mut entry = serde_json::to_value(&stored)?;
                entry["key"] = serde_json::json!(a_call.key);
                entry["compiler_reads"] = serde_json::to_value(encoded)?;
                let entry: Unit = serde_json::from_value(entry)?;
                if compiler::stored(&b_call, &entry) {
                    accepted_collisions.push(name);
                }
            }
        }
        assert!(
            accepted_collisions.is_empty(),
            "reserved observed VALUE accepted as changed physical path: {accepted_collisions:?}"
        );

        for name in [
            "CARGO_MANIFEST_DIR",
            "CARGO_MANIFEST_PATH",
            "CARGO_RUSTC_CURRENT_DIR",
            "CARGO_TARGET_TMPDIR",
            "OUT_DIR",
        ] {
            for token in [TARGET, REGISTRY, PACKAGE] {
                let value = format!("{token}/opaque");
                let fields = BTreeMap::from([(name.into(), Some(value.clone()))]);
                assert!(
                    ca.mapped_environment(&fields, false).is_none(),
                    "raw token in {name}"
                );
                let line = format!("# env-dep:{name}={value}\n");
                assert!(
                    ca.mapped_dep_info(&line, false).is_none(),
                    "raw dep-info token in {name}"
                );
            }
        }
        for name in [
            "UNKNOWN_READ",
            "VERUS_UNKNOWN_PATH",
            "RUST_UNKNOWN_PATH",
            "Z3_UNKNOWN_PATH",
            "VARGO_UNKNOWN_PATH",
            "CARGO_PKG_UNKNOWN_PATH",
        ] {
            let value = format!("{TARGET}/{REGISTRY}/{PACKAGE}/{SOLVER_ADAPTER}");
            let fields = BTreeMap::from([(name.into(), Some(value.clone()))]);
            assert_eq!(ca.mapped_environment(&fields, false), Some(fields));
            let line = format!("# env-dep:{name}={value}\n");
            assert_eq!(ca.mapped_dep_info(&line, false), Some(line));
        }

        let mut foreign_source = serde_json::to_value(&saved)?;
        foreign_source["phases"]["verification"]["sources"]["@@sail-registry@@/foreign/lib.rs"] =
            serde_json::json!({"checksum":"blake3=foreign", "size":1});
        let foreign_source: compiler::Document = serde_json::from_value(foreign_source)?;
        assert!(
            !foreign_source
                .mapped(&cb, true)
                .ok_or("foreign source")?
                .valid(&cb, &a.plan.run_id)
        );
        let mut foreign_environment = serde_json::to_value(&saved)?;
        foreign_environment["phases"]["verification"]["tracked_environment"]["__VERUS_DRIVER_VERIFY_foreign-0.1.0-000000000000"] =
            serde_json::json!("1");
        let foreign_environment: compiler::Document = serde_json::from_value(foreign_environment)?;
        assert!(
            !foreign_environment
                .mapped(&cb, true)
                .ok_or("foreign environment")?
                .valid(&cb, &a.plan.run_id)
        );
        let mut changed = vb.clone();
        changed.insert("UNKNOWN_READ".into(), "changed".into());
        assert!(!relocated.valid(
            &make_call(&b, changed).ok_or("changed call")?,
            &a.plan.run_id
        ));
        let mut absent = vb.clone();
        absent.remove("UNKNOWN_READ");
        assert!(!relocated.valid(&make_call(&b, absent).ok_or("absent call")?, &a.plan.run_id));
        let mut version = vb.clone();
        version.insert("CARGO_PKG_VERSION".into(), "0.2.0".into());
        let other = make_call(&b, version).ok_or("other version")?;
        assert_ne!(ca.key, other.key);
        assert!(
            !saved
                .mapped(&other, true)
                .ok_or("other observations")?
                .valid(&other, &a.plan.run_id)
        );
        let mut fields = BTreeMap::from([
            (
                format!(
                    "__VERUS_DRIVER_VERIFY_{}",
                    package_id(&|name| va.get(name).cloned()).ok_or("package")?
                ),
                Some("1".into()),
            ),
            (format!("__VERUS_DRIVER_VERIFY_{PACKAGE}"), Some("1".into())),
        ]);
        assert!(
            ca.mapped_environment(&fields, false).is_none(),
            "normalization collision"
        );
        fields.remove(&format!(
            "__VERUS_DRIVER_VERIFY_{}",
            package_id(&|name| va.get(name).cloned()).ok_or("package")?
        ));
        fields.insert(
            format!(
                "__VERUS_DRIVER_VERIFY_{}",
                package_id(&|name| vb.get(name).cloned()).ok_or("package")?
            ),
            Some("1".into()),
        );
        assert!(
            cb.mapped_environment(&fields, true).is_none(),
            "localization collision"
        );
        let foreign = "__VERUS_DRIVER_VERIFY_demo-0.1.0-000000000000";
        assert_eq!(ca.environment_name(foreign, false), foreign);
        assert_eq!(cb.environment_name(foreign, true), foreign);
        let unknown_name = format!(
            "UNKNOWN_{}",
            package_id(&|name| va.get(name).cloned()).ok_or("package")?
        );
        assert_eq!(ca.environment_name(&unknown_name, false), unknown_name);
        let unknown = BTreeMap::from([(
            "UNKNOWN_PATH".into(),
            Some(a.root.join("z3").to_string_lossy().into_owned()),
        )]);
        assert_eq!(ca.mapped_environment(&unknown, false), Some(unknown));
        let mut foreign_path = vb.clone();
        foreign_path.insert(
            "VERUS_Z3_PATH".into(),
            a.root.join("z3").to_string_lossy().into_owned(),
        );
        assert!(
            make_call(&b, foreign_path).is_none(),
            "same bytes at an unconfigured path refuse"
        );
        let path = b.root.join("z3");
        let modified = std::fs::metadata(&path)?.modified()?;
        std::fs::write(&path, b"actual solver adapter B")?;
        std::fs::File::open(&path)?.set_times(std::fs::FileTimes::new().set_modified(modified))?;
        assert_eq!(std::fs::metadata(&path)?.modified()?, modified);
        assert!(
            !cb.adapter_intact(),
            "same-size restored-mtime changed adapter refuses"
        );
        assert!(make_call(&b, vb.clone()).is_none());
        assert!(!compiler::stored(&cb, &stored));
        drop(cb);
        b.plan.solver_adapter.as_mut().ok_or("adapter")?.sha256 =
            digest(b"actual solver adapter B");
        assert_ne!(
            ca.key,
            make_call(&b, vb.clone()).ok_or("new adapter call")?.key
        );
        let mut unbound = b.plan.clone();
        unbound.solver_adapter = None;
        let call_with = |plan: &Plan| {
            Call::new(
                plan,
                &b.args,
                true,
                vb.iter().map(|(name, value)| (name.into(), value.into())),
            )
            .map(|call| call.key)
        };
        assert!(
            call_with(&unbound).is_none(),
            "portable missing binding refuses"
        );
        unbound.mode = Mode::MachineLocal {
            out: b.root.join("local"),
        };
        let local_call = Call::new(
            &unbound,
            &b.args,
            true,
            vb.iter().map(|(name, value)| (name.into(), value.into())),
        )
        .ok_or("local observation call")?;
        let literal = BTreeMap::from([("OUT_DIR".into(), Some(format!("{TARGET}/opaque")))]);
        assert_eq!(
            local_call.mapped_environment(&literal, false),
            Some(literal)
        );
        let line = format!("# env-dep:OUT_DIR={TARGET}/opaque\n");
        assert_eq!(local_call.mapped_dep_info(&line, false), Some(line));
        let local_key = call_with(&unbound).ok_or("local call")?;
        std::fs::write(b.root.join("z3"), b"local adapter is context-owned")?;
        assert_eq!(
            call_with(&unbound),
            Some(local_key),
            "local key has no portable adapter binding"
        );
        Ok(())
    }

    #[test]
    fn keys_ignore_this_machines_directories_and_unbound_variables() {
        let a = Path::new("/machine-a");
        let b = Path::new("/elsewhere/machine-b");
        let (pa, pb) = (
            plan(a, Mode::Record { out: a.into() }),
            plan(b, Mode::Record { out: b.into() }),
        );
        let (args_a, args_b) = (args(a), args(b));
        let ka = Call::new(&pa, &args_a, false, vars(a, &[]).into_iter()).map(|c| c.key);
        let kb = Call::new(
            &pb,
            &args_b,
            false,
            vars(
                b,
                &[
                    ("TERM", "xterm"),
                    ("RUSTUP_TOOLCHAIN", "stable-x86_64-unknown-linux-gnu"),
                    ("RUSTUP_HOME", "/elsewhere/.rustup"),
                ],
            )
            .into_iter(),
        )
        .map(|c| c.key);
        assert!(ka.is_some());
        assert_eq!(ka, kb);
        // A bound variable, verification and a non-rendering argument change it.
        let kc = Call::new(
            &pb,
            &args_b,
            false,
            vars(b, &[("RUSTFLAGS", "-Dwarnings")]).into_iter(),
        )
        .map(|c| c.key);
        assert_ne!(ka, kc);
        for name in ["SDKROOT", "MACOSX_DEPLOYMENT_TARGET"] {
            let changed = Call::new(
                &pb,
                &args_b,
                false,
                vars(b, &[(name, "invalid")]).into_iter(),
            )
            .map(|c| c.key);
            assert_ne!(ka, changed, "{name}");
        }
        let kd = Call::new(&pb, &args_b, true, vars(b, &[]).into_iter()).map(|c| c.key);
        assert_ne!(ka, kd);
        // Capped lint flags and rendering options remain bound: malformed
        // options still diagnose, and --force-warn bypasses the cap.
        let capped = |extra: &[&str]| {
            let mut args = args_b.clone();
            args.extend(["--cap-lints", "allow"].map(OsString::from));
            args.extend(extra.iter().map(OsString::from));
            Call::new(&pb, &args, false, vars(b, &[]).into_iter()).map(|c| c.key)
        };
        assert_ne!(
            capped(&[]),
            capped(&[
                "--allow=clippy::zero_ptr",
                "--check-cfg",
                "cfg(test)",
                "-W",
                "unused"
            ])
        );
        // Forced warnings bypass --cap-lints: serving the quiet build would
        // hide diagnostics from a compiler invocation that requests them.
        assert_ne!(capped(&[]), capped(&["--force-warn=unused"]));
        assert_ne!(capped(&[]), capped(&["--force-warn", "unused"]));
        for extra in [
            vec!["--color=invalid"],
            vec!["--diagnostic-width=invalid"],
            vec!["--check-cfg=oops"],
            vec!["--check-cfg", "oops"],
            vec!["--json=diagnostic-rendered-ansi,artifacts"],
        ] {
            assert_ne!(capped(&[]), capped(&extra));
        }
        let mut linted = args_b.clone();
        linted.push("--allow=clippy::zero_ptr".into());
        let ke = Call::new(&pb, &linted, false, vars(b, &[]).into_iter()).map(|c| c.key);
        assert_ne!(ka, ke);
        let mut more = args_b.clone();
        more.push("--cfg".into());
        more.push("feature=\"std\"".into());
        let ke = Call::new(&pb, &more, false, vars(b, &[]).into_iter()).map(|c| c.key);
        assert_ne!(ka, ke);
        // Another package's driver variables are not this invocation's.
        let kf = Call::new(
            &pb,
            &args_b,
            false,
            vars(
                b,
                &[("__VERUS_DRIVER_VERIFY_other-0.1.0-000000000000", "1")],
            )
            .into_iter(),
        )
        .map(|c| c.key);
        assert_eq!(ka, kf);
    }

    #[test]
    fn workspace_packages_are_never_keyed() {
        let root = Path::new("/machine");
        let plan = plan(root, Mode::Record { out: root.into() });
        let args = args(root);
        let mut vars = vars(root, &[]);
        vars.retain(|(k, _)| k != "CARGO_MANIFEST_DIR");
        vars.push(("CARGO_MANIFEST_DIR".into(), "/machine/project".into()));
        assert!(Call::new(&plan, &args, false, vars.into_iter()).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_environment_is_a_miss() {
        use std::os::unix::ffi::OsStringExt as _;
        let root = Path::new("/machine");
        let plan = plan(root, Mode::Record { out: root.into() });
        let args = args(root);
        let mut vars = vars(root, &[]);
        vars.push(("SDKROOT".into(), OsString::from_vec(vec![0xff])));
        assert!(Call::new(&plan, &args, false, vars.into_iter()).is_none());
    }

    #[test]
    fn paths_are_replaced_only_as_whole_prefixes() {
        assert_eq!(
            replace_path("/a/t/x /a/tt /a/t", "/a/t", "T"),
            "T/x /a/tt T"
        );
        assert_eq!(replace_path("/x/a/t/y", "/a/t", "T"), "/x/a/t/y");
        assert_eq!(
            replace_path("dependency=/a/t/deps", "/a/t", "T"),
            "dependency=T/deps"
        );
        assert_eq!(replace_path("/a/t:", "/a/t", "T"), "T:");
    }

    #[test]
    fn dep_info_lists_sources_and_declared_variables() {
        let text = "/t/libdemo.rmeta: /r/demo/src/lib.rs /r/demo/src/a\\ b.rs\n\n/r/demo/src/lib.rs:\n/r/demo/src/a\\ b.rs:\n\n# env-dep:CARGO_PKG_NAME=demo\n# env-dep:ABSENT\n# checksum:blake3=00 file_len:1 /r/demo/src/lib.rs\n";
        assert_eq!(
            dep_info_sources(text),
            ["/r/demo/src/lib.rs", "/r/demo/src/a b.rs"]
        );
        assert_eq!(
            dep_info_environment(text),
            [
                ("CARGO_PKG_NAME".to_owned(), Some("demo".to_owned())),
                ("ABSENT".to_owned(), None)
            ]
        );
    }

    #[test]
    fn only_artifact_notifications_are_quiet() {
        assert!(quiet(
            b"{\"$message_type\":\"artifact\",\"artifact\":\"/t/a.rmeta\",\"emit\":\"metadata\"}\n"
        ));
        assert!(quiet(b""));
        assert!(!quiet(
            b"{\"$message_type\":\"diagnostic\",\"message\":\"x\"}\n"
        ));
        assert!(!quiet(b"warning: something\n"));
    }

    #[test]
    fn the_shipped_resolution_is_renamed_for_a_new_project() {
        let lock = "# This file is automatically @generated by Cargo.\nversion = 4\n\n[[package]]\nname = \"proc-macro2\"\nversion = \"1.0.0\"\n\n[[package]]\nname = \"sail-shipped-dependencies\"\nversion = \"0.1.0\"\ndependencies = [\n \"vstd\",\n]\n\n[[package]]\nname = \"vstd\"\nversion = \"0.0.1\"\n";
        let renamed = rename_root(lock, RECORD_PACKAGE, "zeta");
        assert_eq!(
            renamed.as_deref(),
            Some(
                "# This file is automatically @generated by Cargo.\nversion = 4\n\n[[package]]\nname = \"proc-macro2\"\nversion = \"1.0.0\"\n\n[[package]]\nname = \"vstd\"\nversion = \"0.0.1\"\n\n[[package]]\nname = \"zeta\"\nversion = \"0.1.0\"\ndependencies = [\n \"vstd\",\n]\n"
            )
        );
        assert!(rename_root(lock, RECORD_PACKAGE, "vstd").is_none());
        assert!(rename_root(lock, "absent", "zeta").is_none());
    }

    #[test]
    fn unusual_directories_disable_shipping() {
        assert!(plain_directory(Path::new(
            "/home/u/.cargo/registry/src/index"
        )));
        for bad in [
            "relative",
            "/with space",
            "/a:b",
            "/x/verus_builtin/y",
            "/a\\b",
        ] {
            assert!(!plain_directory(Path::new(bad)), "{bad}");
        }
    }

    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn compiler_source_binding_is_portable_but_checks_every_fresh_byte() -> TestResult {
        let mut a = Machine::new("compiler-source-a", |root| Mode::Record {
            out: root.join("bundle"),
        })?;
        let mut b = Machine::new("compiler-source-b", |root| Mode::Serve {
            bundle: root.join("bundle"),
        })?;
        for machine in [&mut a, &mut b] {
            machine.plan.virtual_base = format!("/rustc-dev/{}", "1".repeat(40));
            let root = machine
                .root
                .join("sysroot/lib/rustlib/src/rust/library/core/src");
            std::fs::create_dir_all(&root)?;
            std::fs::write(root.join("lib.rs"), b"original compiler source")?;
            let sysroot = machine.root.join("sysroot").canonicalize()?;
            machine.plan.compiler_source = CompilerSource::capture(
                &sysroot,
                &machine.plan.virtual_base,
                &machine.plan.registry,
                &machine.plan.target,
            );
            assert!(machine.plan.compiler_source.is_some());
        }
        assert_eq!(
            a.plan.compiler_source.as_ref().map(CompilerSource::keyed),
            b.plan.compiler_source.as_ref().map(CompilerSource::keyed)
        );
        // Recording names the standard library by its virtual root, never by
        // the recording host's rust-src path (VI-0031); serving adds nothing.
        let recorded = a.call(false).ok_or("A call")?.record_args();
        let source = a.plan.compiler_source.as_ref().ok_or("A compiler source")?;
        let mut std_mapping = source.root.clone().into_os_string();
        std_mapping.push(format!("={}", "/rustc/".to_owned() + &"1".repeat(40)));
        assert_eq!(
            recorded.windows(2).last(),
            Some([OsString::from("--remap-path-prefix"), std_mapping].as_slice())
        );
        assert_eq!(
            b.call(false).ok_or("B call")?.record_args(),
            Vec::<OsString>::new(),
            "serving adds no remapping"
        );
        let original = a.call(false).ok_or("A call")?.key;
        assert_eq!(original, b.call(false).ok_or("B call")?.key);
        let source = b.plan.compiler_source.as_ref().ok_or("B compiler source")?;
        let sysroot = b.root.join("sysroot").canonicalize()?;
        assert!(source.current_at(&sysroot, &b.plan).is_some());
        assert!(
            source
                .current_at(&a.root.join("sysroot").canonicalize()?, &b.plan)
                .is_none()
        );
        let file = source.root.join("library/core/src/lib.rs");
        let modified = std::fs::metadata(&file)?.modified()?;
        std::fs::write(&file, b"modified compiler source")?;
        std::fs::File::open(&file)?.set_times(std::fs::FileTimes::new().set_modified(modified))?;
        assert!(source.current_at(&sysroot, &b.plan).is_none());
        b.plan.compiler_source = CompilerSource::capture(
            &sysroot,
            &b.plan.virtual_base,
            &b.plan.registry,
            &b.plan.target,
        );
        assert_ne!(original, b.call(false).ok_or("changed B call")?.key);
        let local = Machine::new("compiler-source-local", |root| Mode::MachineLocal {
            out: root.join("local"),
        })?;
        assert!(local.plan.compiler_source.is_none());
        Ok(())
    }

    #[test]
    fn compiler_source_binding_refuses_aliases_collisions_and_unknown_trees() -> TestResult {
        let machine = Machine::new("compiler-source-refusals", |root| Mode::Record {
            out: root.join("bundle"),
        })?;
        let sysroot = machine.root.canonicalize()?.join("sysroot");
        let source = sysroot.join("lib/rustlib/src/rust");
        std::fs::create_dir_all(&source)?;
        let virtual_base = format!("/rustc-dev/{}", "1".repeat(40));
        let capture = |base: &str, registry: &Path| {
            CompilerSource::capture(&sysroot, base, registry, &machine.plan.target)
        };
        assert!(
            capture(&virtual_base, &machine.plan.registry).is_none(),
            "empty tree"
        );
        std::fs::write(source.join("source.rs"), b"compiler source")?;
        assert!(capture(&virtual_base, &machine.plan.registry).is_some());
        assert!(capture("/foreign/commit", &machine.plan.registry).is_none());
        assert!(capture("/rustc-dev/short", &machine.plan.registry).is_none());
        assert!(capture(&virtual_base, &source).is_none());
        assert!(capture(&virtual_base, &source.join("nested-registry")).is_none());
        assert!(
            CompilerSource::capture(&sysroot, &virtual_base, &machine.plan.registry, &source)
                .is_none()
        );
        #[cfg(unix)]
        {
            let alias = machine.root.join("sysroot-alias");
            std::os::unix::fs::symlink(&sysroot, &alias)?;
            assert!(
                CompilerSource::capture(
                    &alias,
                    &virtual_base,
                    &machine.plan.registry,
                    &machine.plan.target
                )
                .is_none()
            );
            std::os::unix::fs::symlink("source.rs", source.join("alias.rs"))?;
            assert!(capture(&virtual_base, &machine.plan.registry).is_none());
            std::fs::remove_file(source.join("alias.rs"))?;
        }
        std::fs::remove_file(source.join("source.rs"))?;
        std::fs::create_dir(source.join("unreadable-file-shape"))?;
        assert!(capture(&virtual_base, &machine.plan.registry).is_none());
        Ok(())
    }

    #[test]
    fn installed_compiler_and_sysroot_bytes_are_portable_but_never_omitted() -> TestResult {
        let root =
            std::env::temp_dir().join(format!("sail-shipped-sysroot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (a, b) = (root.join("machine-a"), root.join("machine-b"));
        let inputs = [
            "bin/rustc",
            "bin/cargo",
            "lib/librustc_driver.so",
            "lib/rustlib/host/lib/libstd.rlib",
            "lib/rustlib/host/lib/libproc_macro.rlib",
            "lib/rustlib/host/lib/self-contained/crt.o",
        ];
        for machine in [&a, &b] {
            for path in inputs {
                let path = machine.join(path);
                std::fs::create_dir_all(path.parent().ok_or("no parent")?)?;
                std::fs::write(path, b"matching toolchain bytes")?;
            }
        }
        let original = sysroot_identity(&a).ok_or("no identity")?;
        assert_eq!(Some(original.clone()), sysroot_identity(&b));
        for input in inputs {
            let path = b.join(input);
            std::fs::write(&path, b"another compiler input with the same version")?;
            assert_ne!(Some(original.clone()), sysroot_identity(&b), "{input}");
            std::fs::remove_file(&path)?;
            assert_ne!(Some(original.clone()), sysroot_identity(&b), "{input}");
            std::fs::create_dir(&path)?;
            assert_ne!(Some(original.clone()), sysroot_identity(&b), "{input}");
            std::fs::remove_dir(&path)?;
            std::fs::write(path, b"matching toolchain bytes")?;
        }
        #[cfg(unix)]
        {
            let path = b.join("lib/rustlib/host/lib/dangling.rlib");
            std::os::unix::fs::symlink("absent", path)?;
            assert!(sysroot_identity(&b).is_none());
        }
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    /// One machine: a registry with the `demo` package, a target with one
    /// dependency rlib, and the compiler invocation Cargo would make.
    struct Machine {
        root: PathBuf,
        plan: Plan,
        args: Vec<OsString>,
    }

    impl Machine {
        fn new(
            name: &str,
            mode: impl FnOnce(&Path) -> Mode,
        ) -> std::result::Result<Self, Box<dyn std::error::Error>> {
            let root =
                std::env::temp_dir().join(format!("sail-shipped-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root)?;
            let root = root.canonicalize()?;
            let plan = plan(&root, mode(&root));
            let package = plan.registry.join("demo-0.1.0");
            std::fs::create_dir_all(package.join("src"))?;
            std::fs::write(package.join("Cargo.toml"), "[package]\nname = \"demo\"\n")?;
            std::fs::write(package.join("src/lib.rs"), "pub fn f() {}\n")?;
            let deps = plan.target.join("debug/deps");
            std::fs::create_dir_all(&deps)?;
            std::fs::write(deps.join("libdep-1.rlib"), b"dependency bytes")?;
            let mut args = args(&root);
            args.push("--extern".into());
            args.push(format!("dep={}", deps.join("libdep-1.rlib").display()).into());
            Ok(Self { root, plan, args })
        }

        fn call(&self, verifying: bool) -> Option<Call<'_>> {
            Call::new(
                &self.plan,
                &self.args,
                verifying,
                vars(&self.root, &[]).into_iter(),
            )
        }

        /// What rustc writes for the invocation.
        fn compile(&self) -> std::io::Result<()> {
            let deps = self.plan.target.join("debug/deps");
            let source = self.plan.registry.join("demo-0.1.0/src/lib.rs");
            std::fs::write(
                deps.join("libdemo-abc.rmeta"),
                b"metadata naming /rustc-dev/0123",
            )?;
            std::fs::write(deps.join("libdemo-abc.rlib"), b"library")?;
            std::fs::write(
                deps.join("demo-abc.d"),
                format!(
                    "{}/libdemo-abc.rmeta: {}\n\n{}:\n\n# env-dep:CARGO_PKG_NAME=demo\n",
                    deps.display(),
                    source.display(),
                    source.display()
                ),
            )
        }

        fn stderr(&self) -> String {
            format!(
                "{{\"$message_type\":\"artifact\",\"artifact\":\"{}/debug/deps/libdemo-abc.rmeta\",\"emit\":\"metadata\"}}\n",
                self.plan.target.display()
            )
        }

        fn out(&self) -> Option<PathBuf> {
            match &self.plan.mode {
                Mode::Record { out } | Mode::MachineLocal { out } => Some(out.clone()),
                Mode::Serve { .. } => None,
            }
        }
    }

    #[test]
    fn release_inventory_metadata_is_not_a_proof_key_input()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut machine = Machine::new("release-admission-key", |root| Mode::Serve {
            bundle: root.join("bundle"),
        })?;
        let original = machine.call(true).ok_or("original key")?.key;
        machine.plan.release = Some(serde_json::from_value(serde_json::json!({
            "index_sha256":"installation-only", "members":{}
        }))?);
        assert_eq!(machine.call(true).ok_or("admitted key")?.key, original);
        // The optional field is absent from earlier research per-run plans.
        machine.plan.release = None;
        assert!(
            serde_json::to_value(&machine.plan)?
                .get("release")
                .is_none()
        );
        std::fs::remove_dir_all(&machine.root)?;
        Ok(())
    }

    #[test]
    fn boundary_accepts_only_unselected_verified_vstd_and_fresh_dependency_bytes()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut machine = Machine::new("vstd-boundary", |root| Mode::Record {
            out: root.join("bundle"),
        })?;
        let ordinary_vars = vars(&machine.root, &[]);
        assert!(
            Call::vstd_invocation(&machine.plan, &machine.args, ordinary_vars.into_iter())
                .is_none()
        );
        let version = crate::toolchain::Pins::embedded()?
            .verus
            .vstd
            .ok_or("pinned vstd")?;
        let package = machine.plan.registry.join(format!("vstd-{version}"));
        std::fs::create_dir_all(package.join("src"))?;
        let manifest = package.join("Cargo.toml");
        std::fs::write(
            &manifest,
            format!(
                "[package]\nname='vstd'\nversion='{version}'\n[package.metadata.verus]\nverify=true\nis-vstd=true\n"
            ),
        )?;
        machine.args[2] = "vstd".into();
        machine.args[3] = package.join("src/lib.rs").into_os_string();
        machine.args.extend(["--crate-type".into(), "lib".into()]);
        let invocation_vars = vec![
            (OsString::from("CARGO_PKG_NAME"), OsString::from("vstd")),
            ("CARGO_PKG_VERSION".into(), version.clone().into()),
            ("CARGO_MANIFEST_DIR".into(), package.into_os_string()),
        ];
        let key = || {
            Call::vstd_invocation(
                &machine.plan,
                &machine.args,
                invocation_vars.clone().into_iter(),
            )
            .map(|call| call.key)
        };
        let original = key().ok_or("vstd boundary refused")?;
        let dependency = machine.plan.target.join("debug/deps/libdep-1.rlib");
        std::fs::write(&dependency, b"genuinely rebuilt native dependency")?;
        assert_ne!(
            key().ok_or("changed dependency was not readable")?,
            original
        );
        std::fs::remove_file(&dependency)?;
        assert!(
            key().is_none(),
            "missing native prerequisites cannot be served"
        );
        std::fs::write(&dependency, b"dependency bytes")?;
        assert_eq!(key(), Some(original));
        let mut sibling = invocation_vars.clone();
        sibling.retain(|(name, _)| name != "CARGO_MANIFEST_DIR");
        sibling.push((
            "CARGO_MANIFEST_DIR".into(),
            machine.plan.registry.join("foreign-vstd").into_os_string(),
        ));
        assert!(Call::vstd_invocation(&machine.plan, &machine.args, sibling.into_iter()).is_none());
        for (name, value) in [
            ("CARGO_PRIMARY_PACKAGE", "1"),
            ("CARGO_PKG_VERSION", "other"),
            ("CARGO_PKG_NAME", "build_script_build"),
        ] {
            let mut changed = invocation_vars.clone();
            changed.retain(|(key, _)| key != name);
            changed.push((name.into(), value.into()));
            assert!(
                Call::vstd_invocation(&machine.plan, &machine.args, changed.into_iter()).is_none(),
                "{name}"
            );
        }
        std::fs::write(&manifest, "[package]\nname='vstd'\n")?;
        assert!(
            key().is_none(),
            "the package name alone grants no proof reuse"
        );
        Ok(())
    }

    #[test]
    fn boundary_key_binds_actual_solver_identity() {
        let root = Path::new("/machine");
        let mut plan = plan(root, Mode::Record { out: root.into() });
        let args = args(root);
        let key = Call::new(&plan, &args, true, vars(root, &[]).into_iter()).map(|call| call.key);
        assert!(key.is_some());
        plan.solver_sha256 = "replacement-solver".into();
        assert_ne!(
            key,
            Call::new(&plan, &args, true, vars(root, &[]).into_iter()).map(|call| call.key)
        );
    }

    #[test]
    fn local_boundary_preserves_registry_bytes_and_actual_proof_origin() -> TestResult {
        let machine = Machine::new("local-boundary", |root| Mode::MachineLocal {
            out: root.join("local-cache"),
        })?;
        let call = machine.call(false).ok_or("no boundary call")?;
        assert!(
            call.record_args().is_empty(),
            "local sources are never remapped"
        );
        machine.compile()?;
        let output = machine.plan.target.join("debug/deps/libdemo-abc.rmeta");
        let registry_bytes = machine.plan.registry.to_string_lossy().as_bytes().to_vec();
        std::fs::write(&output, &registry_bytes)?;
        call.record(b"", machine.stderr().as_bytes(), Some(0));
        for path in call.outputs() {
            std::fs::remove_file(path)?;
        }
        let replay = call.serve().ok_or("no local replay")?;
        assert_eq!(
            replay.origin.kind,
            sail_api::project::DependencyBuildKind::MachineLocal
        );
        assert_eq!(replay.origin.establishing_run, "original-verification");
        assert_eq!(std::fs::read(output)?, registry_bytes);
        let mut changed_plan = machine.plan.clone();
        changed_plan.registry = machine.root.join("another-registry");
        std::fs::create_dir_all(&changed_plan.registry)?;
        let changed_vars = vars(&machine.root, &[]).into_iter().map(|(name, value)| {
            if name == "CARGO_MANIFEST_DIR" {
                (
                    name,
                    changed_plan.registry.join("demo-0.1.0").into_os_string(),
                )
            } else {
                (name, value)
            }
        });
        let changed = Call::new(&changed_plan, &machine.args, false, changed_vars)
            .ok_or("no changed call")?;
        assert_ne!(
            changed.key, call.key,
            "literal registry root is a key input"
        );
        assert!(changed.serve().is_none());
        Ok(())
    }

    #[test]
    fn forwarded_flags_name_the_miss_unless_only_a_raised_rlimit() {
        let args = |items: &[&str]| -> Vec<String> {
            items.iter().map(|item| (*item).to_owned()).collect()
        };
        for none in [
            args(&["-p", "vest_lib"]),
            args(&["--"]),
            args(&["--", "--rlimit", "100"]),
            args(&["--", "--rlimit=10"]),
            args(&["--fwd-verus-args-to", "roots", "--", "--rlimit", "5"]),
            args(&["--fwd-verus-args-to=roots", "--", "--expand-errors"]),
        ] {
            assert_eq!(forwarded_miss(&none), None, "{none:?}");
        }
        let low = forwarded_miss(&args(&["--", "--rlimit", "5"])).unwrap_or_default();
        assert!(low.contains("`--rlimit 5`") && low.contains("below the default rlimit 10"));
        for flags in [
            args(&["--", "--expand-errors"]),
            args(&["--", "--rlimit", "100", "--expand-errors"]),
            args(&["--fwd-verus-args-to", "all", "--", "--rlimit", "inf"]),
        ] {
            let reason = forwarded_miss(&flags).unwrap_or_default();
            assert!(
                reason.contains("the proof binds them"),
                "{flags:?}: {reason}"
            );
            assert!(reason.contains("--fwd-verus-args-to roots"));
        }
    }

    #[test]
    fn rlimit_flag_is_removed_only_when_exactly_one_finite_value_is_present() {
        let pack = |items: &[&str]| -> String {
            items.iter().fold(String::new(), |mut packed, item| {
                packed.push_str(ARGS_SEP);
                packed.push_str(item);
                packed
            })
        };
        let base = pack(&["--is-vstd"]);
        assert_eq!(
            without_rlimit(&pack(&["--is-vstd", "--rlimit", "100"])),
            Some((base.clone(), 100.0))
        );
        assert_eq!(
            without_rlimit(&pack(&["--rlimit=12.5", "--is-vstd"])),
            Some((base.clone(), 12.5))
        );
        for refused in [
            pack(&["--is-vstd"]),
            pack(&["--is-vstd", "--rlimit"]),
            pack(&["--is-vstd", "--rlimit", "100", "--rlimit", "200"]),
            pack(&["--is-vstd", "--rlimit", "inf"]),
            pack(&["--is-vstd", "--rlimit", "1e9"]),
            pack(&["--is-vstd", "--rlimit", "0"]),
            pack(&["--is-vstd", "--rlimit="]),
            pack(&["--rlimit", "100"]),
            String::new(),
        ] {
            assert_eq!(without_rlimit(&refused), None, "{refused}");
        }
    }

    fn witness(key: &str) -> serde_json::Value {
        serde_json::json!({
            "schema": 1, "kind": "sail-rlimit-witness", "key": key,
            "recorded_rlimit": 10.0, "witness_rlimit": 100.0,
            "recorded_solver_limit": 30_000_000_u64, "witness_solver_limit": 300_000_000_u64,
            "max_query": 12_941_218_u64, "max_query_limit": 30_000_000_u64,
            "over_limit_queries": 0, "queries": 4000, "unknown_answers": 0,
            "solver_processes": 30, "inputs_equal": true, "answers_and_counts_equal": true,
            "normalized_inputs_sha256": "a".repeat(64),
        })
    }

    #[test]
    fn rlimit_witness_admits_only_complete_unchanged_proofs_at_a_higher_rlimit() -> TestResult {
        let key = "k".repeat(64);
        let admits = |value: serde_json::Value,
                      requested: f64|
         -> std::result::Result<bool, Box<dyn std::error::Error>> {
            let witness: RlimitWitness = serde_json::from_value(value)?;
            Ok(witness.admits(&key, requested))
        };
        assert!(admits(witness(&key), 100.0)?);
        assert!(admits(witness(&key), 10.0)?);
        assert!(
            !admits(witness(&key), 9.5)?,
            "a lower rlimit is never subsumed"
        );
        for (field, value) in [
            ("key", serde_json::json!("other")),
            ("inputs_equal", serde_json::json!(false)),
            ("answers_and_counts_equal", serde_json::json!(false)),
            ("unknown_answers", serde_json::json!(1)),
            ("max_query", serde_json::json!(30_000_000_u64)),
            ("over_limit_queries", serde_json::json!(1)),
            ("queries", serde_json::json!(0)),
            ("witness_rlimit", serde_json::json!(10.0)),
            ("schema", serde_json::json!(2)),
        ] {
            let mut changed = witness(&key);
            changed[field] = value;
            assert!(!admits(changed, 100.0)?, "{field}");
        }
        Ok(())
    }

    #[test]
    fn raised_rlimit_serves_the_unit_recorded_without_it_only_with_a_witness() -> TestResult {
        let machine = Machine::new("rlimit-subsumption", |root| Mode::MachineLocal {
            out: root.join("local-cache"),
        })?;
        let manifest = machine.plan.registry.join("demo-0.1.0");
        let package = package_id(&|name: &str| match name {
            "CARGO_PKG_NAME" => Some("demo".to_owned()),
            "CARGO_PKG_VERSION" => Some("0.1.0".to_owned()),
            "CARGO_MANIFEST_DIR" => Some(manifest.to_string_lossy().into_owned()),
            _ => None,
        })
        .ok_or("no package id")?;
        let name = format!("{}{package}", VERUS_OWN[0]);
        let recorded_args = format!("{ARGS_SEP}--is-vstd");
        let call = |value: &str| {
            Call::new(
                &machine.plan,
                &machine.args,
                true,
                vars(&machine.root, &[(&name, value)]).into_iter(),
            )
        };
        let recorded = call(&recorded_args).ok_or("no recorded call")?;
        machine.compile()?;
        recorded.record(b"verified\n", machine.stderr().as_bytes(), Some(0));
        let unit = machine
            .out()
            .ok_or("no cache")?
            .join(UNITS)
            .join(&recorded.key);
        assert!(
            unit.join(UNIT).is_file(),
            "the default-rlimit proof is recorded"
        );
        let remove_outputs = || -> TestResult {
            for path in recorded.outputs() {
                let _ = std::fs::remove_file(path);
            }
            Ok(())
        };
        let raised_args = format!("{recorded_args}{ARGS_SEP}--rlimit{ARGS_SEP}100");
        let raised = call(&raised_args).ok_or("no raised call")?;
        assert_ne!(
            raised.key, recorded.key,
            "the exact key still binds --rlimit"
        );
        remove_outputs()?;
        assert!(raised.serve().is_none(), "no witness, no subsumption");
        std::fs::write(
            unit.join(RLIMIT_WITNESS),
            serde_json::to_vec(&witness(&recorded.key))?,
        )?;
        let replay = raised.serve().ok_or("not served under a raised rlimit")?;
        assert_eq!(replay.stdout, b"verified\n");
        let lower = call(&format!("{recorded_args}{ARGS_SEP}--rlimit{ARGS_SEP}5"))
            .ok_or("no lower call")?;
        remove_outputs()?;
        assert!(lower.serve().is_none(), "a lower rlimit is never subsumed");
        let other = call(&format!(
            "{recorded_args}{ARGS_SEP}--rlimit{ARGS_SEP}100{ARGS_SEP}--expand-errors"
        ))
        .ok_or("no other call")?;
        assert!(other.serve().is_none(), "every other flag stays keyed");
        Ok(())
    }

    #[test]
    fn local_boundary_refuses_changed_native_bytes_and_target_paths() -> TestResult {
        let machine = Machine::new("local-inputs", |root| Mode::MachineLocal {
            out: root.join("local-cache"),
        })?;
        let call = machine.call(false).ok_or("no call")?;
        machine.compile()?;
        let output = machine.plan.target.join("debug/deps/libdemo-abc.rmeta");
        std::fs::write(&output, machine.plan.target.to_string_lossy().as_bytes())?;
        call.record(b"", machine.stderr().as_bytes(), Some(0));
        assert!(
            !machine
                .out()
                .ok_or("no cache")?
                .join(UNITS)
                .join(&call.key)
                .exists()
        );
        machine.compile()?;
        call.record(b"", machine.stderr().as_bytes(), Some(0));
        std::fs::write(
            machine.plan.target.join("debug/deps/libdep-1.rlib"),
            b"different native bytes",
        )?;
        let changed = machine.call(false).ok_or("no changed call")?;
        assert_ne!(changed.key, call.key);
        assert!(changed.serve().is_none());
        Ok(())
    }

    #[test]
    fn publication_refuses_changed_sources_or_native_dependencies()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let machine = Machine::new("boundary-publication", |root| Mode::Record {
            out: root.join("bundle"),
        })?;
        let package = machine.plan.registry.join("demo-0.1.0");
        let mut call = machine.call(false).ok_or("no compiler call")?;
        call.source_tree = Some(package_tree(&package).ok_or("no package snapshot")?);
        machine.compile()?;
        let out = machine.out().ok_or("not recording")?;
        let entry = out.join(UNITS).join(&call.key);
        let added = package.join("new_module.rs");
        std::fs::write(&added, "pub fn changed() {}")?;
        call.record(b"", machine.stderr().as_bytes(), Some(0));
        assert!(
            !entry.exists(),
            "a newly added module must refuse publication"
        );
        std::fs::remove_file(added)?;
        let dependency = machine.plan.target.join("debug/deps/libdep-1.rlib");
        std::fs::write(&dependency, b"changed while the verifier ran")?;
        call.record(b"", machine.stderr().as_bytes(), Some(0));
        assert!(
            !entry.exists(),
            "a changed native input must refuse publication"
        );
        std::fs::write(dependency, b"dependency bytes")?;
        call.record(b"", machine.stderr().as_bytes(), Some(0));
        assert!(
            entry.join(UNIT).is_file(),
            "unchanged inputs can be recorded"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn package_snapshot_refuses_redirected_inputs()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let machine = Machine::new("boundary-source-alias", |root| Mode::Record {
            out: root.join("bundle"),
        })?;
        let package = machine.plan.registry.join("demo-0.1.0");
        assert!(package_tree(&package).is_some());
        std::os::unix::fs::symlink(package.join("src/lib.rs"), package.join("alias.rs"))?;
        assert!(package_tree(&package).is_none());
        Ok(())
    }

    impl Drop for Machine {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn a_recorded_invocation_is_served_on_another_machine_with_its_paths() -> TestResult {
        let build = Machine::new("build", |root| Mode::Record {
            out: root.join("bundle"),
        })?;
        let call = build.call(false).ok_or("no call")?;
        build.compile()?;
        call.record(b"", build.stderr().as_bytes(), Some(0));
        let out = build.out().ok_or("mode")?;
        let user = Machine::new("user", |_| Mode::Serve {
            bundle: out.clone(),
        })?;
        let served = user.call(false).ok_or("no call")?;
        assert_eq!(served.key, call.key);
        let replay = served.serve().ok_or("not served")?;
        assert_eq!(String::from_utf8(replay.stderr)?, user.stderr());
        assert_eq!(replay.stdout, Vec::<u8>::new());
        let deps = user.plan.target.join("debug/deps");
        assert_eq!(std::fs::read(deps.join("libdemo-abc.rlib"))?, b"library");
        let dep_info = std::fs::read_to_string(deps.join("demo-abc.d"))?;
        let source = user.plan.registry.join("demo-0.1.0/src/lib.rs");
        assert!(dep_info.contains(&source.display().to_string()));
        assert!(!dep_info.contains(&build.root.display().to_string()));
        Ok(())
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "The same owned stage/restore pair covers exact rustc grammar and all collision/refusal cases."
    )]
    fn portable_dep_info_preserves_environment_lines_and_restores_current_paths() -> TestResult {
        let mut build = Machine::new("dep-info-build", |root| Mode::Record {
            out: root.join("bundle"),
        })?;
        let mut user = Machine::new("dep-info-user", |_| Mode::Serve {
            bundle: build.root.join("bundle"),
        })?;
        for machine in [&mut build, &mut user] {
            let path = machine.root.join("z3");
            std::fs::write(&path, b"same adapter")?;
            machine.plan.solver_adapter = Some(SolverAdapter {
                path,
                sha256: digest(b"same adapter"),
            });
        }
        let a = build.call(false).ok_or("build call")?;
        let b = user.call(false).ok_or("user call")?;
        build.compile()?;
        let source = build.plan.registry.join("demo-0.1.0/src/lib.rs");
        let package = package_id(&|name| a.vars.get(name).cloned()).ok_or("package")?;
        let unknown = format!(
            "# env-dep:VERUS_UNKNOWN_PATH={}\r\n",
            build.plan.registry.join("opaque").display()
        );
        let text = format!(
            "{0}/libdemo-abc.rmeta: {1}\n\n{1}:\n\n# env-dep:VERUS_Z3_PATH={2}\n# env-dep: __VERUS_DRIVER_ARGS_FOR_{package}=compile\\nline\\rreturn\\\\slash=equals\n# env-dep:OUT_DIR={0}\n# env-dep:ABSENT\n# env-dep:EMPTY=\n{unknown}# env-dep:ESCAPED=back\\\\slash\\nnewline\\rcarriage",
            build.plan.target.join("debug/deps").display(),
            source.display(),
            build.root.join("z3").display()
        );
        let stored = a.mapped_dep_info(&text, false).ok_or("map dep-info")?;
        assert!(stored.contains(&unknown), "unknown env line byte-identical");
        let restored = b.mapped_dep_info(&stored, true).ok_or("restore dep-info")?;
        assert!(restored.contains(&format!(
            "# env-dep:VERUS_Z3_PATH={}\n",
            user.root.join("z3").display()
        )));
        assert!(!restored.contains(&format!(
            "VERUS_Z3_PATH={}",
            build.root.join("z3").display()
        )));
        assert!(restored.contains(&unknown));
        assert!(restored.ends_with("# env-dep:ESCAPED=back\\\\slash\\nnewline\\rcarriage"));
        assert!(restored.contains("# env-dep:ABSENT\n# env-dep:EMPTY=\n"));
        assert_eq!(
            b.mapped_dep_info(&b.mapped_dep_info(&stored, true).ok_or("B again")?, false),
            Some(stored.clone())
        );
        for invalid in [
            "# env-dep:=empty\n",
            "# env-dep:BAD=unknown\\q\n",
            "# env-dep:BAD=trailing\\",
            " # env-dep:BAD=value\n",
            "# env-dependency:BAD=value\n",
            "# env-dep:DUP=1\n# env-dep:DUP=1\n",
        ] {
            assert!(
                a.mapped_dep_info(invalid, false).is_none(),
                "unsupported {invalid:?}"
            );
        }
        let collision = format!(
            "# env-dep:__VERUS_DRIVER_VERIFY_{package}=1\n# env-dep:__VERUS_DRIVER_VERIFY_{PACKAGE}=1\n"
        );
        assert!(a.mapped_dep_info(&collision, false).is_none());
        let user_package = package_id(&|name| b.vars.get(name).cloned()).ok_or("user package")?;
        let collision = format!(
            "# env-dep:__VERUS_DRIVER_VERIFY_{PACKAGE}=1\n# env-dep:__VERUS_DRIVER_VERIFY_{user_package}=1\n"
        );
        assert!(b.mapped_dep_info(&collision, true).is_none());
        assert!(
            a.mapped_dep_info(
                &format!("# env-dep:__VERUS_DRIVER_VERIFY_{PACKAGE}=1\n"),
                false
            )
            .is_none()
        );
        let literal_token = "# env-dep:VERUS_UNKNOWN_PATH=@@sail-registry@@/literal\n";
        assert_eq!(
            b.mapped_dep_info(literal_token, true),
            Some(literal_token.into())
        );
        assert_eq!(
            a.mapped_dep_info(literal_token, false),
            Some(literal_token.into())
        );
        for ending in ["", "\n", "\r\n"] {
            let line = format!(
                "# env-dep:VERUS_Z3_PATH={}{}",
                build.root.join("z3").display(),
                ending
            );
            let stored = a.mapped_dep_info(&line, false).ok_or("known line")?;
            assert_eq!(
                b.mapped_dep_info(&stored, true),
                Some(format!(
                    "# env-dep:VERUS_Z3_PATH={}{}",
                    user.root.join("z3").display(),
                    ending
                ))
            );
            let unknown =
                format!("# env-dep:UNKNOWN\\nNAME=back\\\\slash\\nnewline\\rcarriage{ending}");
            assert_eq!(a.mapped_dep_info(&unknown, false), Some(unknown));
        }

        // Exercise the real artifact staging and restore, not just its codec.
        // Unknown comments remain literal above; this artifact uses actual bound observations.
        let literal = format!(
            "# env-dep:VERUS_Z3_PATH={}\n",
            build.root.join("z3").display()
        );
        let mut dep_info =
            std::fs::read_to_string(build.plan.target.join("debug/deps/demo-abc.d"))?;
        dep_info.push_str(&literal);
        std::fs::write(build.plan.target.join("debug/deps/demo-abc.d"), dep_info)?;
        a.record(b"", build.stderr().as_bytes(), Some(0));
        assert!(b.serve().is_some());
        let dep_path = user.plan.target.join("debug/deps/demo-abc.d");
        let restored = std::fs::read_to_string(&dep_path)?;
        assert!(restored.contains(&format!(
            "# env-dep:VERUS_Z3_PATH={}\n",
            user.root.join("z3").display()
        )));
        assert!(!restored.contains(&build.root.to_string_lossy().into_owned()));
        let stamp = std::fs::metadata(&dep_path)?.modified()?;
        assert!(checked_dep_info_environment(&restored).is_some());
        assert_eq!(
            checked_dep_info_environment(&restored).ok_or("environment")?["VERUS_Z3_PATH"].as_ref(),
            b.vars.get("VERUS_Z3_PATH")
        );
        assert_eq!(
            std::fs::metadata(&dep_path)?.modified()?,
            stamp,
            "freshness inspection never rewrites .d"
        );
        // No portable mappings are applied to machine-local env lines.
        user.plan.mode = Mode::MachineLocal {
            out: user.root.join("local"),
        };
        user.plan.solver_adapter = None;
        let local = user.call(false).ok_or("local call")?;
        assert_eq!(
            local.mapped_dep_info(&text, false),
            Some(local.normalizer.normalize(&text))
        );
        Ok(())
    }

    #[test]
    fn attached_extern_arguments_bind_library_and_vir_bytes() -> TestResult {
        let mut machine = Machine::new("attached-extern", |root| Mode::Record {
            out: root.join("bundle"),
        })?;
        let external = machine.args.pop().ok_or("no extern argument")?;
        machine.args.pop().ok_or("no extern flag")?;
        let mut attached = OsString::from("--extern=");
        attached.push(external);
        machine.args.push(attached);
        let original = machine.call(false).ok_or("no call")?.key;
        let dependency = machine.plan.target.join("debug/deps/libdep-1.rlib");
        std::fs::write(&dependency, b"changed library")?;
        assert_ne!(original, machine.call(false).ok_or("no call")?.key);
        std::fs::write(&dependency, b"dependency bytes")?;
        assert_eq!(original, machine.call(false).ok_or("no call")?.key);
        std::fs::write(dependency.with_extension("vir"), b"exported VIR")?;
        let with_vir = machine.call(false).ok_or("no call")?.key;
        assert_ne!(original, with_vir);
        std::fs::write(dependency.with_extension("vir"), b"changed VIR")?;
        assert_ne!(with_vir, machine.call(false).ok_or("no call")?.key);
        std::fs::remove_file(dependency.with_extension("vir"))?;
        std::fs::create_dir(dependency.with_extension("vir"))?;
        assert!(machine.call(false).is_none());
        std::fs::remove_dir(dependency.with_extension("vir"))?;
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("missing-vir", dependency.with_extension("vir"))?;
            assert!(machine.call(false).is_none());
            std::fs::remove_file(dependency.with_extension("vir"))?;
        }
        machine.args.pop().ok_or("no attached argument")?;
        machine.args.push("--extern=unqualified".into());
        assert!(machine.call(false).is_none());
        machine.args.pop().ok_or("no attached argument")?;
        // Cargo's implicit proc_macro is provided by the pinned compiler.
        machine.args.push("--extern=proc_macro".into());
        assert!(machine.call(false).is_some());
        Ok(())
    }

    #[test]
    fn unsnapshotted_schema_entries_never_place_outputs() -> TestResult {
        let build = Machine::new("old-schema-build", |root| Mode::Record {
            out: root.join("bundle"),
        })?;
        let call = build.call(false).ok_or("no call")?;
        build.compile()?;
        call.record(b"", build.stderr().as_bytes(), Some(0));
        let out = build.out().ok_or("mode")?;
        let path = out.join(UNITS).join(&call.key).join(UNIT);
        let mut document: Value = serde_json::from_slice(&std::fs::read(&path)?)?;
        document["schema"] = Value::from(SCHEMA - 1);
        std::fs::write(path, serde_json::to_vec(&document)?)?;
        let user = Machine::new("old-schema-user", |_| Mode::Serve { bundle: out })?;
        assert!(user.call(false).ok_or("no call")?.serve().is_none());
        assert!(
            !user
                .plan
                .target
                .join("debug/deps/libdemo-abc.rlib")
                .exists()
        );
        Ok(())
    }

    #[test]
    fn mismatches_and_damage_miss_with_trace_reasons() -> TestResult {
        let build = Machine::new("build2", |root| Mode::Record {
            out: root.join("bundle"),
        })?;
        let call = build.call(false).ok_or("no call")?;
        build.compile()?;
        call.record(b"", build.stderr().as_bytes(), Some(0));
        let out = build.out().ok_or("mode")?;
        let entry = out.join(UNITS).join(&call.key);
        assert!(entry.join(UNIT).is_file());

        // An edited registry source: a silent miss, and nothing is placed.
        let user = Machine::new("user2", |_| Mode::Serve {
            bundle: out.clone(),
        })?;
        let refusal = |mut call: Call<'_>, expected: &str| -> TestResult {
            let directory = user.root.join("lookup-trace");
            call.vars.insert(
                TRACE_ENV.to_owned(),
                directory.to_str().ok_or("trace path")?.to_owned(),
            );
            assert!(call.serve().is_none());
            let path = directory.join(format!("{}-{}.json", call.stem, &call.key[..12]));
            let trace: Value = serde_json::from_slice(&std::fs::read(path)?)?;
            let outcome = trace["outcome"].as_str().ok_or("missing refusal reason")?;
            assert!(outcome.contains(expected), "{outcome}");
            Ok(())
        };
        let source = user.plan.registry.join("demo-0.1.0/src/lib.rs");
        std::fs::write(&source, "pub fn f() { /* edited */ }\n")?;
        refusal(user.call(false).ok_or("no call")?, "differs")?;
        assert!(
            !user
                .plan
                .target
                .join("debug/deps/libdemo-abc.rlib")
                .exists()
        );
        std::fs::write(&source, "pub fn f() {}\n")?;

        #[cfg(unix)]
        {
            let redirected = user.root.join("redirected.rs");
            std::fs::rename(&source, &redirected)?;
            std::os::unix::fs::symlink(&redirected, &source)?;
            refusal(user.call(false).ok_or("no call")?, "redirected")?;
            std::fs::remove_file(&source)?;
            std::fs::rename(redirected, &source)?;
        }

        let manifest = user.plan.registry.join("demo-0.1.0/Cargo.toml");
        let original_manifest = std::fs::read(&manifest)?;
        std::fs::write(&manifest, "[package]\nname = \"changed\"\n")?;
        refusal(user.call(false).ok_or("no call")?, "differs")?;
        std::fs::write(manifest, original_manifest)?;

        // A dependency compiled here (other bytes) is another key: a miss.
        let dependency = user.plan.target.join("debug/deps/libdep-1.rlib");
        std::fs::write(&dependency, b"local bytes")?;
        let other = user.call(false).ok_or("no call")?;
        assert_ne!(other.key, call.key);
        refusal(other, "no entry")?;
        std::fs::write(&dependency, b"dependency bytes")?;

        // Even a malformed final destination leaves no earlier outputs.
        let original_unit = std::fs::read(entry.join(UNIT))?;
        let mut malformed: Unit = serde_json::from_slice(&original_unit)?;
        malformed.files.last_mut().ok_or("no files")?.name = "../redirected".into();
        std::fs::write(entry.join(UNIT), serde_json::to_vec(&malformed)?)?;
        refusal(user.call(false).ok_or("no call")?, "unsafe file name")?;
        for name in ["libdemo-abc.rlib", "libdemo-abc.rmeta", "demo-abc.d"] {
            assert!(!user.plan.target.join("debug/deps").join(name).exists());
        }
        std::fs::write(entry.join(UNIT), original_unit)?;

        // A damaged stored file records its reason, places nothing and is a miss.
        std::fs::write(entry.join("files/libdemo-abc.rlib"), b"corrupt")?;
        refusal(user.call(false).ok_or("no call")?, "has another digest")?;
        for name in ["libdemo-abc.rlib", "libdemo-abc.rmeta", "demo-abc.d"] {
            assert!(
                !user.plan.target.join("debug/deps").join(name).exists(),
                "{name}"
            );
        }

        // Both unreadable and missing entries remain traceable misses.
        std::fs::write(entry.join(UNIT), b"{")?;
        refusal(user.call(false).ok_or("no call")?, "unreadable entry")?;
        std::fs::remove_dir_all(&entry)?;
        refusal(user.call(false).ok_or("no call")?, "no entry")?;
        Ok(())
    }

    #[test]
    fn record_lock_copy_error_distinguishes_source_and_destination() -> TestResult {
        let machine = Machine::new("record-copy-error", |root| Mode::Record {
            out: root.join("bundle"),
        })?;
        let workspace = machine.root.join("workspace");
        std::fs::create_dir(&workspace)?;
        let source = workspace.join(LOCKFILE);
        std::fs::write(&source, b"unchanged lock")?;
        let out = machine.out().ok_or("not recording")?;
        let destination = out.join(LOCKFILE);
        let error = finish_record(&machine.plan, Path::new("unused"), &workspace)
            .err()
            .ok_or("missing destination parent was accepted")?;
        assert_eq!(error.code, "SAIL3134");
        for expected in [
            source.display().to_string(),
            destination.display().to_string(),
            "source_exists=Ok(true)".to_owned(),
            "destination_parent_exists=Ok(false)".to_owned(),
        ] {
            assert!(error.message.contains(&expected), "{}", error.message);
        }
        assert_eq!(std::fs::read(&source)?, b"unchanged lock");
        assert!(!out.exists(), "diagnostics must not create a recording");
        std::fs::create_dir(&out)?;
        std::fs::remove_file(&source)?;
        let error = finish_record(&machine.plan, Path::new("unused"), &workspace)
            .err()
            .ok_or("missing source was accepted")?;
        assert_eq!(error.code, "SAIL3134");
        assert!(error.message.contains("source_exists=Ok(false)"));
        assert!(error.message.contains("destination_parent_exists=Ok(true)"));
        assert!(!destination.exists() && !out.join(UNITS).exists());
        Ok(())
    }

    #[test]
    fn noisy_or_failed_invocations_and_registry_paths_are_not_recorded() -> TestResult {
        let build = Machine::new("build3", |root| Mode::Record {
            out: root.join("bundle"),
        })?;
        let call = build.call(false).ok_or("no call")?;
        build.compile()?;
        let out = build.out().ok_or("mode")?;
        call.record(b"", b"warning: unused\n", Some(0));
        call.record(b"", build.stderr().as_bytes(), Some(1));
        call.record(b"output", build.stderr().as_bytes(), Some(0));
        std::fs::write(
            build.plan.target.join("debug/deps/libdemo-abc.rlib"),
            format!("debuginfo {}", build.plan.registry.display()),
        )?;
        call.record(b"", build.stderr().as_bytes(), Some(0));
        assert!(!out.join(UNITS).join(&call.key).exists());
        assert_eq!(std::fs::read_dir(out.join("skipped"))?.count(), 1);
        Ok(())
    }
}
