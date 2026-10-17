//! Unselected vstd compiler-boundary reuse experiment (ADR 0048).
//!
//! Native Rust crates, build scripts and proc macros always execute through
//! Cargo's real compiler. Only a full verified vstd dependency invocation can
//! look up a recorded root artifact after those prerequisites complete. Its
//! key binds their actual --extern bytes, every compiler option, the runtime,
//! installed sysroot, solver bytes and the recorded compiler environment.
//!
//! This reuses the original shipped-build serializer and source-path mapping;
//! schema 2 cannot accept the old all-unit native-artifact bundles. It neither
//! reads nor enables the quarantined machine-local v1 store. Activation requires
//! SAIL_VSTD_BOUNDARY_RESEARCH=1. Fresh dependency-byte equality, complete native
//! inputs, diagnostic parity and useful first-run gains are not yet qualified.

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
const SCHEMA: u32 = 2;
const KIND: &str = "sail-vstd-compile-boundary";
const MANIFEST: &str = "manifest.json";
const UNITS: &str = "units";
const UNIT: &str = "unit.json";

/// Release tooling: record this run's registry units into the directory named.
pub(crate) const RECORD_ENV: &str = "SAIL_SHIPPED_RECORD";
/// An explicit shipped build directory, instead of the one beside `sail`.
pub(crate) const BUNDLE_ENV: &str = "SAIL_SHIPPED_DEPS";
/// Sail's Verus: `<virtual base>=<registry source directory>`.
pub(crate) const SOURCE_BASE_ENV: &str = "SAIL_RUSTC_DEV_SOURCE_BASE";
/// Private plumbing: the run's [`Plan`] for its driver proxies.
pub(crate) const PLAN_ENV: &str = "SAIL_MANAGED_SHIPPED";
/// Diagnosis: a directory that receives, for every registry invocation a
/// serving run looks up, its keyed inputs and the outcome.
pub(crate) const TRACE_ENV: &str = "SAIL_SHIPPED_TRACE";

/// Unselected experiment. Ordinary verification never activates this path.
pub(crate) fn research_enabled() -> bool {
    std::env::var_os("SAIL_VSTD_BOUNDARY_RESEARCH").is_some_and(|value| value == "1")
}

const TARGET: &str = "@@sail-target@@";
const REGISTRY: &str = "@@sail-registry@@";
const PACKAGE: &str = "@@sail-package@@";

/// What a run does with shipped dependencies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case")]
pub(crate) enum Mode {
    /// Serve matching invocations from the shipped build at `bundle`.
    Serve { bundle: PathBuf },
    /// Record every registry invocation into `out` (release tooling).
    Record { out: PathBuf },
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

/// The identity a shipped build must have to be used with a runtime: its
/// build manifest, actual installed compiler/sysroot bytes and virtual base.
fn identity(root: &Path, virtual_base: &str) -> Option<String> {
    // Without rustup the launcher may load another system's compiler
    // libraries. That configuration cannot establish this portable binding.
    if std::env::var_os("VERUS_USE_RUSTUP").is_some_and(|value| value == "0") {
        return None;
    }
    let manifest = std::fs::read(root.join(crate::source_build::MANIFEST)).ok()?;
    let wrapper = root.join("verus/source/target-verus/release/verus");
    let sysroot = super::runner::verifier_sysroot(&wrapper, Path::new(".")).ok()?;
    let compiler = sysroot_identity(&sysroot)?;
    Some(digest(
        format!(
            "{KIND}-v{SCHEMA}\n{}\n{virtual_base}\n{compiler}\n",
            digest(&manifest)
        )
        .as_bytes(),
    ))
}

/// The shipped build beside the running `sail` (`<release>/deps/<variant>`)
/// or named by [`BUNDLE_ENV`].
fn bundle_dir(variant: &OsStr) -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os(BUNDLE_ENV).filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(explicit));
    }
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    Some(exe.parent()?.parent()?.join("deps").join(variant))
}

fn read_manifest(bundle: &Path) -> Option<Manifest> {
    serde_json::from_slice(&std::fs::read(bundle.join(MANIFEST)).ok()?).ok()
}

/// The shipped build for the runtime at `root`, if one of its identity
/// exists.
fn accepted_bundle(root: &Path, identity: &str, virtual_base: &str) -> Option<PathBuf> {
    let bundle = bundle_dir(root.file_name()?)?;
    let manifest = read_manifest(&bundle)?;
    (manifest.schema == SCHEMA
        && manifest.kind == KIND
        && manifest.identity == identity
        && manifest.virtual_base == virtual_base)
        .then_some(bundle)
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
    if !research_enabled() {
        return None;
    }
    let virtual_base = virtual_base()?;
    let root = runtime_root(cargo_verus)?;
    // An ordinary install without a bundle needs no shipped-input probe.
    read_manifest(&bundle_dir(root.file_name()?)?)?;
    let identity = identity(&root, &virtual_base)?;
    let bundle = accepted_bundle(&root, &identity, &virtual_base)?;
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
pub(crate) fn plan(
    cargo_verus: &Path,
    target: &Path,
    registry: Option<&Path>,
    new_target: bool,
    solver: &Path,
) -> Option<Plan> {
    if !research_enabled() {
        return None;
    }
    let recording = std::env::var_os(RECORD_ENV).filter(|v| !v.is_empty());
    // Ordinary unchanged runs do not use a shipped build and must not probe
    // or hash a toolchain solely for an unused shipped-dependency identity.
    if recording.is_none() && !new_target {
        return None;
    }
    let virtual_base = virtual_base()?;
    let registry = registry?;
    if !plain_directory(registry) || !plain_directory(target) || loader_variables() {
        return None;
    }
    let root = runtime_root(cargo_verus)?;
    if recording.is_none() {
        read_manifest(&bundle_dir(root.file_name()?)?)?;
    }
    let identity = identity(&root, &virtual_base)?;
    let mode = if let Some(out) = recording {
        Mode::Record {
            out: std::path::absolute(out).ok()?,
        }
    } else {
        Mode::Serve {
            bundle: accepted_bundle(&root, &identity, &virtual_base)?,
        }
    };
    Some(Plan {
        mode,
        identity,
        registry: registry.to_owned(),
        target: target.to_owned(),
        virtual_base,
        solver_sha256: sha256_file(solver)?.0,
    })
}

/// Completes a recording run: the manifest lists every recorded invocation.
/// It fails unless a verified invocation (vstd) was recorded.
pub(crate) fn finish_record(plan: &Plan, cargo_verus: &Path, workspace: &Path) -> Result<()> {
    let Mode::Record { out } = &plan.mode else {
        return Ok(());
    };
    let failed = |m: String| Error::new("SAIL3134", m);
    // The resolution new projects start from (`sail new`).
    std::fs::copy(workspace.join(LOCKFILE), out.join(LOCKFILE))
        .map_err(|e| failed(format!("{}: {e}", workspace.join(LOCKFILE).display())))?;
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
    if !research_enabled() {
        return None;
    }
    let path = std::env::var_os(PLAN_ENV)?;
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
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

/// One compiler invocation of a registry package under a plan.
pub(crate) struct Call<'a> {
    plan: &'a Plan,
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
        let vars: Vec<_> = vars.collect();
        let lookup = |key: &str| {
            vars.iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value)
        };
        if lookup("CARGO_PKG_NAME")? != "vstd"
            || lookup("CARGO_PKG_VERSION")? != "0.0.0-2026-10-04-0306"
            || lookup("CARGO_PRIMARY_PACKAGE").is_some()
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
        let call = Self::new(plan, args, true, vars.into_iter())?;
        (call.crate_name == "vstd"
            && argument_value(&call.keyed_args, "--crate-type")
                .is_some_and(|kind| matches!(kind.as_str(), "lib" | "rlib")))
        .then_some(call)
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
                if let Mode::Record { out } = &plan.mode {
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
        let key = digest(
            &serde_json::to_vec(&serde_json::json!({
                "kind": KIND,
                "schema": SCHEMA,
                "identity": plan.identity,
                "solver_sha256": plan.solver_sha256,
                "verifying": verifying,
                "args": &keyed_args,
                "environment": &environment,
                "externs": &externs,
            }))
            .map_err(|e| e.to_string())?,
        );
        let package = format!(
            "{}-{}",
            lookup("CARGO_PKG_NAME").unwrap_or_default(),
            lookup("CARGO_PKG_VERSION").unwrap_or_default()
        );
        Ok(Self {
            plan,
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
        })
    }

    /// Extra compiler arguments when recording: name registry sources by the
    /// virtual base, under the directory Cargo names and under its canonical
    /// form (rustc's working directory for a registry package).
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
        let Mode::Record { out } = &self.plan.mode else {
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
                self.normalizer.normalize(&text).into_bytes()
            } else {
                if registries
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
        let (files, dep_info, text) = self.stage_outputs(staging)?;
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
        let declared = dep_info_environment(&self.normalizer.normalize(&text));
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
            files,
            dep_info,
            sources,
            declared,
            stdout: text_file("stdout", stdout)?,
            stderr: text_file("stderr", stderr)?,
        };
        std::fs::write(
            staging.join(UNIT),
            serde_json::to_vec_pretty(&unit).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    }

    /// Serves this invocation from the shipped build: `None` for a miss, after
    /// which the caller compiles as usual. A damaged entry prints one
    /// `warning[SAIL3133]` on `err` and is a miss too.
    pub(crate) fn serve(&self, err: &mut dyn Write) -> Option<Replay> {
        let Mode::Serve { bundle } = &self.plan.mode else {
            return None;
        };
        let entry = bundle.join(UNITS).join(&self.key);
        let outcome = self.try_serve(&entry);
        self.trace(match &outcome {
            Ok(_) => "served",
            Err(Refusal::Miss(reason) | Refusal::Damaged(reason)) => reason,
        });
        match outcome {
            Ok(replay) => Some(replay),
            Err(Refusal::Miss(_)) => None,
            Err(Refusal::Damaged(reason)) => {
                let _ = writeln!(
                    err,
                    "warning[SAIL3133]: ignoring Sail's shipped build of {} ({}): {reason}; compiling it normally",
                    self.crate_name, self.package
                );
                None
            }
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
            }))
            .unwrap_or_default(),
        );
    }

    fn try_serve(&self, entry: &Path) -> std::result::Result<Replay, Refusal> {
        let Ok(bytes) = std::fs::read(entry.join(UNIT)) else {
            return Err(Refusal::Miss("no entry for this key".to_owned()));
        };
        let unit: Unit = serde_json::from_slice(&bytes)
            .map_err(|e| Refusal::Damaged(format!("unreadable entry: {e}")))?;
        if unit.schema != SCHEMA || unit.key != self.key || unit.verifying != self.verifying {
            return Err(Refusal::Damaged("entry of another key".to_owned()));
        }
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
        for (name, value) in &unit.declared {
            let name = self.normalizer.localize(name);
            let current = self.vars.get(&name).map(|v| self.normalizer.normalize(v));
            if current != *value {
                return Err(Refusal::Miss(format!("declared variable {name} differs")));
            }
        }
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
                    write_new(&to, self.normalizer.localize(&text).as_bytes(), false)
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
        Ok(Replay {
            stdout: stdout.into_bytes(),
            stderr: stderr.into_bytes(),
            unit: format!("{}/{}", self.plan.identity, self.key),
        })
    }
}

/// A served invocation's recorded output, with this machine's directories.
pub(crate) struct Replay {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// `<shipped build identity>/<key>`, the target's provenance.
    pub unit: String,
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

    fn plan(root: &Path, mode: Mode) -> Plan {
        Plan {
            mode,
            identity: "identity".into(),
            registry: root.join("home/registry/src/index.crates.io-0"),
            target: root.join("target"),
            virtual_base: "/rustc-dev/0123".into(),
            solver_sha256: "solver-bytes".into(),
        }
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
                Mode::Record { out } => Some(out.clone()),
                Mode::Serve { .. } => None,
            }
        }
    }

    #[test]
    fn boundary_accepts_only_unselected_verified_vstd_and_fresh_dependency_bytes()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut machine = Machine::new("vstd-boundary", |root| Mode::Record {
            out: root.join("bundle"),
        })?;
        let ordinary_vars = vars(&machine.root, &[]);
        assert!(
            Call::vstd_boundary(&machine.plan, &machine.args, ordinary_vars.into_iter()).is_none()
        );
        let package = machine.plan.registry.join("vstd-0.0.0-2026-10-04-0306");
        std::fs::create_dir_all(package.join("src"))?;
        let manifest = package.join("Cargo.toml");
        std::fs::write(
            &manifest,
            "[package]\nname='vstd'\nversion='0.0.0-2026-10-04-0306'\n[package.metadata.verus]\nverify=true\nis-vstd=true\n",
        )?;
        machine.args[2] = "vstd".into();
        machine.args[3] = package.join("src/lib.rs").into_os_string();
        machine.args.extend(["--crate-type".into(), "lib".into()]);
        let invocation_vars = vec![
            (OsString::from("CARGO_PKG_NAME"), OsString::from("vstd")),
            ("CARGO_PKG_VERSION".into(), "0.0.0-2026-10-04-0306".into()),
            ("CARGO_MANIFEST_DIR".into(), package.into_os_string()),
        ];
        let key = || {
            Call::vstd_boundary(
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
        for (name, value) in [
            ("CARGO_PRIMARY_PACKAGE", "1"),
            ("CARGO_PKG_VERSION", "other"),
            ("CARGO_PKG_NAME", "build_script_build"),
        ] {
            let mut changed = invocation_vars.clone();
            changed.retain(|(key, _)| key != name);
            changed.push((name.into(), value.into()));
            assert!(
                Call::vstd_boundary(&machine.plan, &machine.args, changed.into_iter()).is_none(),
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
        let mut err = Vec::new();
        let replay = served.serve(&mut err).ok_or("not served")?;
        assert_eq!(err, Vec::<u8>::new());
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
    fn mismatches_are_silent_misses_and_damage_warns() -> TestResult {
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
        let source = user.plan.registry.join("demo-0.1.0/src/lib.rs");
        std::fs::write(&source, "pub fn f() { /* edited */ }\n")?;
        let mut err = Vec::new();
        assert!(user.call(false).ok_or("no call")?.serve(&mut err).is_none());
        assert_eq!(err, Vec::<u8>::new());
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
            assert!(user.call(false).ok_or("no call")?.serve(&mut err).is_none());
            assert_eq!(err, Vec::<u8>::new());
            std::fs::remove_file(&source)?;
            std::fs::rename(redirected, &source)?;
        }

        let manifest = user.plan.registry.join("demo-0.1.0/Cargo.toml");
        let original_manifest = std::fs::read(&manifest)?;
        std::fs::write(&manifest, "[package]\nname = \"changed\"\n")?;
        assert!(user.call(false).ok_or("no call")?.serve(&mut err).is_none());
        assert_eq!(err, Vec::<u8>::new());
        std::fs::write(manifest, original_manifest)?;

        // A dependency compiled here (other bytes) is another key: a miss.
        let dependency = user.plan.target.join("debug/deps/libdep-1.rlib");
        std::fs::write(&dependency, b"local bytes")?;
        let other = user.call(false).ok_or("no call")?;
        assert_ne!(other.key, call.key);
        assert!(other.serve(&mut err).is_none());
        assert_eq!(err, Vec::<u8>::new());
        std::fs::write(&dependency, b"dependency bytes")?;

        // Even a malformed final destination leaves no earlier outputs.
        let original_unit = std::fs::read(entry.join(UNIT))?;
        let mut malformed: Unit = serde_json::from_slice(&original_unit)?;
        malformed.files.last_mut().ok_or("no files")?.name = "../redirected".into();
        std::fs::write(entry.join(UNIT), serde_json::to_vec(&malformed)?)?;
        assert!(user.call(false).ok_or("no call")?.serve(&mut err).is_none());
        assert!(String::from_utf8(std::mem::take(&mut err))?.starts_with("warning[SAIL3133]"));
        for name in ["libdemo-abc.rlib", "libdemo-abc.rmeta", "demo-abc.d"] {
            assert!(!user.plan.target.join("debug/deps").join(name).exists());
        }
        std::fs::write(entry.join(UNIT), original_unit)?;

        // A damaged stored file warns once, places nothing and is a miss.
        std::fs::write(entry.join("files/libdemo-abc.rlib"), b"corrupt")?;
        assert!(user.call(false).ok_or("no call")?.serve(&mut err).is_none());
        let warning = String::from_utf8(err)?;
        assert!(warning.starts_with("warning[SAIL3133]"), "{warning}");
        assert_eq!(warning.lines().count(), 1);
        for name in ["libdemo-abc.rlib", "libdemo-abc.rmeta", "demo-abc.d"] {
            assert!(
                !user.plan.target.join("debug/deps").join(name).exists(),
                "{name}"
            );
        }

        // An unreadable entry is damaged too; a missing one is silent.
        std::fs::write(entry.join(UNIT), b"{")?;
        let mut err = Vec::new();
        assert!(user.call(false).ok_or("no call")?.serve(&mut err).is_none());
        assert!(String::from_utf8(err)?.starts_with("warning[SAIL3133]"));
        std::fs::remove_dir_all(&entry)?;
        let mut err = Vec::new();
        assert!(user.call(false).ok_or("no call")?.serve(&mut err).is_none());
        assert_eq!(err, Vec::<u8>::new());
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
