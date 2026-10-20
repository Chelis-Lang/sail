//! Owned Cargo invocation, artifact validation, and complete report reuse.

use super::diagnostics::{Diagnostics, Phase};
use super::environment::input_environment_key;
use super::files::{digest, event, file_digest, json};
use super::input::{SNAPSHOT_FIELDS, managed_metadata as metadata, snapshot_metadata};
#[cfg(test)]
use super::receipts::artifact_hashes;
use super::receipts::{
    Invocation, PhaseDigests, RECEIPT_VERSION, Receipt, Runtime, complete, read,
};
use crate::{Error, Result};
use sail_api::project::{
    Currentness, Execution, NoReuse, ProjectResult, SCHEMA_VERSION, TargetResult, VerifiedAs,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
#[cfg(not(unix))]
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// The one note for a result whose inputs could not be confirmed unchanged.
pub(crate) const INPUTS_UNCONFIRMED: &str =
    "SAIL3114: inputs could not be confirmed unchanged; re-run for a current result";

/// Explicit managed verification environment; no caller Cargo artifacts are changed.
#[derive(Debug, Clone)]
pub struct Config {
    /// Project directory in which Cargo-Verus is invoked.
    pub workspace: PathBuf,
    /// Sail cache root for owned Cargo directories and receipts.
    pub cache: PathBuf,
    /// Installed Sail executable, used through private aliases.
    pub sail: PathBuf,
    /// Actual Cargo-Verus executable.
    pub cargo_verus: PathBuf,
    /// Actual Cargo executable (never the private adapter).
    pub cargo: PathBuf,
    /// The toolchain every Cargo, rustc and Cargo-Verus process of the run
    /// is started with (ADR 0058): the engine's pinned toolchain unless the
    /// caller or the project chose one.
    pub cargo_toolchain: super::CargoToolchain,
    /// Sail's solver shim.
    pub shim: PathBuf,
    /// Actual solver executable.
    pub z3: PathBuf,
    /// Existing cache daemon socket, if available.
    pub socket: Option<PathBuf>,
    /// Original Cargo-Verus arguments, excluding Sail options.
    pub args: Vec<String>,
    /// Force a fresh proof even when a receipt exists.
    pub force: bool,
    /// Ask Verus for the AIR and SMT transcript logs that attribute each
    /// solver query to an obligation: per-obligation records, per-check
    /// cache provenance and counterexample models. Callers that read the
    /// report set it (`--output json`, jobs, MCP); plain `sail verify`,
    /// which prints only Verus's own output, does not (ADR 0047).
    /// Interactive-engine runs always log.
    pub attribution_logs: bool,
}

struct Paths {
    target: PathBuf,
    cargo_alias: PathBuf,
    proxy_alias: PathBuf,
    receipts: PathBuf,
    runtime: PathBuf,
    run: PathBuf,
    build_inputs: PathBuf,
    input_state: PathBuf,
    compiler_inputs: PathBuf,
    /// One empty file per proof context of this compiler family.
    family_contexts: PathBuf,
    /// The workspace's directory of proof contexts.
    projects: PathBuf,
    /// Whether this run may reuse and record proof receipts. A run with a
    /// process-loader variable set may not (ADR 0046).
    reuse: bool,
    /// This run's shipped-dependency plan file, when it has one (ADR 0048).
    shipped_plan: Option<PathBuf>,
    /// `SAIL_RUSTC_DEV_SOURCE_BASE` for Sail's Verus (ADR 0048).
    source_base: Option<std::ffi::OsString>,
}

fn selected(args: &[String], names: &BTreeSet<String>) -> Vec<String> {
    let mut packages = Vec::new();
    let mut it = args.iter().take_while(|s| s.as_str() != "--");
    while let Some(s) = it.next() {
        if s == "-p" || s == "--package" {
            if let Some(v) = it.next() {
                packages.push(v.clone());
            }
        } else if let Some(v) = s.strip_prefix("--package=") {
            packages.push(v.to_owned());
        } else if let Some(v) = s.strip_prefix("-p") {
            packages.push(v.strip_prefix('=').unwrap_or(v).to_owned());
        }
    }
    if packages.is_empty() {
        names.iter().cloned().collect()
    } else {
        packages
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "The context explicitly enumerates all executable and environment bindings."
)]
fn context(config: &Config, engine: Option<&crate::engine::Request>) -> Result<Context> {
    let mut identity = Vec::new();
    let mut hashed = super::files::FileDigests::default();
    if let Some(engine) = engine {
        // The engine install is validated by content before anything of it is
        // executed; its files then join the identity like the stock toolchain.
        hashed.prefetch(
            [engine.manifest.as_path(), engine.stock_wrapper.as_path()]
                .into_iter()
                .chain(engine.artifacts.iter().map(PathBuf::as_path)),
        );
        crate::engine::validate_request(engine, |path| hashed.hash(path))?;
    }
    // The two sysroot probes are independent processes; their results are
    // consumed in the original order, so the first reported error is unchanged.
    let wrapper = actual_wrapper(&config.cargo_verus);
    let rustc = resolve_program(
        &std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()),
        &config.workspace,
    );
    let rustup = resolve_program(std::ffi::OsStr::new("rustup"), &config.workspace);
    let (cargo_probe, verifier_probe) = std::thread::scope(|scope| {
        let cargo_probe = rustc
            .as_ref()
            .ok()
            .map(|rustc| scope.spawn(move || sysroot_probe(config, rustc).output()));
        let verifier_probe = wrapper
            .as_ref()
            .ok()
            .map(|wrapper| scope.spawn(move || verifier_sysroot(wrapper, &config.workspace)));
        let failed = || Error::new("SAIL3104", "toolchain probe thread failed");
        (
            cargo_probe.map(|probe| probe.join().map_err(|_| failed())),
            verifier_probe.map(|probe| probe.join().map_err(|_| failed())),
        )
    });
    // Installed executables, engine files and sysroot libraries are identified
    // by stat: any change to them changes the context, so the next run is fresh.
    let installed = |path: &Path| super::files::stat_identity(path);
    for path in [
        &config.sail,
        &config.cargo_verus,
        &config.cargo,
        &config.z3,
        &config.shim,
    ] {
        identity.push((path.to_string_lossy().into_owned(), installed(path)?));
    }
    if let Some(engine) = engine {
        for path in [&engine.manifest, &engine.stock_wrapper]
            .into_iter()
            .chain(engine.artifacts.iter())
        {
            identity.push((path.to_string_lossy().into_owned(), installed(path)?));
        }
    }
    let wrapper = wrapper?;
    identity.push((wrapper.to_string_lossy().into_owned(), installed(&wrapper)?));
    let mut environment = context_environment(config);
    let rustc = rustc?;
    identity.push((rustc.to_string_lossy().into_owned(), installed(&rustc)?));
    let actual = cargo_probe
        .ok_or_else(|| Error::new("SAIL3104", "cannot identify Rust toolchain"))??
        .map_err(|e| Error::new("SAIL3104", format!("cannot identify Rust toolchain: {e}")))?;
    if !actual.status.success() {
        return Err(Error::new(
            "SAIL3104",
            String::from_utf8_lossy(&actual.stderr),
        ));
    }
    let rustup = rustup?;
    identity.push((rustup.to_string_lossy().into_owned(), installed(&rustup)?));
    let cargo_sysroot = PathBuf::from(String::from_utf8_lossy(&actual.stdout).trim());
    let verifier_sysroot = verifier_probe
        .ok_or_else(|| Error::new("SAIL3104", "cannot identify verifier toolchain"))???;
    identity.push((
        config
            .cargo_verus
            .with_file_name("rust_verify")
            .to_string_lossy()
            .into_owned(),
        installed(&wrapper.with_file_name("rust_verify"))?,
    ));
    for path in sysroot_files(&BTreeSet::from([cargo_sysroot, verifier_sysroot]))? {
        identity.push((path.to_string_lossy().into_owned(), installed(&path)?));
    }
    // The exact Cargo configuration bytes join the context, not only its path.
    let invocation = std::path::absolute(&config.workspace)
        .map_err(|e| Error::new("SAIL3104", e.to_string()))?;
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".cargo")));
    for path in super::input::configuration_paths(&invocation, home.as_deref())
        .into_iter()
        .filter(|p| p.is_file())
    {
        identity.push((path.to_string_lossy().into_owned(), hashed.hash(&path)?));
    }
    for parent in invocation.ancestors() {
        for file in [
            ".cargo/config",
            ".cargo/config.toml",
            "rust-toolchain",
            "rust-toolchain.toml",
        ] {
            let path = parent.join(file);
            if path.is_file() {
                environment.insert(
                    path.as_os_str().as_encoded_bytes().to_vec(),
                    hashed.hash(&path)?.into_bytes(),
                );
            }
        }
    }
    for value in config
        .args
        .iter()
        .chain(std::env::var("CARGO_BUILD_TARGET").ok().iter())
    {
        let candidate = value.strip_prefix("--target=").unwrap_or(value);
        if Path::new(candidate)
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|v| v.eq_ignore_ascii_case("json"))
        {
            let path = config.workspace.join(candidate);
            if path.is_file() {
                identity.push((path.to_string_lossy().into_owned(), hashed.hash(&path)?));
            }
        }
    }
    identity.sort();
    let state = config.cache.join("runtime-identities");
    std::fs::create_dir_all(&state).map_err(|e| Error::new("SAIL3104", e.to_string()))?;
    let identity_sha =
        digest(&serde_json::to_vec(&identity).map_err(|e| Error::new("SAIL3104", e.to_string()))?);
    if !state.join(format!("{identity_sha}.json")).is_file() {
        json(&state.join(format!("{identity_sha}.json")), &identity)?;
    }
    environment.insert(
        b"sail-runtime-identities".to_vec(),
        identity_sha.clone().into_bytes(),
    );
    let environment = environment.into_iter().collect::<Vec<_>>();
    let (hash, compiler_hash) = context_hashes(&config.args, engine, &identity, &environment)?;
    let store_hash = store_family_hash(&config.args, engine, &identity, &environment)?;
    super::files::json(&state.join(format!("{hash}.link")), &identity_sha)?;
    Ok(Context {
        hash,
        compiler_hash,
        store_hash,
    })
}

/// The Cargo toolchain's `rustc --print sysroot`, run as Cargo runs rustc:
/// with the selected toolchain (ADR 0058).
fn sysroot_probe(config: &Config, rustc: &Path) -> Command {
    let mut command = Command::new(rustc);
    config.cargo_toolchain.apply(&mut command);
    command
        .args(["--print", "sysroot"])
        .current_dir(&config.workspace);
    command
}

/// The proof context's environment: the variables the tools read, the
/// resolved homes, and the toolchain Sail selects, which every tool of the
/// run sees exactly as if the caller had set it (ADR 0058).
fn context_environment(config: &Config) -> BTreeMap<Vec<u8>, Vec<u8>> {
    let mut environment: BTreeMap<Vec<u8>, Vec<u8>> = std::env::vars_os()
        .filter(|(k, _)| input_environment_key(k))
        .map(|(k, v)| (k.as_encoded_bytes().to_vec(), v.as_encoded_bytes().to_vec()))
        .chain(super::environment::resolved_homes())
        .collect();
    if let Some(toolchain) = config.cargo_toolchain.selected() {
        environment.insert(
            super::cargo_toolchain::ENV.as_bytes().to_vec(),
            toolchain.as_bytes().to_vec(),
        );
    }
    environment
}

/// The compiler family as the dependency store sees it (ADR 0045): the
/// compiler family digest without the arguments that only select which
/// workspace packages and targets Cargo builds (`-p`, `--workspace`,
/// `--exclude`, `--lib`, `--bin`, `--bins`). Cargo-Verus gives a dependency
/// the same verifier arguments whichever members are selected; a selection
/// that changes how the dependency is compiled (feature unification) changes
/// Cargo's unit hashes, so Cargo rebuilds the seeded units and the stored
/// receipt does not match. So `-p a` in a workspace and a single-package
/// project share vstd's entry (rc.5 finding 15). Arguments after `--`
/// (Verus's own) are kept.
fn store_family_hash(
    args: &[String],
    engine: Option<&crate::engine::Request>,
    identity: &[(String, String)],
    environment: &[(Vec<u8>, Vec<u8>)],
) -> Result<String> {
    Ok(digest(
        &serde_json::to_vec(&(
            "sail-dependency-store-family-v1",
            without_selection(args),
            engine.map(crate::engine::Request::artifact_identity),
            identity,
            environment,
        ))
        .map_err(|e| Error::new("SAIL3104", e.to_string()))?,
    ))
}

/// `args` without Cargo's package and target selection, up to `--`.
fn without_selection(args: &[String]) -> Vec<&str> {
    let mut out = Vec::new();
    let mut it = args.iter().map(String::as_str);
    while let Some(arg) = it.next() {
        if arg == "--" {
            out.push(arg);
            out.extend(it.by_ref());
            break;
        }
        match arg {
            "-p" | "--package" | "--exclude" | "--bin" => {
                it.next();
            }
            "--workspace" | "--all" | "--lib" | "--bins" => {}
            _ if ["--package=", "--exclude=", "--bin=", "-p"]
                .iter()
                .any(|prefix| arg.starts_with(prefix)) => {}
            _ => out.push(arg),
        }
    }
    out
}

/// Files of the installed sysroots that compilation can load: `bin/rustc`,
/// `bin/cargo`, the files directly in `lib/` and in each `lib/rustlib/*/lib/`.
fn sysroot_files(sysroots: &BTreeSet<PathBuf>) -> Result<Vec<PathBuf>> {
    let unreadable =
        |e: std::io::Error| Error::new("SAIL3104", format!("cannot list sysroot: {e}"));
    let mut files = BTreeSet::new();
    let directory = |dir: &Path, files: &mut BTreeSet<PathBuf>| -> Result<()> {
        match std::fs::read_dir(dir) {
            Ok(entries) => {
                for entry in entries {
                    let path = entry.map_err(unreadable)?.path();
                    if path.is_file() {
                        files.insert(path);
                    }
                }
                Ok(())
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(unreadable(error)),
        }
    };
    for sysroot in sysroots {
        for name in ["rustc", "cargo"] {
            let path = sysroot.join("bin").join(name);
            if path.is_file() {
                files.insert(path);
            }
        }
        directory(&sysroot.join("lib"), &mut files)?;
        match std::fs::read_dir(sysroot.join("lib/rustlib")) {
            Ok(entries) => {
                for entry in entries {
                    let library = entry.map_err(unreadable)?.path().join("lib");
                    if library.is_dir() {
                        directory(&library, &mut files)?;
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(unreadable(error)),
        }
    }
    Ok(files.into_iter().collect())
}

struct Context {
    hash: String,
    compiler_hash: String,
    /// The dependency store's view of the compiler family
    /// ([`store_family_hash`]).
    store_hash: String,
}

fn context_hashes(
    args: &[String],
    engine: Option<&crate::engine::Request>,
    identity: &[(String, String)],
    environment: &[(Vec<u8>, Vec<u8>)],
) -> Result<(String, String)> {
    // Keep the existing proof-context encoding: focus selectors still bind
    // reports, receipts, runtime validation and prepared invocation contexts.
    let hash = digest(
        &serde_json::to_vec(&(1, args, engine, identity, environment))
            .map_err(|e| Error::new("SAIL3104", e.to_string()))?,
    );
    let compiler_engine = engine.map(crate::engine::Request::artifact_identity);
    // Cargo package/target selection and the driver-local focus selector do
    // not change the compiler family. Cargo fingerprints the selected units;
    // proof contexts and requested-target receipts still bind the exact args.
    // Compiler/solver bytes, other arguments and the exact environment stay
    // bound. Cargo freshness supplies ordinary artifacts; exact proof receipts
    // authorize skipped verification targets.
    let compiler_hash = digest(
        &serde_json::to_vec(&(
            "sail-managed-compiler-artifacts-v1",
            without_selection(args),
            compiler_engine,
            identity,
            environment,
        ))
        .map_err(|e| Error::new("SAIL3104", e.to_string()))?,
    );
    Ok((hash, compiler_hash))
}

/// The manifest and target name of every selected target.
fn selected_targets(
    data: &Value,
    expected: &BTreeSet<(String, String, String)>,
) -> Vec<(PathBuf, String)> {
    expected
        .iter()
        .filter_map(|(id, name, _)| {
            let package = data["packages"]
                .as_array()?
                .iter()
                .find(|package| package["id"] == *id)?;
            Some((
                PathBuf::from(package["manifest_path"].as_str()?),
                name.clone(),
            ))
        })
        .collect()
}

fn actual_wrapper(cargo_verus: &Path) -> Result<PathBuf> {
    // Cargo-Verus uses current_exe, not its invoked symlink's directory.
    cargo_verus
        .canonicalize()
        .map(|p| p.with_file_name("verus"))
        .map_err(|e| Error::new("SAIL3104", e.to_string()))
}

pub(super) fn verifier_sysroot(wrapper: &Path, workspace: &Path) -> Result<PathBuf> {
    let output = Command::new(wrapper)
        .args(["--version", "--output-json"])
        // This is verifier metadata, including when a native proxy asks for
        // its current sysroot. Cargo mode requires a leading rustc argument
        // and would reject this standalone version query instead.
        .env_remove("__VERUS_DRIVER_VIA_CARGO__")
        .current_dir(workspace)
        .output()
        .map_err(|e| Error::new("SAIL3104", e.to_string()))?;
    let version: Value = serde_json::from_slice(&output.stdout).map_err(|e| {
        Error::new(
            "SAIL3104",
            format!("cannot identify verifier toolchain: {e}"),
        )
    })?;
    let toolchain = version["verus"]["toolchain"]
        .as_str()
        .and_then(|v| v.split_whitespace().next())
        .ok_or_else(|| {
            Error::new(
                "SAIL3104",
                "verifier did not report its active Rust toolchain",
            )
        })?;
    let output = Command::new("rustup")
        .args(["run", toolchain, "rustc", "--print", "sysroot"])
        .current_dir(workspace)
        .output()
        .map_err(|e| Error::new("SAIL3104", e.to_string()))?;
    if !output.status.success() {
        return Err(Error::new(
            "SAIL3104",
            format!(
                "cannot resolve verifier toolchain: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        ));
    }
    Ok(PathBuf::from(
        String::from_utf8_lossy(&output.stdout).trim(),
    ))
}

fn resolve_program(program: &std::ffi::OsStr, workspace: &Path) -> Result<PathBuf> {
    // Keep the invoked name: rustup dispatches by argv[0], so canonicalizing
    // rustc's symlink and executing `rustup --print sysroot` changes its role.
    let path = Path::new(program);
    let resolved = if path.components().count() > 1 {
        Some(workspace.join(path)).filter(|p| p.is_file())
    } else {
        std::env::var_os("PATH")
            .into_iter()
            .flat_map(|v| std::env::split_paths(&v).collect::<Vec<_>>())
            .map(|dir| dir.join(path))
            .find(|p| p.is_file())
    };
    resolved.ok_or_else(|| {
        Error::new(
            "SAIL3104",
            format!("cannot resolve compiler {}", path.display()),
        )
    })
}

fn alias(original: &Path, path: &Path) -> Result<()> {
    // An alias that resolves to the original's own inode is that file, so no
    // byte comparison could differ. Copies and stale links keep the original
    // full content comparison and replacement.
    if same_file(original, path) || path.is_file() && file_digest(path)? == file_digest(original)? {
        return Ok(());
    }
    let _ = std::fs::remove_file(path);
    std::fs::hard_link(original, path)
        .or_else(|_| std::fs::copy(original, path).map(|_| ()))
        .map_err(|e| Error::new("SAIL3104", e.to_string()))
}

fn same_file(original: &Path, path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        match (std::fs::metadata(original), std::fs::metadata(path)) {
            (Ok(original), Ok(path)) => {
                original.is_file() && original.dev() == path.dev() && original.ino() == path.ino()
            }
            _ => false,
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (original, path);
        false
    }
}

/// Exclusive use of one compiler family: its shared Cargo target and every
/// proof context in it. A proof context determines its family, so this one
/// owner also serialises each context's receipts and run records. The shared
/// workspace lock keeps `sail cache projects prune` from removing the
/// workspace while the run, or its caller reading the run's records, uses it.
struct Ownership {
    _family: File,
    workspace: File,
}

impl Ownership {
    /// Releases the compiler family and keeps the workspace held.
    fn into_workspace(self) -> File {
        self.workspace
    }
}

fn ownership_lock(directory: &Path) -> Result<File> {
    managed_lock(&directory.join("ownership.lock"), false)
}

/// Takes a managed lock: shared for the workspace, exclusive for a family.
fn managed_lock(path: &Path, shared: bool) -> Result<File> {
    // Another managed run of the same workspace holds the target: wait for it,
    // up to `SAIL_MANAGED_LOCK_WAIT_SECS` (default 30 minutes). A cancelled
    // job's process is killed, which releases or abandons the wait.
    let wait = std::env::var("SAIL_MANAGED_LOCK_WAIT_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .map_or(LOCK_WAIT, std::time::Duration::from_secs);
    super::projects::lock(path, shared, Instant::now() + wait).map_err(|e| {
        if e.kind() == std::io::ErrorKind::WouldBlock {
            Error::new("SAIL3104", format!("managed target is in use: {e}"))
        } else {
            Error::new("SAIL3104", e.to_string())
        }
    })
}

/// How long a managed run waits for another run of the same workspace.
const LOCK_WAIT: std::time::Duration = std::time::Duration::from_mins(30);

fn prepare(
    config: &Config,
    workspace: &Path,
    hash: &str,
    compiler_hash: &str,
    run_id: &str,
) -> Result<(Paths, Ownership)> {
    let root = config
        .cache
        .join("projects")
        .join(digest(workspace.as_os_str().as_encoded_bytes()));
    let context = root.join(hash);
    let compiler = root.join("compiler-artifacts").join(compiler_hash);
    // One Cargo target per workspace and compiler identity, as plain `cargo
    // verus` shares one target directory: repair functions and full runs of
    // the same compiler invocation share build products, and Cargo's own
    // fingerprints decide rebuilds. Proof receipts stay per proof context. The
    // owner survives clean, invocation and receipt publication.
    let workspace_lock = managed_lock(&root.join(super::projects::WORKSPACE_LOCK), true)?;
    let ownership = Ownership {
        _family: ownership_lock(&compiler)?,
        workspace: workspace_lock,
    };
    let paths = Paths {
        target: compiler.join("target"),
        cargo_alias: compiler.join("sail-cargo"),
        proxy_alias: compiler.join("sail-verus-driver"),
        receipts: context.join("receipts"),
        runtime: context.join("runtime.json"),
        build_inputs: context.join("build-inputs.json"),
        input_state: context.join("input-state.json"),
        compiler_inputs: compiler.join("build-inputs.json"),
        run: context.join("runs").join(run_id),
        family_contexts: compiler.join("contexts"),
        projects: root,
        reuse: true,
        shipped_plan: None,
        source_base: None,
    };
    for dir in [
        &paths.receipts,
        &paths.run,
        &paths.run.join("invocations"),
        &paths.run.join("sidecars"),
        &paths.run.join("pending-receipts"),
        &paths.family_contexts,
    ] {
        std::fs::create_dir_all(dir).map_err(|e| Error::new("SAIL3104", e.to_string()))?;
    }
    // Registers this proof context in its family, so that a sibling context
    // can find its dependency receipts (`sibling_receipt`).
    let member = paths.family_contexts.join(hash);
    if !member.is_file() {
        std::fs::write(&member, []).map_err(|e| Error::new("SAIL3104", e.to_string()))?;
    }
    alias(&config.sail, &paths.cargo_alias)?;
    alias(&config.sail, &paths.proxy_alias)?;
    Ok((paths, ownership))
}

/// The directory Cargo actually builds in for this invocation's owned target.
///
/// `cargo verus focus` does not verify dependencies, so it places its artifacts
/// in a child `verus-partial` directory of the given target directory
/// (cargo-verus `src/subcommands.rs`, `verify_deps == false`). It also enables
/// checksum freshness, so byte-identical source leaves that build fresh and no
/// verifier runs. Cleaning must address the directory that holds that build.
fn cargo_build_target(target: &Path, args: &[String]) -> PathBuf {
    if focused(args) {
        target.join("verus-partial")
    } else {
        target.to_path_buf()
    }
}

/// Cargo's cache-directory tag. `cargo clean` refuses a target without it,
/// and a target seeded from the dependency store (ADR 0045) starts without
/// one, since Cargo writes it only when it creates the directory itself.
const CACHEDIR_TAG: &[u8] = b"Signature: 8a477f597d28d172789f06886806bc55\n\
# This file is a cache directory tag created by cargo.\n\
# For information about cache directory tags see https://bford.info/cachedir/\n";

/// Forces Cargo to compile `packages` again in Sail's target.
///
/// `cargo clean -p` also deletes the packages' rustc incremental state. That
/// state is only a cache: rustc checks it against the source contents and the
/// compiler options before using any of it, and plain Cargo keeps it across
/// edits. Deleting it made every edit of a large crate about 1 s slower than
/// plain Cargo-Verus (vest), so the clean leaves it in place (ADR 0047).
fn clean(config: &Config, target: &Path, packages: &[String]) -> Result<()> {
    if packages.is_empty() {
        return Ok(());
    }
    let build_target = cargo_build_target(target, &config.args);
    let tag = build_target.join("CACHEDIR.TAG");
    if build_target.is_dir() && !tag.exists() {
        std::fs::write(&tag, CACHEDIR_TAG).map_err(|e| Error::new("SAIL3105", e.to_string()))?;
    }
    let kept = keep_incremental(&build_target);
    let cleaned = cargo_clean(config, &build_target, packages);
    restore_incremental(kept);
    cleaned
}

/// Moves each rustc incremental directory in `build_target`
/// (`<profile>/incremental` and `<target triple>/<profile>/incremental`)
/// aside, so that `cargo clean` cannot delete it.
fn keep_incremental(build_target: &Path) -> Vec<(PathBuf, PathBuf)> {
    let mut kept = Vec::new();
    let children = |directory: &Path| -> Vec<PathBuf> {
        std::fs::read_dir(directory)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.file_type().is_ok_and(|t| t.is_dir()))
            .map(|entry| entry.path())
            .collect()
    };
    for profile in children(build_target).into_iter().flat_map(|child| {
        let mut found = children(&child);
        found.push(child);
        found
    }) {
        // A run killed during its clean leaves its copy behind; the target
        // is owned exclusively, so no other run is using one.
        for stale in std::fs::read_dir(&profile).into_iter().flatten().flatten() {
            if stale
                .file_name()
                .to_string_lossy()
                .starts_with("incremental.sail-kept-")
            {
                let _ = std::fs::remove_dir_all(stale.path());
            }
        }
        let incremental = profile.join("incremental");
        if !std::fs::symlink_metadata(&incremental).is_ok_and(|m| m.is_dir()) {
            continue;
        }
        let aside = profile.join(format!("incremental.sail-kept-{}", std::process::id()));
        if std::fs::rename(&incremental, &aside).is_ok() {
            kept.push((aside, incremental));
        }
    }
    kept
}

/// Puts the directories [`keep_incremental`] moved back. A directory that
/// cannot be restored is deleted: the cache is lost, nothing else.
fn restore_incremental(kept: Vec<(PathBuf, PathBuf)>) {
    for (aside, incremental) in kept {
        if std::fs::symlink_metadata(&incremental).is_ok()
            || std::fs::rename(&aside, &incremental).is_err()
        {
            let _ = std::fs::remove_dir_all(&aside);
        }
    }
}

fn cargo_clean(config: &Config, build_target: &Path, packages: &[String]) -> Result<()> {
    let mut cmd = Command::new(&config.cargo);
    config.cargo_toolchain.apply(&mut cmd);
    // Cargo-Verus builds with a different rustc fingerprint, so a clean that
    // rewrote the target's rustc-info cache would force the following build to
    // probe its compiler driver again. The clean neither reads nor writes that
    // cache; Cargo still probes rustc for this invocation itself.
    cmd.current_dir(&config.workspace)
        .env("CARGO_CACHE_RUSTC_INFO", "0")
        .args(["clean", "--target-dir"])
        .arg(build_target);
    for p in packages {
        cmd.arg("-p").arg(p);
    }
    for window in config.args.windows(2) {
        if window[0] == "--manifest-path" {
            cmd.arg("--manifest-path").arg(&window[1]);
        }
    }
    for arg in &config.args {
        if arg.starts_with("--manifest-path=") {
            cmd.arg(arg);
        }
    }
    let output = cmd
        .output()
        .map_err(|e| Error::new("SAIL3105", e.to_string()))?;
    if !output.status.success() {
        return Err(Error::new(
            "SAIL3105",
            String::from_utf8_lossy(&output.stderr),
        ));
    }
    Ok(())
}

fn invoke(
    config: &Config,
    paths: &Paths,
    out: &mut Option<&mut dyn Write>,
    err: &mut dyn Write,
    seeded: &[super::store::Seeded],
) -> Result<Option<i32>> {
    let split = config
        .args
        .iter()
        .position(|a| a == "--")
        .unwrap_or(config.args.len());
    let mut cmd = Command::new(&config.cargo_verus);
    // Cargo-Verus, the Cargo it runs through Sail's adapter
    // (`SAIL_MANAGED_CARGO`) and every compiler below inherit the selection.
    config.cargo_toolchain.apply(&mut cmd);
    cmd.current_dir(&config.workspace)
        .args([
            "verus",
            if focused(&config.args) {
                "focus"
            } else {
                "verify"
            },
        ])
        .arg("--target-dir")
        .arg(&paths.target)
        .args(&config.args[..split])
        // Cargo-Verus parses relevant selectors only until the first forwarded
        // Cargo argument. Keep the owned relevant target-dir first and the
        // irrelevant message format after the caller's unchanged Cargo options.
        .arg("--message-format=json-render-diagnostics")
        // Cargo writes to Sail's pipe, not the terminal, so it would print
        // no colour; ask for it as plain Cargo-Verus would on this terminal.
        // An argument, not CARGO_TERM_COLOR, so it is not a proof input.
        .args((out.is_some() && terminal_colour(&config.args[..split])).then_some("--color=always"))
        .args(&config.args[split..])
        .env("RUSTC_BOOTSTRAP", "1")
        .env("CARGO", &paths.cargo_alias)
        .env("SAIL_MANAGED_CARGO", &config.cargo)
        .env(
            crate::adapter::CARGO_METADATA_ENV,
            crate::adapter::metadata_target(&config.cache)?,
        )
        .env("SAIL_MANAGED_PROXY", &paths.proxy_alias)
        .env("SAIL_MANAGED_RUNTIME", &paths.runtime)
        .env("SAIL_MANAGED_ARTIFACTS", paths.run.join("artifacts.jsonl"))
        .env("VERUS_Z3_PATH", &config.shim)
        .env("SAIL_Z3_PATH", &config.z3)
        .env("SAIL_SOLVER_MODE", "default")
        .env_remove(super::shipped::PLAN_ENV)
        .env_remove(super::shipped::SOURCE_BASE_ENV)
        .env_remove("SAIL_ENGINE")
        .env_remove("SAIL_ENGINE_FUNCTION")
        .env_remove("SAIL_ENGINE_EVENTS")
        .env_remove("SAIL_ENGINE_FOCUS_REPORT")
        .env_remove("SAIL_ENGINE_CONTEXT")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    if let Some(socket) = &config.socket {
        cmd.env("SAIL_DAEMON_SOCKET", socket);
    } else {
        cmd.env_remove("SAIL_DAEMON_SOCKET");
    }
    if let Some(plan) = &paths.shipped_plan {
        cmd.env(super::shipped::PLAN_ENV, plan);
    }
    if let Some(base) = &paths.source_base {
        cmd.env(super::shipped::SOURCE_BASE_ENV, base);
    }
    #[cfg(unix)]
    let visible = super::visible_output::Receiver::bind()
        .map_err(|error| Error::new("SAIL3106", error.to_string()))?;
    cmd.env_remove(super::visible_output::ENV);
    #[cfg(unix)]
    cmd.env(super::visible_output::ENV, visible.path());
    let mut child = cmd
        .spawn()
        .map_err(|e| Error::new("SAIL3106", e.to_string()))?;
    let (stdout, captured) = match relay(
        &mut child,
        out,
        err,
        seeded,
        #[cfg(unix)]
        Some(visible),
    ) {
        Ok(output) => output,
        Err(error) => {
            // A closed stdout or stderr (`| head`) ends the run: stop Cargo.
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let code = child
        .wait()
        .map_err(|e| Error::new("SAIL3106", e.to_string()))?
        .code();
    std::fs::write(paths.run.join("cargo.stdout"), stdout)
        .and_then(|()| std::fs::write(paths.run.join("cargo.stderr"), captured))
        .map_err(|e| Error::new("SAIL3106", e.to_string()))?;
    Ok(code)
}

/// Whether Sail's stderr is a terminal on which plain Cargo would print
/// colour, and the user chose no colour setting of their own.
fn terminal_colour(cargo_args: &[String]) -> bool {
    use std::io::IsTerminal as _;
    colour_wanted(
        std::io::stderr().is_terminal(),
        std::env::var_os("CARGO_TERM_COLOR").is_some(),
        cargo_args,
    )
}

fn colour_wanted(terminal: bool, colour_env: bool, cargo_args: &[String]) -> bool {
    terminal
        && !colour_env
        && !cargo_args
            .iter()
            .any(|a| a == "--color" || a.starts_with("--color="))
}

/// One original Cargo-Verus pipe record.
enum Output {
    Line(Vec<u8>),
    Diagnostics(Vec<u8>),
    Closed,
}

#[cfg(unix)]
#[path = "output_readers.rs"]
mod output_readers;

/// Preserve Cargo's original diagnostic rendering and stdout records. Only a
/// fresh completion marker is removed, after earlier same-pipe records have
/// been forwarded or intentionally suppressed for a structured caller. The ACK
/// then allows that driver to exit; it does not order independent Cargo jobs.
#[allow(
    clippy::too_many_lines,
    reason = "Original reader setup, completion handling and unconditional owned cleanup stay together."
)]
fn relay(
    child: &mut std::process::Child,
    out: &mut Option<&mut dyn Write>,
    err: &mut dyn Write,
    seeded: &[super::store::Seeded],
    #[cfg(unix)] visible: Option<super::visible_output::Receiver>,
) -> Result<(Vec<u8>, Vec<u8>)> {
    #[cfg(not(unix))]
    use std::io::BufRead as _;
    let failed = |error: std::io::Error| Error::new("SAIL3106", error.to_string());
    let (tx, rx) = std::sync::mpsc::channel::<std::io::Result<Output>>();
    #[cfg(unix)]
    let readers = output_readers::Readers::start(child, &tx).map_err(failed)?;
    #[cfg(not(unix))]
    let readers = [
        child.stdout.take().map(|pipe| {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let mut reader = std::io::BufReader::new(pipe);
                loop {
                    let mut line = Vec::new();
                    match reader.read_until(b'\n', &mut line) {
                        Ok(0) => break,
                        Ok(_) => {
                            if tx.send(Ok(Output::Line(line))).is_err() {
                                break;
                            }
                        }
                        Err(error) => {
                            let _ = tx.send(Err(error));
                            break;
                        }
                    }
                }
                let _ = tx.send(Ok(Output::Closed));
            })
        }),
        child.stderr.take().map(|mut pipe| {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let mut buffer = vec![0; 65536];
                loop {
                    match pipe.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(count) => {
                            if tx
                                .send(Ok(Output::Diagnostics(buffer[..count].to_vec())))
                                .is_err()
                            {
                                break;
                            }
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                        Err(error) => {
                            let _ = tx.send(Err(error));
                            break;
                        }
                    }
                }
                let _ = tx.send(Ok(Output::Closed));
            })
        }),
    ];
    drop(tx);
    let result = (|| {
        #[cfg(not(unix))]
        if readers.iter().any(Option::is_none) {
            return Err(Error::new("SAIL3106", "missing Cargo output pipe"));
        }
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        let mut emitted = BTreeSet::new();
        let mut closed = 0;
        while closed != 2 {
            let piece = rx
                .recv()
                .map_err(|_| Error::new("SAIL3106", "Cargo output reader disconnected"))?;
            match piece.map_err(failed)? {
                Output::Closed => closed += 1,
                Output::Diagnostics(bytes) => {
                    err.write_all(&bytes)
                        .and_then(|()| err.flush())
                        .map_err(failed)?;
                    stderr.extend_from_slice(&bytes);
                }
                Output::Line(line) => {
                    #[cfg(unix)]
                    let decoded = visible
                        .as_ref()
                        .map(|receiver| receiver.decode(&line))
                        .transpose()
                        .map_err(failed)?;
                    #[cfg(unix)]
                    let original = decoded
                        .as_ref()
                        .map_or(line.as_slice(), |decoded| decoded.bytes.as_slice());
                    #[cfg(not(unix))]
                    let original = line.as_slice();
                    relay_line(original, out, seeded, &mut emitted, &mut stdout)?;
                    #[cfg(unix)]
                    if let Some(completion) = decoded.and_then(|decoded| decoded.completion) {
                        completion.acknowledge().map_err(failed)?;
                    }
                }
            }
        }
        Ok((stdout, stderr))
    })();
    drop(rx);
    // Reject waiting senders before stopping Cargo or joining its readers.
    #[cfg(unix)]
    let cleanup = visible.map_or(Ok(()), super::visible_output::Receiver::finish);
    // Readers stop independently of EOF; invoke retains Cargo kill/wait ownership.
    #[cfg(unix)]
    let joined = readers.finish().map_err(failed);
    // Preserve the previous non-Unix failure path; only Unix readers are cancellable.
    #[cfg(not(unix))]
    if result.is_err() {
        return result;
    }
    #[cfg(not(unix))]
    let joined = {
        let mut result = Ok(());
        for reader in readers.into_iter().flatten() {
            if reader.join().is_err() {
                result = Err(Error::new("SAIL3106", "Cargo output reader failed"));
            }
        }
        result
    };
    let output = result?;
    #[cfg(unix)]
    cleanup.map_err(failed)?;
    joined?;
    Ok(output)
}

/// Process the exact original Cargo record after removing only its private marker.
fn relay_line(
    line: &[u8],
    out: &mut Option<&mut dyn Write>,
    seeded: &[super::store::Seeded],
    emitted: &mut BTreeSet<usize>,
    stdout: &mut Vec<u8>,
) -> Result<()> {
    let failed = |error: std::io::Error| Error::new("SAIL3106", error.to_string());
    if let Ok(artifact) = serde_json::from_slice::<Value>(line)
        && let Some(bytes) = seeded_stdout(&artifact, seeded, emitted)
    {
        relay_stdout(bytes, out).map_err(failed)?;
        stdout.extend_from_slice(bytes);
    }
    if !cargo_message(line) {
        relay_stdout(line, out).map_err(failed)?;
    }
    stdout.extend_from_slice(line);
    Ok(())
}

fn relay_stdout(bytes: &[u8], out: &mut Option<&mut dyn Write>) -> std::io::Result<()> {
    if let Some(out) = out {
        out.write_all(bytes).and_then(|()| out.flush())?;
    }
    Ok(())
}

#[cfg(all(test, unix))]
#[path = "visible_output_tests.rs"]
mod visible_output_tests;

/// Restores a seeded root's exact output at Cargo's fresh-artifact boundary,
/// before a dependent target's output. Existing targets and rebuilt units do
/// not replay; each newly seeded root can emit at most once.
fn seeded_stdout<'a>(
    artifact: &Value,
    seeded: &'a [super::store::Seeded],
    emitted: &mut BTreeSet<usize>,
) -> Option<&'a [u8]> {
    if artifact["reason"] != "compiler-artifact" || artifact["fresh"] != true {
        return None;
    }
    let name = artifact["target"]["name"].as_str()?.replace('-', "_");
    let kind = artifact["target"]["kind"].as_array()?;
    let (index, seed) = seeded.iter().enumerate().find(|(index, seed)| {
        seed.placed
            && !emitted.contains(index)
            && artifact["package_id"].as_str() == Some(seed.receipt.package_id.as_str())
            && name == seed.receipt.invocation.identity.name
            && kind.len() == seed.receipt.kind.len()
            && seed.receipt.kind.iter().all(|kind| {
                artifact["target"]["kind"].as_array().is_some_and(|kinds| {
                    kinds
                        .iter()
                        .any(|candidate| candidate.as_str() == Some(kind))
                })
            })
    })?;
    emitted.insert(index);
    Some(&seed.shown_stdout)
}

/// Whether a stdout line is one of Cargo's JSON messages (an object with a
/// `reason`), rather than text a compiler printed.
fn cargo_message(line: &[u8]) -> bool {
    serde_json::from_slice::<Value>(line)
        .is_ok_and(|value| value.get("reason").is_some_and(Value::is_string))
}

pub(crate) fn focused(args: &[String]) -> bool {
    args.iter().skip_while(|a| a.as_str() != "--").any(|s| {
        [
            "--verify-module",
            "--verify-only-module",
            "--verify-function",
            "--verify-root",
        ]
        .iter()
        .any(|flag| s == flag || s.starts_with(&format!("{flag}=")))
    })
}

fn invocations(paths: &Paths) -> Vec<Invocation> {
    std::fs::read_dir(paths.run.join("invocations"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| read(&e.path()))
        .collect()
}

/// A complete negative full run can retain ordinary dependency contracts, but
/// neither Cargo's 101 nor Currentness alone establishes that completion.
/// Stock and source-engine runs both qualify; only the completion evidence
/// differs (the engine's sealed terminal, or stock's normal exit and whole
/// document). Without this, a failed proof consumed the generation and the next
/// run cleaned every build-script package, re-verifying vstd.
///
/// A run whose only failure is a compile error in a selected crate qualifies
/// too ([`confined_compile_failure`]).
///
/// A run that established build-script declarations (`script_coverage` false:
/// a project's first managed run, or any run after one that withdrew them)
/// qualifies only with Cargo's own completion evidence as well: Cargo finished
/// its build ([`cargo_build_failed`]) and ran, in this run, every build script
/// whose contract is kept. Currentness already requires every scheduled
/// script to hold a covered contract after the run. Without this, a project
/// whose first runs failed re-verified vstd on every run until one passed
/// (issue #123), although a passing establishing run keeps the contracts.
fn negative_compiler_generation(
    paths: &Paths,
    data: &Value,
    expected: &BTreeSet<(String, String, String)>,
    result: &ProjectResult,
    script_coverage: bool,
    engine: bool,
    args: &[String],
) -> bool {
    if focused(args)
        || result.currentness != Currentness::Current
        || result.exit_code != Some(101)
        || result.error.is_some()
        || expected.is_empty()
    {
        return false;
    }
    let established = || {
        cargo_build_failed(&paths.run.join("cargo.stdout"))
            && super::build_inputs::executed(
                &paths.build_inputs,
                &paths.run.join("artifacts.jsonl"),
            )
    };
    if !(script_coverage || established()) {
        return false;
    }
    let check = || -> Option<bool> {
        let mut calls = BTreeMap::new();
        let mut starts = Vec::new();
        for entry in std::fs::read_dir(paths.run.join("invocations")).ok()? {
            let path = entry.ok()?.path();
            match path.extension()?.to_str()? {
                "json" => {
                    let call: Invocation = read(&path)?;
                    let key = call.identity.key();
                    if path.file_stem()?.to_str()? != key
                        || call.run_id != result.run_id
                        || calls.insert(key, call).is_some()
                    {
                        return Some(false);
                    }
                }
                "started" => {
                    let start: Value = read(&path)?;
                    if start["run_id"] != result.run_id {
                        return Some(false);
                    }
                    starts.push(start["target"].as_str()?.to_owned());
                }
                _ => return Some(false),
            }
        }
        // A call replayed from the shipped dependency build (ADR 0048) starts
        // no verifier process.
        let mut names: Vec<_> = calls
            .values()
            .filter(|call| call.dependency_build.is_none())
            .map(|call| call.identity.name.clone())
            .collect();
        names.sort();
        starts.sort();
        if names.is_empty() || names != starts {
            return Some(false);
        }
        let completed =
            |call: &Invocation| crate::engine::completed_compiler_output(&paths.run, call, engine);
        let proof_failure = calls.values().any(|call| call.report.exit_code == Some(1))
            && calls.values().all(completed);
        if !proof_failure
            && !confined_compile_failure(paths, data, expected, calls.values(), completed)
        {
            return Some(false);
        }
        Some(expected.iter().all(|(id, name, kind)| {
            calls
                .values()
                .filter(|call| selected_call(data, (id, name, kind), call))
                .count()
                == 1
        }))
    };
    check() == Some(true)
}

/// Whether `call` is the compilation of the selected target `(id, name, kind)`.
fn selected_call(data: &Value, (id, name, kind): (&str, &str, &str), call: &Invocation) -> bool {
    data["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|package| {
            package["id"] == *id
                && package["manifest_path"]
                    .as_str()
                    .is_some_and(|manifest| Path::new(manifest) == call.identity.manifest)
                && name.replace('-', "_") == call.identity.name
                && (kind == call.identity.crate_type
                    || (kind == "lib" && call.identity.crate_type == "rlib"))
        })
}

/// A run that failed only because a selected crate did not compile, as
/// Cargo's own records show:
///
/// - every recorded verifier start has its completion (checked by the caller),
///   and every expected target has exactly one, so Cargo started each selected
///   crate. Cargo starts a unit only after every unit it depends on has
///   completed, so each build script and dependency of the selection finished;
/// - every dependency verified in this run completed successfully, and every
///   selected crate either completed its verification or stopped with a
///   normal compiler error exit (1) and no Sail error;
/// - Cargo finished its build (one `build-finished` record, unsuccessful), so
///   no unit was interrupted; and
/// - every build-script contract kept for this context was executed by Cargo
///   in this run (`build-script-executed`, fresh or not).
///
/// The failing crate gets no receipt: its compilation did not succeed. Only
/// the dependencies' ordinary build-script contracts are retained, and the
/// next run revalidates each of them independently before any use.
fn confined_compile_failure<'a>(
    paths: &Paths,
    data: &Value,
    expected: &BTreeSet<(String, String, String)>,
    calls: impl Iterator<Item = &'a Invocation> + Clone,
    completed: impl Fn(&Invocation) -> bool,
) -> bool {
    let selected = |call: &Invocation| {
        expected
            .iter()
            .any(|(id, name, kind)| selected_call(data, (id, name, kind), call))
    };
    let compile_error = |call: &Invocation| {
        call.report.exit_code == Some(1)
            && !call.success
            && call.report.error.is_none()
            && call.report.invariant_violations().is_empty()
    };
    let mut failed = false;
    for call in calls {
        if selected(call) {
            if !completed(call) {
                if !compile_error(call) {
                    return false;
                }
                failed = true;
            }
        } else if !(call.success && completed(call)) {
            return false;
        }
    }
    failed
        && cargo_build_failed(&paths.run.join("cargo.stdout"))
        && super::build_inputs::executed(&paths.build_inputs, &paths.run.join("artifacts.jsonl"))
}

/// Cargo's JSON output ends one build, which failed: exactly one
/// `build-finished` record, with `success: false`.
fn cargo_build_failed(stdout: &Path) -> bool {
    let Ok(bytes) = std::fs::read(stdout) else {
        return false;
    };
    let finished: Vec<Value> = bytes
        .split(|b| *b == b'\n')
        .filter_map(|line| serde_json::from_slice::<Value>(line).ok())
        .filter(|value| value["reason"] == "build-finished")
        .collect();
    matches!(finished.as_slice(), [only] if only["success"] == false)
}

struct CompilerGeneration<'a> {
    compiler: &'a str,
    script_coverage: bool,
    source_engine: bool,
    args: &'a [String],
}

fn publish_compiler_generation(
    paths: &Paths,
    data: &Value,
    expected: &BTreeSet<(String, String, String)>,
    result: &ProjectResult,
    generation: &CompilerGeneration<'_>,
) {
    if (result.currentness == Currentness::Current && result.exit_code == Some(0))
        || negative_compiler_generation(
            paths,
            data,
            expected,
            result,
            generation.script_coverage,
            generation.source_engine,
            generation.args,
        )
    {
        super::build_inputs::family::publish(
            &paths.compiler_inputs,
            generation.compiler,
            &paths.target,
            &paths.build_inputs,
            data,
        );
    }
}

fn artifacts(paths: &Paths) -> Vec<Value> {
    std::fs::read_to_string(paths.run.join("artifacts.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|v| v["reason"] == "compiler-artifact")
        .collect()
}

fn receipt_path(paths: &Paths, package: &str, name: &str, kind: &[String]) -> PathBuf {
    paths.receipts.join(format!(
        "{}.json",
        digest(&serde_json::to_vec(&(package, name, kind)).unwrap_or_default())
    ))
}

/// The receipt directories of the other proof contexts in this compiler
/// family, in a stable order. Each receipt still binds its own context.
fn sibling_receipts(paths: &Paths, context: &str) -> Vec<(String, PathBuf)> {
    let mut siblings: Vec<(String, PathBuf)> = std::fs::read_dir(&paths.family_contexts)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name != context && !name.starts_with('.'))
        .map(|name| {
            let receipts = paths.projects.join(&name).join("receipts");
            (name, receipts)
        })
        .collect();
    siblings.sort();
    siblings
}

/// The revision fields decide a targeted clean; the other small header fields
/// can only suppress optional restoration. Skipping the embedded report avoids
/// constructing megabytes for vstd; reuse still checks the complete receipt.
#[derive(serde::Deserialize)]
struct ReceiptRevision {
    package_id: String,
    input_revision: String,
    // These optional hint fields never authorize reuse. Keeping their raw JSON
    // type preserves the existing dirty scan even for malformed headers.
    #[serde(default)]
    version: Value,
    #[serde(default)]
    context: Value,
    #[serde(default)]
    name: Value,
    #[serde(default)]
    kind: Value,
}

/// Only suppress optional content restoration; complete receipt validation is
/// still performed after Cargo. The caller has already freshly compared every
/// decoded revision and must retain the plan after any cleanup or uncertainty.
fn receipt_covered_targets(
    paths: &Paths,
    expected: &BTreeSet<(String, String, String)>,
    context: &str,
    revisions: &BTreeMap<PathBuf, ReceiptRevision>,
) -> bool {
    !expected.is_empty()
        && expected.iter().all(|(package, name, kind)| {
            // Multi-kind targets conservatively keep the optional plan.
            let kinds = vec![kind.clone()];
            revisions
                .get(&receipt_path(paths, package, name, &kinds))
                .is_some_and(|receipt| {
                    receipt.package_id == *package
                        && !receipt.input_revision.is_empty()
                        && receipt.version.as_u64() == Some(u64::from(RECEIPT_VERSION))
                        && receipt.context.as_str() == Some(context)
                        && receipt.name.as_str() == Some(name)
                        && receipt.kind == serde_json::json!(kinds)
                })
        })
}

fn dirty_receipt_packages(
    revisions: &BTreeMap<PathBuf, ReceiptRevision>,
    data: &Value,
) -> Vec<String> {
    // Each current revision is its own complete snapshot; they overlap.
    let receipts: Vec<&ReceiptRevision> = revisions.values().collect();
    let current = crate::parallel::in_order(&receipts, |r| target_revision(data, &r.package_id));
    receipts
        .into_iter()
        .zip(current)
        .filter(|(r, current)| r.input_revision != *current)
        .map(|(r, _)| r.package_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[allow(
    clippy::too_many_lines,
    reason = "Receipt matching is kept alongside the observed Cargo target that authorizes reuse."
)]
fn target_records(
    paths: &Paths,
    data: &Value,
    context: &str,
    force: bool,
    expected: &BTreeSet<(String, String, String)>,
    seeded: &[super::store::Seeded],
) -> Result<(Vec<TargetResult>, BTreeSet<String>)> {
    let mut results = Vec::new();
    let mut missing = BTreeSet::new();
    let calls = invocations(paths);
    let siblings = sibling_receipts(paths, context);
    // Every unit's declared variables, as Cargo fingerprints them; their
    // current values bind each receipt this run publishes.
    let declared = super::environment::values(&super::environment::declared_names(&paths.target));
    let artifacts = artifacts(paths);
    let verified = |artifact: &Value| {
        let package = data["packages"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|p| p["id"] == artifact["package_id"])?;
        (package
            .pointer("/metadata/verus/verify")
            .and_then(Value::as_bool)
            == Some(true)
            && !artifact["target"]["kind"]
                .as_array()
                .is_some_and(|k| k.iter().any(|v| v == "custom-build")))
        .then_some(package)
    };
    // Each stored receipt is read once, and every file that the receipts and
    // the produced artifacts bind is read in one overlapping batch: a
    // dependency's artifacts are also each dependent's inputs.
    let digests = PhaseDigests::default();
    let mut stored: HashMap<PathBuf, Option<Receipt>> = HashMap::new();
    let mut batch: Vec<PathBuf> = Vec::new();
    for artifact in &artifacts {
        if verified(artifact).is_none() {
            continue;
        }
        let produced: Vec<PathBuf> = artifact["filenames"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(PathBuf::from)
            .collect();
        batch.extend(produced.iter().map(|p| p.with_extension("vir")));
        batch.extend(produced);
        let path = receipt_path(
            paths,
            artifact["package_id"].as_str().unwrap_or_default(),
            artifact["target"]["name"].as_str().unwrap_or_default(),
            &artifact["target"]["kind"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>(),
        );
        let receipt = read::<Receipt>(&path);
        if let Some(receipt) = &receipt {
            batch.extend(receipt.artifact_hashes.keys().cloned());
            batch.extend(receipt.invocation.dependencies.keys().cloned());
        }
        stored.insert(path, receipt);
    }
    // Each verified package's current revision is its own complete snapshot.
    // They are taken while the batch is read, in the same phase.
    let ids: Vec<String> = artifacts
        .iter()
        .filter(|artifact| verified(artifact).is_some())
        .filter_map(|artifact| artifact["package_id"].as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(str::to_owned)
        .collect();
    let revisions: HashMap<String, String> = std::thread::scope(|scope| {
        let current =
            scope.spawn(|| crate::parallel::in_order(&ids, |id| target_revision(data, id)));
        digests.prefetch(
            batch
                .iter()
                .filter(|path| path.is_file())
                .map(PathBuf::as_path),
        );
        current.join().ok()
    })
    .map(|current| ids.into_iter().zip(current).collect())
    .unwrap_or_default();
    for artifact in artifacts {
        let Some(package) = verified(&artifact) else {
            continue;
        };
        let name = artifact["target"]["name"].as_str().unwrap_or_default();
        let kind: Vec<String> = artifact["target"]["kind"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
        let package_id = artifact["package_id"].as_str().unwrap_or_default();
        let requested = expected.iter().any(|(id, target, target_kind)| {
            id == package_id
                && target.replace('-', "_") == name.replace('-', "_")
                && kind.contains(target_kind)
        });
        let force = force && requested;
        let input_revision = revisions
            .get(package_id)
            .cloned()
            .unwrap_or_else(|| target_revision(data, package_id));
        let path = receipt_path(paths, package_id, name, &kind);
        let call = calls.iter().find(|c| {
            c.identity.manifest == Path::new(package["manifest_path"].as_str().unwrap_or_default())
                && c.identity.name == name.replace('-', "_")
                && kind.iter().any(|k| {
                    k == &c.identity.crate_type || (k == "lib" && c.identity.crate_type == "rlib")
                })
        });
        if let Some(call) = call {
            let mut artifact_paths: Vec<PathBuf> = artifact["filenames"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(PathBuf::from)
                .collect();
            artifact_paths.extend(
                artifact_paths
                    .clone()
                    .into_iter()
                    .map(|p| p.with_extension("vir"))
                    .filter(|p| p.is_file()),
            );
            let hashes = digests.hashes(&artifact_paths)?;
            let reusable =
                paths.reuse && call.success && complete(&call.report) && !input_revision.is_empty();
            if reusable {
                json(
                    &paths
                        .run
                        .join("pending-receipts")
                        .join(path.file_name().unwrap_or_default()),
                    &Receipt {
                        version: RECEIPT_VERSION,
                        committed: paths.run.join("committed.json"),
                        checksum: String::new(),
                        package_id: package_id.to_owned(),
                        name: name.to_owned(),
                        kind: kind.clone(),
                        context: context.to_owned(),
                        input_revision: input_revision.clone(),
                        invocation: call.clone(),
                        artifact_hashes: hashes,
                        declared_environment: declared.clone(),
                    }
                    .seal(),
                )?;
            } else {
                let _ = std::fs::remove_file(&path);
            }
            results.push(TargetResult {
                dependency_store: None,
                dependency_build: call.dependency_build.clone(),
                package_id: package_id.to_owned(),
                target: name.to_owned(),
                kind,
                establishing_run: call.establishing_run().to_owned(),
                reused: call.retained_from.is_some(),
                requested,
                complete: call.success && complete(&call.report),
                report: call.report.clone(),
            });
        } else if let Some((receipt, store_key)) = {
            let mut observed: BTreeSet<PathBuf> = artifact["filenames"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(PathBuf::from)
                .collect();
            observed.extend(
                observed
                    .clone()
                    .into_iter()
                    .map(|p| p.with_extension("vir"))
                    .filter(|p| p.is_file()),
            );
            let matches = |r: &Receipt, context: &str| {
                paths.reuse
                    && artifact["fresh"] == true
                    && !force
                    && !input_revision.is_empty()
                    && r.input_revision == input_revision
                    && r.package_id == package_id
                    && r.invocation.identity.manifest
                        == Path::new(package["manifest_path"].as_str().unwrap_or_default())
                    && r.invocation.identity.name == name.replace('-', "_")
                    && kind.iter().any(|k| {
                        k == &r.invocation.identity.crate_type
                            || (k == "lib" && r.invocation.identity.crate_type == "rlib")
                    })
                    && r.name == name
                    && r.kind == kind
                    && r.artifact_hashes.keys().cloned().collect::<BTreeSet<_>>() == observed
                    && r.valid_in(context, &digests)
            };
            stored
                .remove(&path)
                .unwrap_or_else(|| read::<Receipt>(&path))
                .filter(|r| matches(r, context))
                .or_else(|| {
                    // A dependency's full verification does not depend on
                    // which function a repair selects, so a sibling context
                    // of this compiler family may have established exactly
                    // these artifact bytes. The selected targets always need
                    // a receipt of their own proof context.
                    if requested {
                        return None;
                    }
                    let file = path.file_name()?;
                    siblings.iter().find_map(|(sibling, receipts)| {
                        read::<Receipt>(&receipts.join(file)).filter(|r| matches(r, sibling))
                    })
                })
                .map(|r| (r, None))
                .or_else(|| {
                    // A dependency this run placed from the dependency store
                    // (ADR 0045): its stored receipt, moved into this target,
                    // must describe exactly the bytes Cargo found fresh here.
                    if requested {
                        return None;
                    }
                    seeded.iter().find_map(|seed| {
                        let r = &seed.receipt;
                        (artifact["fresh"] == true
                            && r.package_id == package_id
                            && r.invocation.identity.manifest
                                == Path::new(package["manifest_path"].as_str().unwrap_or_default())
                            && r.invocation.identity.name == name.replace('-', "_")
                            && r.name == name
                            && r.kind == kind
                            && r.artifact_hashes.keys().cloned().collect::<BTreeSet<_>>()
                                == observed
                            && digests.matches(&r.artifact_hashes)
                            && digests.matches(&r.invocation.dependencies)
                            && super::environment::unchanged(&r.declared_environment))
                        .then(|| (r.clone(), seed.placed.then(|| seed.key.clone())))
                    })
                })
        } {
            event(
                &serde_json::json!({"event":"target-reused","target":name,"establishing_run":receipt.invocation.establishing_run()}),
            );
            results.push(TargetResult {
                dependency_store: store_key,
                dependency_build: receipt.invocation.dependency_build.clone(),
                package_id: package_id.to_owned(),
                target: name.to_owned(),
                kind,
                establishing_run: receipt.invocation.establishing_run().to_owned(),
                reused: true,
                requested,
                complete: true,
                report: receipt.invocation.report,
            });
        } else {
            missing.insert(package["name"].as_str().unwrap_or_default().to_owned());
        }
    }
    // A failing compiler may not emit an artifact; preserve its observed target report.
    for call in calls {
        if !results.iter().any(|r| {
            r.establishing_run == call.establishing_run()
                && r.target.replace('-', "_") == call.identity.name
                && r.kind.iter().any(|k| {
                    k == &call.identity.crate_type
                        || (k == "lib" && call.identity.crate_type == "rlib")
                })
        }) {
            let package = data["packages"].as_array().into_iter().flatten().find(|p| {
                p["manifest_path"]
                    .as_str()
                    .is_some_and(|s| Path::new(s) == call.identity.manifest)
            });
            let requested = package.is_some_and(|p| {
                expected.iter().any(|(id, name, kind)| {
                    p["id"].as_str() == Some(id.as_str())
                        && name.replace('-', "_") == call.identity.name
                        && (kind == &call.identity.crate_type
                            || (kind == "lib" && call.identity.crate_type == "rlib"))
                })
            });
            results.push(TargetResult {
                dependency_store: None,
                dependency_build: call.dependency_build.clone(),
                package_id: package
                    .and_then(|p| p["id"].as_str())
                    .unwrap_or_default()
                    .to_owned(),
                establishing_run: call.establishing_run().to_owned(),
                reused: call.retained_from.is_some(),
                target: call.identity.name,
                kind: vec![call.identity.crate_type],
                requested,
                complete: call.success && complete(&call.report),
                report: call.report,
            });
        }
    }
    results.sort_by(|a, b| {
        (&a.package_id, &a.target, &a.kind).cmp(&(&b.package_id, &b.target, &b.kind))
    });
    results
        .dedup_by(|a, b| a.package_id == b.package_id && a.target == b.target && a.kind == b.kind);
    Ok((results, missing))
}

/// Package ids reachable from `roots` through every resolved dependency kind.
pub(super) fn dependency_closure<'a>(
    data: &Value,
    roots: impl IntoIterator<Item = &'a str>,
) -> BTreeSet<String> {
    let mut closure: BTreeSet<String> = roots.into_iter().map(str::to_owned).collect();
    let mut pending: Vec<String> = closure.iter().cloned().collect();
    while let Some(id) = pending.pop() {
        if let Some(node) = data["resolve"]["nodes"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|n| n["id"].as_str() == Some(id.as_str()))
        {
            for dependency in node["dependencies"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                if closure.insert(dependency.to_owned()) {
                    pending.push(dependency.to_owned());
                }
            }
        }
    }
    closure
}

/// The packages Cargo can compile for the selected targets, or `None` when
/// the selection is not known exactly (then every resolved package counts).
fn selected_closure(
    data: &Value,
    expected: &BTreeSet<(String, String, String)>,
) -> Option<BTreeSet<String>> {
    (!expected.is_empty())
        .then(|| dependency_closure(data, expected.iter().map(|(id, _, _)| id.as_str())))
}

/// Whether every unit Cargo reported in this run belongs to `closure`. An
/// unparsable record or a unit outside it refuses proof-time coverage.
fn artifacts_within(paths: &Paths, closure: Option<&BTreeSet<String>>) -> bool {
    let Some(closure) = closure else {
        return true;
    };
    std::fs::read_to_string(paths.run.join("artifacts.jsonl")).is_ok_and(|text| {
        text.lines()
            .filter(|line| !line.trim().is_empty())
            .all(|line| {
                serde_json::from_str::<Value>(line).is_ok_and(|record| {
                    let unit = record["reason"] == "compiler-artifact"
                        || record["reason"] == "build-script-executed";
                    !unit
                        || record["package_id"]
                            .as_str()
                            .is_some_and(|id| closure.contains(id))
                })
            })
    })
}

fn target_revision(data: &Value, package_id: &str) -> String {
    let closure = dependency_closure(data, [package_id]);
    // Only the fields the snapshot reads: copying the whole metadata (several
    // MB on vest) for each target cost more than the snapshot itself.
    let mut package_data = serde_json::Map::new();
    for field in SNAPSHOT_FIELDS {
        if let Some(value) = data.get(field) {
            package_data.insert(field.to_owned(), value.clone());
        }
    }
    let mut package_data = Value::Object(package_data);
    // Every closure package is content-bound, registry and git ones included:
    // local edits or corruption of a fetched crate cannot silently retain an
    // old proof.
    let packages: Vec<Value> = data["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["id"].as_str().is_some_and(|id| closure.contains(id)))
        .map(|p| serde_json::json!({ "manifest_path": p["manifest_path"] }))
        .collect();
    package_data["packages"] = Value::Array(packages);
    if let Some(observed) = data["sail_source_inputs_by_package"].as_object() {
        package_data["sail_source_inputs"] = Value::Array(
            closure
                .iter()
                .filter_map(|id| observed.get(id).and_then(Value::as_array))
                .flatten()
                .cloned()
                .collect(),
        );
    }
    // A dependency whose whole closure is registry or git code (vstd) is
    // bound to what the workspace decides for it, its resolved closure and
    // the `[profile]` table, instead of the bytes of the workspace manifest
    // and lockfile, so adding an unrelated dependency keeps its receipt, as
    // Cargo keeps it fresh (#177). The toolchain files and Cargo
    // configuration stay bound by their bytes.
    let foreign = super::store::foreign_closure_binding(data, &closure);
    let mut extras = Vec::new();
    if foreign.is_some() {
        let workspace = Path::new(data["workspace_root"].as_str().unwrap_or_default());
        extras.extend(
            ["rust-toolchain", "rust-toolchain.toml"]
                .iter()
                .map(|name| workspace.join(name))
                .filter(|path| path.is_file()),
        );
        let root = data["packages"].as_array().and_then(|packages| {
            packages
                .iter()
                .find(|p| p["id"].as_str() == Some(package_id))?["manifest_path"]
                .as_str()
                .and_then(|manifest| Path::new(manifest).parent())
                .map(Path::to_owned)
        });
        let Some(root) = root else {
            return String::new();
        };
        // The snapshot adds the manifest and lockfile of its workspace root;
        // the package's own directory has neither outside its own files.
        package_data["workspace_root"] = Value::String(root.to_string_lossy().into_owned());
        if package_data["sail_invocation_directory"].is_null() {
            package_data["sail_invocation_directory"] =
                Value::String(workspace.to_string_lossy().into_owned());
        }
    }
    let snapshot = snapshot_metadata(&package_data, &extras);
    if !snapshot.complete {
        return String::new();
    }
    match foreign {
        Some(binding) => digest(
            &serde_json::to_vec(&("sail-foreign-target-v1", &snapshot.revision, &binding))
                .unwrap_or_default(),
        ),
        None => snapshot.revision,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "All supported Cargo target selectors are resolved explicitly before complete coverage can be claimed."
)]
fn expected_targets(data: &Value, args: &[String]) -> BTreeSet<(String, String, String)> {
    let workspace = args.iter().any(|a| a == "--workspace" || a == "--all");
    let mut selected_names = Vec::new();
    let mut excluded = Vec::new();
    let mut bins = Vec::new();
    let mut lib = false;
    let mut all_bins = false;
    let mut it = args.iter().take_while(|a| a.as_str() != "--");
    while let Some(a) = it.next() {
        match a.as_str() {
            "-p" | "--package" => {
                if let Some(v) = it.next() {
                    selected_names.push(v.as_str());
                }
            }
            "--exclude" => {
                if let Some(v) = it.next() {
                    excluded.push(v.as_str());
                }
            }
            "--bin" => {
                if let Some(v) = it.next() {
                    bins.push(v.as_str());
                }
            }
            "--lib" => lib = true,
            "--bins" => all_bins = true,
            _ => {
                if let Some(v) = a.strip_prefix("--package=") {
                    selected_names.push(v);
                } else if let Some(v) = a.strip_prefix("-p") {
                    selected_names.push(v.strip_prefix('=').unwrap_or(v));
                }
                if let Some(v) = a.strip_prefix("--exclude=") {
                    excluded.push(v);
                }
                if let Some(v) = a.strip_prefix("--bin=") {
                    bins.push(v);
                }
            }
        }
    }
    let default = data["resolve"]["root"].as_str();
    let mut expected = BTreeSet::new();
    for package in data["packages"].as_array().into_iter().flatten() {
        let id = package["id"].as_str().unwrap_or_default();
        let name = package["name"].as_str().unwrap_or_default();
        let member = data["workspace_members"]
            .as_array()
            .is_some_and(|m| m.contains(&package["id"]));
        let chosen = if !selected_names.is_empty() {
            selected_names.iter().any(|n| {
                *n == name
                    || *n == id
                    || n.split_once('@').is_some_and(|(selected, version)| {
                        selected == name && package["version"].as_str() == Some(version)
                    })
            })
        } else if workspace {
            member
        } else if let Some(root) = default {
            id == root
        } else {
            data["workspace_default_members"]
                .as_array()
                .is_some_and(|m| m.contains(&package["id"]))
        };
        if !chosen
            || excluded.contains(&name)
            || package
                .pointer("/metadata/verus/verify")
                .and_then(Value::as_bool)
                != Some(true)
        {
            continue;
        }
        let features = data["resolve"]["nodes"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|p| p["id"] == package["id"])
            .and_then(|p| p["features"].as_array());
        for target in package["targets"].as_array().into_iter().flatten() {
            let kinds = target["kind"].as_array();
            let bin = kinds.is_some_and(|k| k.iter().any(|v| v == "bin"));
            let library = kinds.is_some_and(|k| {
                k.iter().any(|v| {
                    matches!(
                        v.as_str(),
                        Some("lib" | "rlib" | "dylib" | "cdylib" | "staticlib" | "proc-macro")
                    )
                })
            });
            let name = target["name"].as_str().unwrap_or_default();
            let selected_kind = if lib || all_bins || !bins.is_empty() {
                (lib && library) || (bin && (all_bins || bins.contains(&name)))
            } else {
                library || bin
            };
            if !selected_kind {
                continue;
            }
            if target["required-features"]
                .as_array()
                .is_some_and(|required| {
                    required
                        .iter()
                        .any(|f| features.is_none_or(|enabled| !enabled.contains(f)))
                })
            {
                continue;
            }
            expected.insert((
                id.to_owned(),
                name.to_owned(),
                kinds
                    .and_then(|k| k.first())
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ));
        }
    }
    expected
}

/// Rechecks a completed result's recorded inputs without starting Cargo or
/// Verus (see `read_check`).
///
/// # Errors
/// Returns `SAIL3108` when the context has no record of a Current run.
pub fn refresh_result(
    cache: &Path,
    result: &ProjectResult,
    environment: &BTreeMap<String, String>,
) -> Result<super::input::InputSnapshot> {
    refresh_context(
        cache,
        Path::new(&result.workspace),
        &result.context_sha256,
        environment,
    )
}

/// Rechecks a context's latest Current record without invoking Cargo. The
/// environment is bound by the context identity itself.
///
/// # Errors
/// Returns `SAIL3108` until a run of this context has ended Current.
pub fn refresh_context(
    cache: &Path,
    workspace: &Path,
    context_sha256: &str,
    _environment: &BTreeMap<String, String>,
) -> Result<super::input::InputSnapshot> {
    super::read_check::check(cache, workspace, context_sha256)
}

/// Whether every runtime file of `context` still has its recorded identity
/// (stat for installed files, content for configuration), checked in parallel.
fn runtime_current(cache: &Path, context: &str) -> bool {
    let state = cache.join("runtime-identities");
    let identity_sha = read::<String>(&state.join(format!("{context}.link")));
    let identities = identity_sha
        .as_ref()
        .and_then(|sha| read::<Vec<(String, String)>>(&state.join(format!("{sha}.json"))));
    identities.as_ref().is_some_and(|rows| {
        identity_sha
            .as_ref()
            .is_some_and(|sha| *sha == digest(&serde_json::to_vec(rows).unwrap_or_default()))
            && crate::parallel::in_order(rows, |(path, expected)| {
                super::files::identity_matches(Path::new(path), expected)
            })
            .into_iter()
            .all(|matches| matches)
    })
}

fn script_roots(data: &Value) -> Vec<PathBuf> {
    data["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| {
            p["targets"].as_array().is_some_and(|t| {
                t.iter().any(|v| {
                    v["kind"]
                        .as_array()
                        .is_some_and(|k| k.iter().any(|k| k == "custom-build"))
                })
            })
        })
        .filter_map(|p| {
            p["manifest_path"]
                .as_str()
                .and_then(|s| Path::new(s).parent())
                .map(Path::to_owned)
        })
        .collect()
}

/// Invokes Cargo and returns current or historical per-target verification reports.
///
/// # Errors
/// Stable `SAIL310x` errors identify setup, metadata, locking, or observation failures.
#[allow(
    clippy::too_many_lines,
    reason = "The top-level lifecycle keeps locking and input validation visibly scoped together."
)]
pub fn run(config: &Config, err: &mut dyn Write) -> Result<ProjectResult> {
    run_engine(config, err, None)
}

/// [`run`], also showing Verus's own stdout on `out` as plain
/// `cargo verus verify` would print it. `sail verify` uses this; JSON callers
/// use [`run`], whose stdout is theirs.
///
/// # Errors
///
/// As [`run`].
pub fn run_showing_output(
    config: &Config,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<ProjectResult> {
    run_engine_held(config, Some(out), err, None).map(|(result, _)| result)
}

pub(crate) fn run_engine(
    config: &Config,
    err: &mut dyn Write,
    engine: Option<crate::engine::Request>,
) -> Result<ProjectResult> {
    run_engine_held(config, None, err, engine).map(|(result, _)| result)
}

/// [`run_engine`], also returning the shared workspace lock, so the caller can
/// read the run's records before `sail cache projects prune` may remove them.
#[allow(
    clippy::too_many_lines,
    reason = "The complete proof lifecycle and final input sealing remain together."
)]
pub(crate) fn run_engine_held(
    config: &Config,
    mut out: Option<&mut dyn Write>,
    err: &mut dyn Write,
    engine: Option<crate::engine::Request>,
) -> Result<(ProjectResult, File)> {
    let research = super::research::selected()?;
    let mut owned = config.clone();
    owned.workspace =
        std::path::absolute(&owned.workspace).map_err(|e| Error::new("SAIL3100", e.to_string()))?;
    owned.cache = owned.workspace.join(&owned.cache);
    if let Some(engine) = &engine {
        owned.cargo_verus.clone_from(&engine.cargo_verus);
    }
    let config = &owned;
    // Cargo options that change what Cargo prints (its message format and
    // verbose command lines, which would show Sail's driver and flags) are
    // also left to plain Cargo-Verus.
    let cargo_output = config
        .args
        .iter()
        .take_while(|a| a.as_str() != "--")
        .find(|a| {
            a.starts_with("--message-format") || matches!(a.as_str(), "-v" | "-vv" | "--verbose")
        });
    if let Some(unbound) = cargo_output.or_else(|| {
        config.args.iter().find(|s| {
            let s = s.as_str();
            [
                "--tests",
                "--benches",
                "--examples",
                "--all-targets",
                "--test",
                "--bench",
                "--example",
                "--no-verify",
            ]
            .iter()
            .any(|flag| s == *flag || s.starts_with(&format!("{flag}=")))
                || s == "--target-dir"
                || s.starts_with("--target-dir=")
                || s == "--no-verify"
                || s.starts_with("--log")
                || s == "--verus-source"
                || s == "--sysroot"
                || s.starts_with("--sysroot=")
        })
    }) {
        // SAIL3112: a setup managed verification cannot bind to a receipt.
        // `sail verify` then verifies it as plain Cargo-Verus does (ADR 0046).
        return Err(Error::new(
            "SAIL3112",
            format!("`{unbound}` is not bound to a proof receipt by managed verification"),
        ));
    }
    let started = Instant::now();
    let diagnostics = Diagnostics::new();
    let invocation = std::path::absolute(&config.workspace)
        .map_err(|e| Error::new("SAIL3100", e.to_string()))?;
    let loader = super::configuration::check(&invocation, &config.args)?;
    if let Some(variable) = &loader {
        let _ = writeln!(
            err,
            "note[SAIL3112]: {variable} is set, so this run reuses no proof receipts and records none; it verifies what Cargo rebuilds, as `cargo verus verify` does (unset {variable} to reuse proofs)"
        );
    }
    // The input snapshot before the run is taken while Cargo reads the
    // metadata, from the package list and extra inputs the previous run of
    // this invocation selected, and kept only if this run selects exactly the
    // same ones: then it is the snapshot this run's selection reads. The hint
    // authorizes nothing; any difference takes the snapshot again.
    let hint_path = super::input::hint_path(&config.cache, &invocation, &config.args);
    let speculative_before = {
        let hint_path = hint_path.clone();
        std::thread::Builder::new()
            .spawn(move || {
                let hint = read::<super::input::SnapshotHint>(&hint_path)?;
                let (snapshot, stamp) =
                    super::input::snapshot_metadata_stamped(&hint.projection, &hint.extras);
                Some((hint, snapshot, stamp))
            })
            .ok()
    };
    // Cargo metadata and the runtime context are independent: neither reads
    // the other's result. They overlap, and their results are consumed in the
    // original order, so the first reported error is unchanged.
    let (data, context_result, metadata_phase) = std::thread::scope(|scope| {
        let pending = scope.spawn(|| {
            metadata(
                &config.workspace,
                &config.args,
                &config.cargo,
                &config.cargo_toolchain,
                &config.cache,
            )
        });
        let context_phase = diagnostics.phase(Phase::RuntimeContext);
        let context_result = context(config, engine.as_ref());
        drop(context_phase);
        let metadata_phase = diagnostics.phase(Phase::Metadata);
        let data = pending
            .join()
            .unwrap_or_else(|_| Err(Error::new("SAIL3103", "Cargo metadata reader failed")));
        (data, context_result, metadata_phase)
    });
    let mut data = data?;
    // An excluded cache subtree cannot also contain source inputs. Refuse
    // before prepare(), dependency seeding or any proof receipt is inspected.
    super::configuration::check_cache_inputs(&data, &config.cache)?;
    data["sail_invocation_directory"] = Value::String(invocation.to_string_lossy().into_owned());
    data["sail_cache_directory"] = Value::String(config.cache.to_string_lossy().into_owned());
    let workspace_root = PathBuf::from(data["workspace_root"].as_str().unwrap_or_default());
    if std::env::var_os("RUSTC_WORKSPACE_WRAPPER").is_some() {
        return Err(Error::new(
            "SAIL3112",
            "RUSTC_WORKSPACE_WRAPPER is set, and managed verification cannot chain a workspace rustc wrapper",
        ));
    }
    let expected = expected_targets(&data, &config.args);
    if expected.is_empty() {
        return Err(Error::new(
            "SAIL3112",
            "no selected Cargo target declares package.metadata.verus.verify = true, so there is no proof to record",
        ));
    }
    drop(metadata_phase);
    let Context {
        hash,
        compiler_hash,
        store_hash,
    } = context_result?;

    let run_id = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    );
    let inputs = diagnostics.phase(Phase::Inputs);
    let (mut paths, ownership) = prepare(config, &workspace_root, &hash, &compiler_hash, &run_id)?;
    paths.reuse = loader.is_none();
    // A Cargo target this run creates (a new project, worktree or sandbox, or
    // after a prune) may start from the machine's dependency store (ADR 0045).
    // Focused runs build in a separate directory and verify no dependency. A
    // run without proof reuse neither reads nor fills the store (ADR 0046).
    let store_candidates = if paths.reuse && super::store::enabled() && !focused(&config.args) {
        super::store::candidates(&data, &expected, &store_hash)
    } else {
        Vec::new()
    };
    let new_target = super::store::new_target(&paths.target);
    // Sail's Verus names the shipped build's registry sources by this
    // machine's registry directory, in every managed run (ADR 0048): a target
    // or store entry may hold shipped units from an earlier run.
    let registry = super::shipped::registry_dir(&data);
    paths.source_base = registry
        .as_deref()
        .filter(|_| research != super::research::Mode::RetainedContent)
        .filter(|_| {
            super::shipped::research_enabled()
                || super::shipped::has_adjacent_bundle(&config.cargo_verus)
        })
        .filter(|_| {
            !super::shipped::research_enabled()
                || std::env::var_os(super::shipped::LOCAL_ENV).is_none()
        })
        .filter(|_| super::shipped::portable_context(&config.workspace, &config.args))
        .and_then(|registry| {
            Some(super::shipped::source_base(
                &super::shipped::virtual_base()?,
                registry,
            ))
        });
    let shipped_plan = if focused(&config.args)
        || engine.is_some()
        || !paths.reuse
        || research == super::research::Mode::RetainedContent
    {
        None
    } else {
        super::shipped::plan(
            &config.cargo_verus,
            &paths.target,
            registry.as_deref(),
            new_target,
            &config.z3,
            &config.shim,
            &run_id,
            &config.workspace,
            &config.args,
            &data,
        )
    };
    if let Some(plan) = &shipped_plan {
        let file = paths.run.join("shipped.json");
        json(&file, plan)?;
        paths.shipped_plan = Some(file);
    }
    super::build_inputs::family::begin(
        &paths.compiler_inputs,
        &compiler_hash,
        &paths.target,
        &paths.build_inputs,
        &data,
    )?;
    let source_state = paths.build_inputs.with_file_name("source-inputs.json");
    let (sources, source_coverage) = super::source_inputs::load(&source_state);
    data["sail_source_inputs"] = serde_json::to_value(&sources).unwrap_or_default();
    data["sail_source_inputs_by_package"] =
        serde_json::to_value(super::source_inputs::packages(&source_state)).unwrap_or_default();
    let (stale_scripts, mut extras, script_coverage) =
        super::build_inputs::stale(&paths.build_inputs, &data);
    let script_roots = script_roots(&data);
    extras.extend(script_roots.clone());
    let selection = super::input::SnapshotHint {
        version: super::input::SNAPSHOT_HINT_VERSION,
        projection: snapshot_projection(&data, &data["sail_source_inputs"]),
        extras: extras.clone(),
    };
    let mut before = match speculative_before
        .and_then(|thread| thread.join().ok())
        .flatten()
    {
        // Cargo may have written inputs while it read the metadata (a lockfile
        // update, extracted registry sources): the snapshot must be unchanged.
        Some((hint, snapshot, stamp))
            if hint == selection && super::input::unchanged_since(&snapshot, stamp.as_ref()) =>
        {
            snapshot
        }
        _ => {
            // A hint is only ever a hint: failing to record one changes nothing.
            let _ = std::fs::create_dir_all(hint_path.parent().unwrap_or(&config.cache))
                .map_err(|e| Error::new("SAIL3102", e.to_string()))
                .and_then(|()| json(&hint_path, &selection));
            snapshot_metadata(&data, &extras)
        }
    };
    before.complete &= source_coverage;
    before.roots.extend(super::input::configuration_paths(
        &invocation,
        std::env::var_os("CARGO_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")))
            .as_deref(),
    ));
    // Generated build-script files are compared by content after the run,
    // not across it: this run's own cleaning and scripts rewrite them (#120).
    // A new target does not exist yet: canonicalize its owned parent.
    let generated = paths.target.canonicalize().ok().or_else(|| {
        Some(
            paths
                .target
                .parent()?
                .canonicalize()
                .ok()?
                .join(paths.target.file_name()?),
        )
    });
    let before_run = without_generated(&before, generated.as_deref());
    let schedule_path = paths
        .build_inputs
        .with_file_name("unscheduled-scripts.json");
    let schedule_before = super::build_inputs::family::schedule::Scope {
        data: &data,
        snapshot: &before_run,
        context: &hash,
        compiler: &compiler_hash,
    };
    let skipped_scripts = if paths.reuse {
        super::build_inputs::family::schedule::load(&schedule_path, &schedule_before)
    } else {
        BTreeSet::new()
    };
    let (stale_scripts, script_coverage) = if skipped_scripts.is_empty() {
        (stale_scripts, script_coverage)
    } else {
        // Only unobserved scripts may be exempted. Every recorded script's
        // declared paths/environment and the late family recheck still run.
        let (stale, _, coverage) =
            super::build_inputs::stale_after(&paths.build_inputs, &data, &skipped_scripts);
        (stale, coverage)
    };
    // Sail's targeted cleans exist to observe what a receipt binds. A run
    // without reuse records no receipt, so Cargo's own freshness decides, as
    // for plain Cargo-Verus.
    if paths.reuse && !stale_scripts.is_empty() {
        clean(config, &paths.target, &stale_scripts)?;
    }
    let absent = super::absent::Observer::begin(&before_run, &paths.run.join("observer"));
    // Cargo must compile nothing outside the selected targets' dependency
    // closure; the read-time record covers exactly that closure. Build-script
    // coverage likewise treats scripts outside an explicit selection's closure
    // as never scheduled; the same unit check binds it.
    let scope = selected_closure(&data, &expected);
    let script_scope = super::build_inputs::selection_closure(&data);
    let units_in_scope = |paths: &Paths| {
        artifacts_within(paths, scope.as_ref()) && artifacts_within(paths, script_scope.as_ref())
    };
    let mut outside_scope = false;
    // Imported observations were first checked before guard acquisition. Check
    // them again under the original live owners before using shared outputs.
    // Cleaning a newly changed script prevents learning new bytes beside output
    // generated from the older bytes, including restored-mtime changes.
    let late_scripts =
        super::build_inputs::family::recheck(&paths.build_inputs, &data, &paths.target)?;
    let late_scripts: Vec<_> = late_scripts
        .into_iter()
        .filter(|name| !stale_scripts.contains(name))
        .collect();
    if paths.reuse {
        clean(config, &paths.target, &late_scripts)?;
    }
    let stamp_before = super::input::mutation_stamp(&before_run);
    if super::configuration::check(&invocation, &config.args)? != loader {
        return Err(Error::new(
            "SAIL3112",
            "the process environment changed during verification setup",
        ));
    }
    super::files::json(&paths.input_state, &data)?;
    event(&serde_json::json!({
        "kind":"context-ready", "workspace":before.workspace, "run_id":run_id,
        "targets":expected.iter().map(|(package,name,kind)| serde_json::json!({"package_id":package,"name":name,"kind":[kind]})).collect::<Vec<_>>(),
        "context_sha256":hash, "input_revision":before.revision,
        "complete":before.complete && script_coverage && absent.ready(),
        "source_observation":source_state.is_file()
    }));
    drop(inputs);
    // The solver's provenance digest is a complete read that overlaps target
    // cleanup; the final runtime check still reads the solver again.
    let solver_digest = {
        let z3 = config.z3.clone();
        std::thread::spawn(move || file_digest(&z3))
    };
    let cleanup = diagnostics.phase(Phase::TargetCleanup);
    let forced = config.force
        || std::env::var("SAIL_MANAGED_FORCE").as_deref() == Ok("1")
        || focused(&config.args)
        || !before.complete
        || !absent.ready();
    let names: BTreeSet<String> = data["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| {
            data["workspace_members"]
                .as_array()
                .is_some_and(|members| members.contains(&p["id"]))
                && p.pointer("/metadata/verus/verify").and_then(Value::as_bool) == Some(true)
        })
        .filter_map(|p| p["name"].as_str().map(str::to_owned))
        .collect();
    if forced && paths.reuse {
        clean(config, &paths.target, &selected(&config.args, &names))?;
    }
    let receipt_revisions: BTreeMap<PathBuf, ReceiptRevision> = std::fs::read_dir(&paths.receipts)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| Some((entry.path(), read::<ReceiptRevision>(&entry.path())?)))
        .collect();
    let dirty = dirty_receipt_packages(&receipt_revisions, &data);
    if paths.reuse && !dirty.is_empty() {
        clean(config, &paths.target, &dirty)?;
    }
    drop(cleanup);
    let source_engine = engine.is_some();
    // A header may only avoid optional restoration work. Unexpected Cargo
    // rebuilds still run the genuine verifier, and the ordinary complete
    // receipt/artifact/currentness checks below remain the reuse authority.
    // Reuse this phase's fresh revision scan instead of hashing inputs again.
    let receipt_covered = !new_target
        && script_coverage
        && stamp_before.is_some()
        && stale_scripts.is_empty()
        && late_scripts.is_empty()
        && dirty.is_empty()
        && receipt_covered_targets(&paths, &expected, &hash, &receipt_revisions);
    let mut retained_notes = Vec::new();
    let runtime = Runtime {
        run_id: run_id.clone(),
        args: config.args.clone(),
        z3: config.z3.clone(),
        z3_sha256: solver_digest
            .join()
            .unwrap_or_else(|_| file_digest(&config.z3))?,
        daemon: config.socket.is_some(),
        sidecars: paths.run.join("sidecars"),
        invocations: paths.run.join("invocations"),
        original_wrapper: actual_wrapper(&config.cargo_verus)?,
        receipts: paths.receipts.clone(),
        context_sha256: hash.clone(),
        input_revision: before.revision.clone(),
        packages: data["packages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| {
                Some((
                    PathBuf::from(p["manifest_path"].as_str()?),
                    p["id"].as_str()?.to_owned(),
                ))
            })
            .collect(),
        target_kinds: data["packages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|package| {
                Some((
                    PathBuf::from(package["manifest_path"].as_str()?),
                    package["targets"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|target| {
                            Some((
                                target["name"].as_str()?.replace('-', "_"),
                                target["kind"]
                                    .as_array()?
                                    .iter()
                                    .filter_map(|kind| kind.as_str().map(str::to_owned))
                                    .collect(),
                            ))
                        })
                        .collect(),
                ))
            })
            .collect(),
        // Retention is optional restoration work: a target whose compile is
        // cheap next to retention compiles normally instead (#156).
        retained: if !forced
            && !source_engine
            && paths.reuse
            && !receipt_covered
            && super::retained::cost::pays(
                &paths.receipts,
                selected_targets(&data, &expected)
                    .iter()
                    .map(|(manifest, name)| (manifest.as_path(), name.as_str())),
            ) {
            super::retained::Plan::prepare(
                &data,
                &paths.target,
                &expected,
                &actual_wrapper(&config.cargo_verus)?,
                &config.z3,
                &run_id,
                &mut retained_notes,
            )
        } else {
            None
        },
        attribution_logs: config.attribution_logs || engine.is_some(),
        engine,
    };
    json(&paths.runtime, &runtime)?;
    let invocation_phase = diagnostics.phase(Phase::Invocation);
    // After every cleanup, so nothing seeded is cleaned again before Cargo
    // decides its freshness.
    let seeded = if new_target {
        super::store::seed(&config.cache, &paths.target, &store_candidates, &data, err)
    } else {
        // Only a dependency without a receipt of this context needs the store.
        let own: BTreeSet<String> = std::fs::read_dir(&paths.receipts)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| read::<Receipt>(&e.path()))
            .map(|r| r.package_id)
            .collect();
        let unowned: Vec<_> = store_candidates
            .iter()
            .filter(|c| !own.contains(&c.root))
            .cloned()
            .collect();
        super::store::attach(&config.cache, &paths.target, &unowned, &data)
    };
    let mut exit = invoke(config, &paths, &mut out, err, &seeded)?;
    drop(invocation_phase);
    if let Some(plan) = &shipped_plan {
        super::shipped::finish_record(plan, &config.cargo_verus, &workspace_root)?;
    }
    let records_phase = diagnostics.phase(Phase::TargetRecords);
    outside_scope |= !units_in_scope(&paths);
    // The inputs after the run: Cargo metadata again, the build-script and
    // source-input records this run's artifacts update, and the complete
    // content snapshot they select.
    let after_inputs = |after_metadata: Option<Result<Value>>| -> Result<AfterInputs> {
        super::build_inputs::update(
            &paths.build_inputs,
            &paths.run.join("artifacts.jsonl"),
            &data,
        )?;
        let (sources, source_coverage) =
            super::source_inputs::update(&source_state, &paths.run.join("artifacts.jsonl"), &data)?;
        // Coverage after the run needs a contract for every script Cargo ran.
        let unscheduled = super::build_inputs::unscheduled(
            &paths.run.join("cargo.stdout"),
            &paths.run.join("artifacts.jsonl"),
            &data,
        )
        .unwrap_or_default();
        let (_, mut after_extras, after_coverage) =
            super::build_inputs::stale_after(&paths.build_inputs, &data, &unscheduled);
        after_extras.extend(script_roots.iter().cloned());
        let sources = serde_json::to_value(sources).unwrap_or_default();
        // Without metadata in hand, the snapshot is taken while Cargo reads it,
        // from the package list before the run, and kept only if the metadata
        // after the run lists exactly the same packages: then it is the
        // snapshot that list selects. Otherwise it is taken again.
        let (after_data, speculative) = match after_metadata {
            Some(after) => (after, None),
            None => std::thread::scope(|threads| {
                let snapshot = threads.spawn(|| {
                    super::input::snapshot_metadata_stamped(
                        &snapshot_projection(&data, &sources),
                        &after_extras,
                    )
                });
                let after = metadata(
                    &config.workspace,
                    &config.args,
                    &config.cargo,
                    &config.cargo_toolchain,
                    &config.cache,
                );
                (after, snapshot.join().ok())
            }),
        };
        let mut after_data = after_data?;
        after_data["sail_cache_directory"] = data["sail_cache_directory"].clone();
        after_data["sail_invocation_directory"] = data["sail_invocation_directory"].clone();
        after_data["sail_source_inputs"] = sources;
        after_data["sail_source_inputs_by_package"] =
            serde_json::to_value(super::source_inputs::packages(&source_state)).unwrap_or_default();
        super::files::json(&paths.input_state, &after_data)?;
        // Kept only if nothing it observed changed while Cargo read the
        // metadata (which may rewrite the lockfile).
        let mut after = match speculative {
            Some((snapshot, stamp))
                if same_snapshot_selection(&data, &after_data)
                    && super::input::unchanged_since(&snapshot, stamp.as_ref()) =>
            {
                snapshot
            }
            _ => snapshot_metadata(&after_data, &after_extras),
        };
        after.roots.extend(super::input::configuration_paths(
            &invocation,
            std::env::var_os("CARGO_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")))
                .as_deref(),
        ));
        Ok(AfterInputs {
            data: after_data,
            snapshot: after,
            unscheduled,
            after_coverage,
            source_coverage,
        })
    };
    // A target Cargo found up to date without a matching receipt (pruned
    // receipts, an interrupted run, an unknown earlier result) gets no report
    // unless Cargo runs again. A run that shows Verus's output does not retry:
    // plain Cargo-Verus prints one up-to-date run, and so does Sail. The next
    // JSON, job or MCP run re-establishes the receipt. Without reuse there is
    // no retry either. The shown run's result, which is not printed, simply
    // lacks those targets.
    let shown = out.is_some();
    let retry_possible = paths.reuse && !shown && exit == Some(0);
    // The post-run Cargo metadata starts once the verifier invocation has
    // ended and overlaps the receipt checks, which never read it. When no
    // retry can follow, the rest of the inputs after the run overlap them
    // too: neither reads what the other writes, and the stamp that brackets
    // every content read of both is taken once both have finished. A retry
    // discards the metadata and reads everything again after its own
    // invocation.
    let (records, after_thread) = std::thread::scope(|threads| {
        let after = threads.spawn(|| {
            if retry_possible {
                (
                    Some(metadata(
                        &config.workspace,
                        &config.args,
                        &config.cargo,
                        &config.cargo_toolchain,
                        &config.cache,
                    )),
                    None,
                )
            } else {
                (None, Some(after_inputs(None)))
            }
        });
        let records = target_records(&paths, &data, &hash, forced, &expected, &seeded);
        (records, after.join())
    });
    let reader_failed = || Error::new("SAIL3103", "Cargo metadata reader failed");
    let (mut after_metadata, overlapped) = after_thread.unwrap_or_else(|_| {
        (
            Some(Err(reader_failed())),
            (!retry_possible).then(|| Err(reader_failed())),
        )
    });
    let (mut targets, missing) = records?;
    if retry_possible && !missing.is_empty() {
        after_metadata = None;
        let retry_packages = missing.into_iter().collect::<Vec<_>>();
        clean(config, &paths.target, &retry_packages)?;
        std::fs::write(paths.run.join("artifacts.jsonl"), [])
            .map_err(|e| Error::new("SAIL3106", e.to_string()))?;
        std::fs::remove_dir_all(paths.run.join("pending-receipts"))
            .and_then(|()| std::fs::create_dir_all(paths.run.join("pending-receipts")))
            .map_err(|e| Error::new("SAIL3107", e.to_string()))?;
        exit = invoke(config, &paths, &mut out, err, &[])?;
        outside_scope |= !units_in_scope(&paths);
        let (retried, missing) = target_records(&paths, &data, &hash, forced, &expected, &seeded)?;
        targets = retried;
        if !missing.is_empty() {
            return Err(Error::new(
                "SAIL3107",
                "selected Cargo targets did not produce proof receipts after a fresh verification",
            ));
        }
    }
    drop(records_phase);
    let post_run = diagnostics.phase(Phase::PostRun);
    let AfterInputs {
        data: after_data,
        snapshot: after,
        unscheduled,
        after_coverage,
        source_coverage,
    } = match overlapped {
        Some(inputs) => inputs?,
        None => after_inputs(after_metadata)?,
    };
    // Resolve fresh receipt bindings while the input stamp still brackets every
    // content read. A newly discovered include may already belong to another
    // observed package, so global currentness alone does not imply that the
    // pre-invocation target revision contained that particular include.
    let pending = completed_receipts(&paths.run, &after_data)?;
    let after_run = without_generated(&after, generated.as_deref());
    let schedule_after = super::build_inputs::family::schedule::Scope {
        data: &after_data,
        snapshot: &after_run,
        context: &hash,
        compiler: &compiler_hash,
    };
    let scheduling_confirmed = super::build_inputs::family::schedule::confirmed(
        &skipped_scripts,
        &schedule_before,
        &schedule_after,
        &unscheduled,
    );
    if !scheduling_confirmed {
        // Withhold Current now, and restore original cleanup next time even
        // if an unexpectedly scheduled unit left the same input bytes.
        let _ = std::fs::remove_file(&schedule_path);
    }
    let stamp_after = super::input::mutation_stamp(&after_run);
    let absent = absent.finish();
    let currentness = if !paths.reuse
        || !before.complete
        || !after.complete
        || !after_coverage
        || !source_coverage
        || !absent.complete
        || outside_scope
        || !scheduling_confirmed
        || !runtime_current(&config.cache, &hash)
        || stamp_before.is_none()
        || stamp_after.is_none()
    {
        Currentness::Unknown
    } else if before_run.digests == after_run.digests
        && stamp_before == stamp_after
        && !absent.changed
    {
        Currentness::Current
    } else {
        Currentness::Historical
    };
    drop(post_run);
    if paths.reuse
        && !shown
        && exit == Some(0)
        && expected.iter().any(|(id, name, kind)| {
            !targets.iter().any(|t| {
                &t.package_id == id
                    && t.target.replace('-', "_") == name.replace('-', "_")
                    && t.kind.contains(kind)
            })
        })
    {
        return Err(Error::new(
            "SAIL3107",
            "not every selected verification target produced a complete observation",
        ));
    }
    let verifier_invocations = u64::try_from(
        std::fs::read_dir(paths.run.join("invocations"))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|s| s == "started"))
            .count(),
    )
    .unwrap_or(u64::MAX);
    let reused_targets =
        u64::try_from(targets.iter().filter(|t| t.reused).count()).unwrap_or(u64::MAX);
    let mut notes = retained_notes;
    // Only a release with a shipped vstd proof has a fast path to offer.
    if super::shipped::has_adjacent_bundle(&config.cargo_verus) {
        notes.extend(config.cargo_toolchain.note());
    }
    if let Some(variable) = &loader {
        notes.push(format!(
            "SAIL3112: {variable} is set, so this run reused and recorded no proof receipts; targets Cargo found up to date have no report"
        ));
    }
    if currentness == Currentness::Unknown {
        notes.push(INPUTS_UNCONFIRMED.to_owned());
    }
    // A script package Cargo did not schedule (#121) establishes nothing.
    if !script_coverage
        && stale_scripts.iter().any(|name| {
            data["packages"].as_array().into_iter().flatten().any(|p| {
                p["name"].as_str() == Some(name.as_str())
                    && p["id"].as_str().is_none_or(|id| !unscheduled.contains(id))
            })
        })
    {
        notes.push("Build-script input declarations were established during this run".to_owned());
    }
    if focused(&config.args) {
        notes.push(
            "Requested function/module scope is partial; whole-target reuse is disabled".to_owned(),
        );
    }
    for target in targets.iter().filter(|t| t.dependency_build.is_some()) {
        notes.push(format!(
            "SAIL3132: {} replays its complete dependency proof established by {}",
            target.package_id, target.establishing_run
        ));
    }
    for target in targets.iter().filter(|t| t.dependency_store.is_some()) {
        notes.push(format!(
            "SAIL3130: {} was verified by run {} in another workspace and reused from Sail's dependency store",
            target.package_id, target.establishing_run
        ));
    }
    let fallbacks = engine_fallbacks(&targets);
    if fallbacks > 0 {
        notes.push(format!(
            "SAIL3123: the interactive engine faulted for {fallbacks} target(s); their exact stock Verus results are reported and publish no proof receipt"
        ));
    }
    let result = ProjectResult {
        kind: "sail-project-result".to_owned(),
        schema_version: SCHEMA_VERSION,
        run_id,
        workspace: before.workspace.to_string_lossy().into_owned(),
        context_sha256: hash,
        input_revision: before.revision,
        currentness,
        exit_code: exit,
        targets,
        execution: Execution {
            verifier_invocations,
            reused_targets,
            wall_ms: started.elapsed().as_secs_f64() * 1000.0,
        },
        notes,
        error: None,
        engine: runtime
            .engine
            .as_ref()
            .map(crate::engine::Request::identity),
        no_reuse: loader.as_ref().map(|variable| NoReuse {
            code: "SAIL3112".to_owned(),
            reason: format!("{variable} is set"),
            verified_as: VerifiedAs::Managed,
        }),
    };
    let publication = diagnostics.phase(Phase::Publication);
    if result.currentness == Currentness::Current
        && let Some(context) = paths.input_state.parent()
    {
        // What a later read must find unchanged: this run's final digests of
        // the selection's dependency closure and every configuration path.
        let configuration = super::input::configuration_paths(
            &invocation,
            std::env::var_os("CARGO_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")))
                .as_deref(),
        );
        super::read_check::record(
            context,
            &result.input_revision,
            super::read_check::entries(&after, &after_data, scope.as_ref(), &configuration),
        )?;
    }
    json(&paths.run.join("result.json"), &result)?;
    if result.currentness == Currentness::Current {
        if exit == Some(0) {
            super::build_inputs::family::schedule::publish(
                &schedule_path,
                &super::build_inputs::family::schedule::Scope {
                    data: &after_data,
                    snapshot: &after_run,
                    context: &result.context_sha256,
                    compiler: &compiler_hash,
                },
                &result.run_id,
                result.currentness,
                result.exit_code,
                &unscheduled,
            );
        }
        for (path, receipt) in pending {
            json(&path, &receipt)?;
        }
        super::receipts::publish(&paths.run, &paths.receipts, &result.context_sha256)?;
        // Dependencies reused through the store become this context's own
        // receipts, with every ordinary receipt check on later runs.
        for target in result.targets.iter().filter(|t| t.reused) {
            let Some(seed) = seeded.iter().find(|s| {
                s.receipt.package_id == target.package_id
                    && s.receipt.invocation.establishing_run() == target.establishing_run
            }) else {
                continue;
            };
            let input_revision = target_revision(&after_data, &target.package_id);
            let path = receipt_path(&paths, &target.package_id, &target.target, &target.kind);
            if input_revision.is_empty() {
                continue;
            }
            // A store problem never changes a run's result: without the
            // adopted receipt, the next run checks the store entry again.
            if let Err(error) = super::receipts::adopt(
                &paths.run,
                &paths.receipts,
                path.file_name().unwrap_or_default(),
                &result.context_sha256,
                input_revision,
                seed.receipt.clone(),
            ) {
                event(
                    &serde_json::json!({"event":"dependency-adopt-failed","package_id":target.package_id,"error":error.message}),
                );
            }
        }
        store_dependencies(config, &paths, &store_candidates, &result, &after_data);
        if result.exit_code == Some(0)
            && let Some(plan) = &runtime.retained
        {
            let _ = plan.publish(&runtime);
        }
    }
    publish_compiler_generation(
        &paths,
        &after_data,
        &expected,
        &result,
        &CompilerGeneration {
            compiler: &compiler_hash,
            script_coverage,
            source_engine,
            args: &config.args,
        },
    );
    drop(publication);
    // Freeing the metadata and snapshots (tens of thousands of allocations on
    // vest, about 15 ms) does not hold up the caller; nothing reads them.
    let _ = std::thread::Builder::new().spawn(move || {
        drop((data, after_data, after, before_run, after_run));
        drop((before.digests, before.observation_paths));
    });
    Ok((result, ownership.into_workspace()))
}

/// The metadata [`snapshot_metadata`] reads from `data`, with `sources` as
/// the observed source inputs: exactly what the snapshot of metadata that
/// lists the same packages, with those sources, reads.
fn snapshot_projection(data: &Value, sources: &Value) -> Value {
    let mut projection = serde_json::Map::new();
    for field in SNAPSHOT_FIELDS {
        if let Some(value) = data.get(field) {
            projection.insert(field.to_owned(), value.clone());
        }
    }
    projection.insert("sail_source_inputs".to_owned(), sources.clone());
    projection.insert(
        "packages".to_owned(),
        Value::Array(
            data["packages"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|p| serde_json::json!({ "manifest_path": p["manifest_path"] }))
                .collect(),
        ),
    );
    Value::Object(projection)
}

/// Whether two metadata documents select the same snapshot, given the same
/// invocation and cache directories and source inputs: the same workspace
/// root, target directory and package manifests, in the same order.
fn same_snapshot_selection(before: &Value, after: &Value) -> bool {
    let manifests = |data: &Value| -> Vec<Value> {
        data["packages"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|p| p["manifest_path"].clone())
            .collect()
    };
    before["workspace_root"] == after["workspace_root"]
        && before["target_directory"] == after["target_directory"]
        && manifests(before) == manifests(after)
}

/// The inputs observed after a run's invocation (see `run_engine_held`).
struct AfterInputs {
    data: Value,
    snapshot: super::input::InputSnapshot,
    unscheduled: BTreeSet<String>,
    after_coverage: bool,
    source_coverage: bool,
}

/// Offers each store candidate this Current run verified, or reused from its
/// own proof context, to the dependency store (ADR 0045). Never fails the run.
fn store_dependencies(
    config: &Config,
    paths: &Paths,
    candidates: &[super::store::Candidate],
    result: &ProjectResult,
    data: &Value,
) {
    for candidate in candidates {
        // A held key is never stored again; checking first spares re-reading
        // the receipt's artifacts (vstd's are about 40 MB) on every run.
        if super::store::holds(&config.cache, candidate) {
            continue;
        }
        let Some(target) = result.targets.iter().find(|t| {
            t.package_id == candidate.root
                && t.complete
                && t.dependency_store.is_none()
                && t.kind.iter().any(|k| k == "lib")
        }) else {
            continue;
        };
        let Some(receipt) = read::<Receipt>(&receipt_path(
            paths,
            &target.package_id,
            &target.target,
            &target.kind,
        ))
        .filter(|r| r.valid(&result.context_sha256)) else {
            continue;
        };
        let sidecar = paths
            .projects
            .join(&receipt.context)
            .join("runs")
            .join(&receipt.invocation.run_id)
            .join("sidecars")
            .join(receipt.invocation.identity.key());
        super::store::publish(
            candidate,
            &super::store::Publication {
                cache: &config.cache,
                target: &paths.target,
                artifacts: &paths.run.join("artifacts.jsonl"),
                build_inputs: &paths.build_inputs,
                receipt,
                sidecar,
                data,
            },
        );
    }
}

/// The part of `snapshot` that must stay unchanged across a run: everything
/// but the files inside the managed Cargo target (`generated`, canonical).
///
/// Compiler-declared inputs there are build-script products such as serde's
/// `OUT_DIR/private.rs`. The target is exclusively owned by this run, which
/// cleans and reruns scripts, so their stamps change while their scripts'
/// own inputs (observed outside the target) do not. Rustc reads them only
/// after their script has finished, so their content after the run is the
/// content compiled. That content stays bound: the receipts' target revisions
/// and the read-time record are taken from the complete after snapshot.
/// Without a canonical target nothing is left out.
fn without_generated(
    snapshot: &super::input::InputSnapshot,
    generated: Option<&Path>,
) -> super::input::InputSnapshot {
    let mut view = snapshot.clone();
    if let Some(target) = generated {
        view.observation_paths
            .retain(|path| !path.starts_with(target));
        view.roots.retain(|path| !path.starts_with(target));
        view.digests.retain(|path, _| !path.starts_with(target));
    }
    view
}

/// Targets established in this run by the stock fallback after an engine fault.
fn engine_fallbacks(targets: &[TargetResult]) -> usize {
    targets
        .iter()
        .filter(|target| {
            !target.reused
                && target.report.engine.as_ref().is_some_and(|engine| {
                    engine.established_by == sail_api::output::EngineEstablishment::StockFallback
                })
        })
        .count()
}

fn completed_receipts(run: &Path, data: &Value) -> Result<Vec<(PathBuf, Receipt)>> {
    std::fs::read_dir(run.join("pending-receipts"))
        .map_err(|e| Error::new("SAIL3107", e.to_string()))?
        .map(|entry| {
            let path = entry
                .map_err(|e| Error::new("SAIL3107", e.to_string()))?
                .path();
            let mut receipt: Receipt = read(&path)
                .ok_or_else(|| Error::new("SAIL3107", "staged proof receipt is unreadable"))?;
            if receipt.clone().seal().checksum != receipt.checksum {
                return Err(Error::new(
                    "SAIL3107",
                    "staged proof receipt checksum failed",
                ));
            }
            receipt.input_revision = target_revision(data, &receipt.package_id);
            if receipt.input_revision.is_empty() {
                return Err(Error::new(
                    "SAIL3107",
                    "completed target input revision is unavailable",
                ));
            }
            Ok((path, receipt.seal()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn conflicting_research_refuses_before_runner_setup()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        if std::env::var("SAIL_RESEARCH_SELECTION_EXPECTED").as_deref() == Ok("conflict") {
            let absent = std::path::PathBuf::from("/not-a-research-fixture");
            let config = super::Config {
                workspace: absent.clone(),
                cache: absent.clone(),
                sail: absent.clone(),
                cargo_verus: absent.clone(),
                cargo: absent.clone(),
                cargo_toolchain: crate::managed::CargoToolchain::Caller,
                shim: absent.clone(),
                z3: absent,
                socket: None,
                args: Vec::new(),
                force: false,
                attribution_logs: false,
            };
            let error = super::run_engine_held(&config, None, &mut Vec::new(), None)
                .err()
                .ok_or("expected refusal")?;
            assert_eq!(error.code, "SAIL3135");
        }
        Ok(())
    }

    use super::*;

    /// Colour is requested only on a terminal, and never over the user's
    /// own colour setting.
    #[test]
    fn colour_follows_the_terminal_unless_the_user_chose() {
        let args = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert!(colour_wanted(true, false, &args(&["-p", "x"])));
        assert!(!colour_wanted(false, false, &args(&[])));
        assert!(!colour_wanted(true, true, &args(&[])));
        assert!(!colour_wanted(true, false, &args(&["--color", "never"])));
        assert!(!colour_wanted(true, false, &args(&["--color=never"])));
    }

    #[test]
    fn cleans_keep_rustc_incremental_state() -> std::result::Result<(), Box<dyn std::error::Error>>
    {
        let root = std::env::temp_dir().join(format!("sail-keep-inc-{}", std::process::id()));
        let host = root.join("debug/incremental/demo-1");
        let cross = root.join("x86_64-unknown-linux-gnu/release/incremental/demo-2");
        let stale = root.join("debug/incremental.sail-kept-1");
        for directory in [&host, &cross, &stale] {
            std::fs::create_dir_all(directory)?;
        }
        std::fs::write(host.join("s"), b"session")?;
        let kept = keep_incremental(&root);
        assert_eq!(kept.len(), 2);
        assert!(!host.exists() && !cross.exists() && !stale.exists());
        restore_incremental(kept);
        assert_eq!(std::fs::read(host.join("s"))?, b"session");
        assert!(cross.is_dir());
        let leftovers = std::fs::read_dir(root.join("debug"))?
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains("sail-kept"))
            .count();
        assert_eq!(leftovers, 0);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    include!("runner_artifact_isolation_tests.rs");
    #[cfg(unix)]
    include!("runner_cargo_toolchain_tests.rs");
    fn compiler_request(function: Option<&str>) -> crate::engine::Request {
        crate::engine::Request {
            function: function.map(str::to_owned),
            cargo_verus: PathBuf::from("engine/cargo-verus"),
            manifest: PathBuf::from("engine/sail-build.json"),
            manifest_sha256: "manifest-bytes".to_owned(),
            stock_wrapper: PathBuf::from("engine/verus"),
            artifacts: vec![PathBuf::from("engine/rust_verify")],
        }
    }

    #[test]
    fn compiler_artifact_identity_omits_selection_and_the_engine_function()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let args = vec!["-p".to_owned(), "input".to_owned()];
        let identity = vec![("compiler".to_owned(), "compiler-bytes".to_owned())];
        let environment = vec![(b"RUSTFLAGS".to_vec(), b"flags".to_vec())];
        let selected = compiler_request(Some("selected"));
        let missing = compiler_request(Some("missing"));
        let full = compiler_request(None);
        assert_eq!(
            serde_json::to_vec(&selected.artifact_identity())?,
            serde_json::to_vec(&full)?
        );
        let hashes = |request: Option<&crate::engine::Request>| {
            context_hashes(&args, request, &identity, &environment)
        };
        let (proof, compiler) = hashes(Some(&selected))?;
        assert_eq!(
            proof,
            digest(&serde_json::to_vec(&(
                1,
                &args,
                &selected,
                &identity,
                &environment
            ))?)
        );
        for request in [&missing, &full] {
            let (other_proof, other_compiler) = hashes(Some(request))?;
            assert_ne!(proof, other_proof);
            assert_eq!(compiler, other_compiler);
        }
        assert_ne!(compiler, hashes(None)?.1);
        let mut variants = vec![selected.clone(); 5];
        variants[0].cargo_verus = "another/cargo-verus".into();
        variants[1].manifest = "another/sail-build.json".into();
        variants[2].manifest_sha256 = "different-manifest".into();
        variants[3].stock_wrapper = "another/verus".into();
        variants[4].artifacts.push("another/rust_verify".into());
        for request in variants {
            assert_ne!(compiler, hashes(Some(&request))?.1);
        }
        let (unselected_proof, unselected_compiler) =
            context_hashes(&[], Some(&selected), &identity, &environment)?;
        assert_ne!(proof, unselected_proof);
        assert_eq!(compiler, unselected_compiler);
        assert_ne!(
            compiler,
            context_hashes(&args, Some(&selected), &[], &environment)?.1
        );
        let changed = vec![(b"RUSTFLAGS".to_vec(), b"different".to_vec())];
        assert_ne!(
            compiler,
            context_hashes(&args, Some(&selected), &identity, &changed)?.1
        );
        Ok(())
    }

    #[test]
    fn compiler_family_selection_keeps_exact_proofs_and_other_inputs()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let identity = vec![("compiler".to_owned(), "bytes".to_owned())];
        let environment = vec![(b"RUSTFLAGS".to_vec(), b"flags".to_vec())];
        let request = compiler_request(None);
        let hashes = |args: &[&str]| {
            let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
            context_hashes(&args, Some(&request), &identity, &environment)
        };
        let (proof, family) = hashes(&["--package", "input"])?;
        for selection in [
            &["--package", "input", "--lib"][..],
            &["-pother", "--bin=main"],
            &["--workspace", "--exclude=other", "--bins"],
        ] {
            let (other_proof, other_family) = hashes(selection)?;
            assert_ne!(proof, other_proof, "proof args stay exact: {selection:?}");
            assert_eq!(family, other_family, "selection: {selection:?}");
        }
        for changed in [
            &["--features", "extra"][..],
            &["--no-default-features"],
            &["--all-features"],
            &["--profile", "test"],
            &["--release"],
            &["--target", "another-target"],
            &["--config", "build.rustflags=['--cfg', 'changed']"],
            &["--manifest-path", "other/Cargo.toml"],
            &["--", "--lib"],
            &["--", "-p", "input"],
        ] {
            assert_ne!(
                family,
                hashes(changed)?.1,
                "input remains bound: {changed:?}"
            );
        }
        Ok(())
    }

    /// rc.5 finding 15: the dependency store's family ignores which members
    /// and targets Cargo builds, and nothing else.
    #[test]
    fn store_family_ignores_only_package_and_target_selection()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let identity = vec![("compiler".to_owned(), "compiler-bytes".to_owned())];
        let environment = vec![(b"RUSTFLAGS".to_vec(), b"flags".to_vec())];
        let family = |args: &[&str]| {
            let args: Vec<String> = args.iter().map(|&a| a.to_owned()).collect();
            store_family_hash(&args, None, &identity, &environment)
        };
        let plain = family(&[])?;
        for selection in [
            &["-p", "a"][..],
            &["-pa"],
            &["--package", "a", "--lib"],
            &["--package=a", "--bin", "main"],
            &["--bin=main", "--bins"],
            &["--workspace", "--exclude", "b"],
            &["--all", "--exclude=b"],
        ] {
            assert_eq!(family(selection)?, plain, "{selection:?}");
        }
        for other in [
            &["--features", "x"][..],
            &["-p", "a", "--release"],
            &["--", "-p", "a"],
            &["--", "--lib"],
            &["--manifest-path", "other/Cargo.toml"],
        ] {
            assert_ne!(family(other)?, plain, "{other:?}");
        }
        assert_eq!(
            family(&["-p", "a", "--", "--rlimit", "5"])?,
            family(&["--", "--rlimit", "5"])?
        );
        assert_ne!(
            plain,
            store_family_hash(&[], None, &[], &environment)?,
            "the toolchain identity stays bound"
        );
        assert_ne!(
            plain,
            store_family_hash(&[], None, &identity, &[])?,
            "the environment stays bound"
        );
        Ok(())
    }

    fn compiler_fixture(root: &Path) -> std::result::Result<Config, std::io::Error> {
        std::fs::create_dir_all(root)?;
        let sail = root.join("sail");
        std::fs::write(&sail, b"owned executable")?;
        Ok(Config {
            workspace: root.to_owned(),
            cache: root.join("cache"),
            sail: sail.clone(),
            cargo_verus: sail.clone(),
            cargo: sail.clone(),
            cargo_toolchain: crate::managed::CargoToolchain::Caller,
            shim: sail.clone(),
            z3: sail,
            socket: None,
            args: Vec::new(),
            force: true,
            attribution_logs: true,
        })
    }

    #[test]
    fn shared_compiler_target_has_exclusive_ownership_and_private_proof_paths()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-compiler-family-{}", std::process::id()));
        let config = compiler_fixture(&root)?;
        let (selected, ownership) = prepare(&config, &root, "selected", "compiler", "first")?;
        std::fs::create_dir_all(&selected.target)?;
        let dependency = selected.target.join("ordinary-dependency.rlib");
        std::fs::write(&dependency, "ordinary dependency")?;
        // A concurrent run of the same family waits for the owner, then proceeds.
        let waiting = std::thread::spawn({
            let config = config.clone();
            let root = root.clone();
            move || {
                prepare(&config, &root, "waiting", "compiler", "second")
                    .map(|(_, ownership)| drop(ownership))
                    .map_err(|e| e.code)
            }
        });
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(!waiting.is_finished(), "concurrent family owner admitted");
        drop(ownership);
        assert_eq!(waiting.join().map_err(|_| "waiter panicked")?, Ok(()));
        // Failed family acquisition must also release the new proof-context lock.
        let (missing, ownership) = prepare(&config, &root, "missing", "compiler", "second")?;
        assert_eq!(selected.target, missing.target);
        assert_eq!(selected.cargo_alias, missing.cargo_alias);
        assert_eq!(selected.proxy_alias, missing.proxy_alias);
        assert_eq!(std::fs::read(&dependency)?, b"ordinary dependency");
        assert_ne!(selected.receipts, missing.receipts);
        assert_ne!(selected.runtime, missing.runtime);
        assert_ne!(selected.run, missing.run);
        assert_ne!(selected.build_inputs, missing.build_inputs);
        let (isolated, isolated_owner) =
            prepare(&config, &root, "different", "different", "third")?;
        assert_ne!(missing.target, isolated.target);
        drop(isolated_owner);
        drop(ownership);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn compiler_family_process_probe() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let Some(root) = std::env::var_os("SAIL_TEST_COMPILER_FAMILY") else {
            return Ok(());
        };
        let result = ownership_lock(Path::new(&root));
        if std::env::var_os("SAIL_TEST_COMPILER_FAMILY_BUSY").is_some() {
            assert_eq!(
                result
                    .err()
                    .ok_or("child unexpectedly acquired family")?
                    .code,
                "SAIL3104"
            );
        } else {
            drop(result?);
        }
        Ok(())
    }

    #[test]
    fn compiler_family_lock_is_process_exclusive_and_released_on_error_and_unwind()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-compiler-process-{}", std::process::id()));
        let config = compiler_fixture(&root)?;
        let probe = |busy| -> std::result::Result<(), Box<dyn std::error::Error>> {
            let family = config
                .cache
                .join("projects")
                .join(digest(root.as_os_str().as_encoded_bytes()))
                .join("compiler-artifacts/compiler");
            let mut command = Command::new(std::env::current_exe()?);
            command
                .args([
                    "--exact",
                    "managed::runner::tests::compiler_family_process_probe",
                ])
                .env("SAIL_TEST_COMPILER_FAMILY", family)
                .env_remove("SAIL_TEST_COMPILER_FAMILY_BUSY");
            if busy {
                command
                    .env("SAIL_TEST_COMPILER_FAMILY_BUSY", "1")
                    .env("SAIL_MANAGED_LOCK_WAIT_SECS", "0");
            }
            let output = command.output()?;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
            Ok(())
        };
        let (_, ownership) = prepare(&config, &root, "context", "compiler", "run")?;
        probe(true)?;
        drop(ownership);
        probe(false)?;
        let mut broken = config.clone();
        broken.sail = root.join("missing-executable");
        assert!(prepare(&broken, &root, "context", "compiler", "error").is_err());
        let outcome = std::panic::catch_unwind(|| {
            let owner = prepare(&config, &root, "context", "compiler", "unwind");
            assert!(owner.is_ok());
            std::panic::resume_unwind(Box::new("controlled ownership unwind"));
        });
        assert!(outcome.is_err());
        let (_, ownership) = prepare(&config, &root, "context", "compiler", "after")?;
        drop(ownership);
        probe(false)?;
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    fn compiler_receipt(paths: &Paths, data: &Value, artifact: &Path) -> Receipt {
        let mut report = crate::report::setup_failure(&[], "SAIL3100", "fixture");
        report.error = None;
        report.exit_code = Some(0);
        report.crates.push(sail_api::output::CrateReport {
            name: Some("input".to_owned()),
            verus: sail_api::output::VerusResults {
                verified: Some(0),
                errors: Some(0),
                success: Some(true),
                encountered_vir_error: Some(false),
            },
            functions: Vec::new(),
        });
        Receipt {
            version: RECEIPT_VERSION,
            checksum: String::new(),
            committed: paths.run.join("committed.json"),
            package_id: "input".to_owned(),
            name: "input".to_owned(),
            kind: vec!["lib".to_owned()],
            context: "selected".to_owned(),
            input_revision: target_revision(data, "input"),
            invocation: Invocation {
                retained_from: None,
                identity: super::super::receipts::TargetIdentity {
                    manifest: PathBuf::from(
                        data["packages"][0]["manifest_path"]
                            .as_str()
                            .unwrap_or_default(),
                    ),
                    name: "input".to_owned(),
                    crate_type: "lib".to_owned(),
                    rustc_args_sha256: "args".to_owned(),
                },
                run_id: "run".to_owned(),
                report,
                success: true,
                dependencies: BTreeMap::new(),
                dependency_build: None,
            },
            artifact_hashes: artifact_hashes(&[artifact.to_owned()]).unwrap_or_default(),
            declared_environment: BTreeMap::new(),
        }
        .seal()
    }

    #[test]
    fn optional_retention_keeps_cold_missing_and_changed_revisions()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-retained-warm-{}", std::process::id()));
        let config = compiler_fixture(&root)?;
        let (paths, ownership) = prepare(&config, &root, "selected", "compiler", "run")?;
        let source = root.join("input.rs");
        std::fs::write(&source, b"A")?;
        let data = serde_json::json!({
            "workspace_root":root, "sail_cache_directory":config.cache,
            "target_directory":paths.target,
            "packages":[{"id":"input", "manifest_path":root.join("Cargo.toml")}],
            "resolve":{"nodes":[{"id":"input","dependencies":[]}]}
        });
        let expected = BTreeSet::from([("input".into(), "input".into(), "lib".into())]);
        let path = receipt_path(&paths, "input", "input", &["lib".into()]);
        let load = || -> BTreeMap<PathBuf, ReceiptRevision> {
            read(&path)
                .map(|receipt| BTreeMap::from([(path.clone(), receipt)]))
                .unwrap_or_default()
        };
        assert!(!receipt_covered_targets(
            &paths,
            &expected,
            "selected",
            &load()
        ));
        let original = compiler_receipt(&paths, &data, &root.join("absent-artifact"));
        json(&path, &original)?;
        assert!(receipt_covered_targets(
            &paths,
            &expected,
            "selected",
            &load()
        ));
        assert_eq!(dirty_receipt_packages(&load(), &data), Vec::<String>::new());
        // A matching header is explicitly not proof authority: this fixture
        // has no committed proof or artifact, and ordinary validation refuses.
        assert!(!original.valid("selected"));
        std::fs::write(&source, b"B")?;
        assert_eq!(dirty_receipt_packages(&load(), &data), ["input"]);
        let edited = compiler_receipt(&paths, &data, &root.join("absent-artifact"));
        json(&path, &edited)?;
        assert_eq!(dirty_receipt_packages(&load(), &data), Vec::<String>::new());
        std::fs::write(&source, b"A")?;
        // Returned A still needs restoration against B's currently held receipt.
        assert_eq!(dirty_receipt_packages(&load(), &data), ["input"]);
        json(&path, &original)?;
        assert_eq!(dirty_receipt_packages(&load(), &data), Vec::<String>::new());
        let mut missing = expected.clone();
        missing.insert(("other".into(), "other".into(), "lib".into()));
        assert!(!receipt_covered_targets(
            &paths,
            &missing,
            "selected",
            &load()
        ));
        std::fs::remove_file(&path)?;
        assert!(!receipt_covered_targets(
            &paths,
            &expected,
            "selected",
            &load()
        ));
        drop(ownership);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn optional_retention_malformed_or_foreign_headers_keep_plan()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-retained-header-{}", std::process::id()));
        let config = compiler_fixture(&root)?;
        let (paths, ownership) = prepare(&config, &root, "selected", "compiler", "run")?;
        let expected = BTreeSet::from([("input".into(), "input".into(), "lib".into())]);
        let path = receipt_path(&paths, "input", "input", &["lib".into()]);
        let original = serde_json::json!({"version":RECEIPT_VERSION,"package_id":"input",
            "name":"input","kind":["lib"],"context":"selected","input_revision":"revision"});
        for (field, value) in [
            ("version", serde_json::json!(0)),
            ("context", serde_json::json!("foreign")),
            ("package_id", serde_json::json!("foreign")),
            ("name", serde_json::json!("foreign")),
            ("kind", serde_json::json!(["bin"])),
            ("kind", serde_json::json!("lib")),
            ("input_revision", serde_json::json!("")),
            ("context", Value::Null),
        ] {
            let mut header = original.clone();
            header[field] = value;
            json(&path, &header)?;
            let revisions = read(&path)
                .map(|r| BTreeMap::from([(path.clone(), r)]))
                .unwrap_or_default();
            assert!(!receipt_covered_targets(
                &paths, &expected, "selected", &revisions
            ));
        }
        for bytes in [
            b"{malformed".as_slice(),
            br#"{"package_id":"input","input_revision":"old"}"#,
        ] {
            std::fs::write(&path, bytes)?;
            let revisions = read(&path)
                .map(|r| BTreeMap::from([(path.clone(), r)]))
                .unwrap_or_default();
            assert!(!receipt_covered_targets(
                &paths, &expected, "selected", &revisions
            ));
        }
        drop(ownership);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn newly_seeded_stdout_replays_once_at_its_fresh_artifact()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-seeded-output-{}", std::process::id()));
        let config = compiler_fixture(&root)?;
        let (paths, ownership) = prepare(&config, &root, "selected", "compiler", "run")?;
        let data = serde_json::json!({"packages":[{"id":"input", "manifest_path":root.join("Cargo.toml")}]});
        let shown = b"verification results:: 1 verified, 0 errors\n";
        let seed = super::super::store::Seeded {
            key: "key".into(),
            receipt: compiler_receipt(&paths, &data, &root.join("absent")),
            placed: true,
            shown_stdout: shown.to_vec(),
        };
        let mut seeds = [seed];
        let record = |package, fresh| {
            serde_json::json!({
                "reason":"compiler-artifact", "package_id":package, "fresh":fresh,
                "target":{"name":"input","kind":["lib"]}
            })
        };
        let mut emitted = BTreeSet::new();
        assert_eq!(
            seeded_stdout(&record("other", true), &seeds, &mut emitted),
            None
        );
        assert_eq!(
            seeded_stdout(&record("input", false), &seeds, &mut emitted),
            None
        );
        assert_eq!(
            seeded_stdout(&record("input", true), &seeds, &mut emitted),
            Some(shown.as_slice())
        );
        assert_eq!(
            seeded_stdout(&record("input", true), &seeds, &mut emitted),
            None
        );
        seeds[0].placed = false;
        assert_eq!(
            seeded_stdout(&record("input", true), &seeds, &mut BTreeSet::new()),
            None
        );
        drop(ownership);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    struct NegativeGeneration {
        root: PathBuf,
        paths: Paths,
        data: Value,
        expected: BTreeSet<(String, String, String)>,
        call: Invocation,
        result: ProjectResult,
        _ownership: Ownership,
    }

    impl NegativeGeneration {
        fn new(label: &str) -> std::result::Result<Self, Box<dyn std::error::Error>> {
            // Cargo reports canonical manifest paths; macOS's temporary
            // directory is reached through the `/var` symlink.
            let root = std::env::temp_dir().canonicalize()?.join(format!(
                "sail-negative-generation-{label}-{}",
                std::process::id()
            ));
            let config = compiler_fixture(&root)?;
            let (paths, ownership) = prepare(&config, &root, "negative", "compiler", "run")?;
            let data = serde_json::json!({"packages":[{
                "id":"input", "name":"input", "manifest_path":root.join("Cargo.toml")
            }]});
            let mut call = compiler_receipt(&paths, &data, &root.join("absent")).invocation;
            call.success = false;
            call.report.exit_code = Some(1);
            call.report.crates[0].verus.verified = Some(1);
            call.report.crates[0].verus.errors = Some(1);
            call.report.crates[0].verus.success = Some(false);
            let result = ProjectResult {
                kind: "sail-project-result".to_owned(),
                schema_version: SCHEMA_VERSION,
                run_id: "run".to_owned(),
                workspace: root.to_string_lossy().into_owned(),
                context_sha256: "negative".to_owned(),
                input_revision: "revision".to_owned(),
                currentness: Currentness::Current,
                exit_code: Some(101),
                targets: Vec::new(),
                execution: Execution {
                    verifier_invocations: 1,
                    reused_targets: 0,
                    wall_ms: 1.0,
                },
                notes: Vec::new(),
                error: None,
                engine: None,
                no_reuse: None,
            };
            let fixture = Self {
                root,
                paths,
                data,
                call,
                result,
                _ownership: ownership,
                expected: BTreeSet::from([(
                    "input".to_owned(),
                    "input".to_owned(),
                    "lib".to_owned(),
                )]),
            };
            fixture.seal()?;
            Ok(fixture)
        }

        fn seal(&self) -> std::io::Result<()> {
            let sidecar = self
                .paths
                .run
                .join("sidecars")
                .join(self.call.identity.key());
            std::fs::create_dir_all(&sidecar)?;
            std::fs::write(
                sidecar.join("verus.stdout"),
                include_bytes!("../../tests/fixtures/engine-terminal/failed-proof.json"),
            )?;
            std::fs::write(
                sidecar.join("engine.events.jsonl"),
                include_bytes!("../../tests/fixtures/engine-terminal/terminal-failure.jsonl"),
            )?;
            std::fs::write(
                self.paths
                    .run
                    .join("invocations")
                    .join(format!("{}.json", self.call.identity.key())),
                serde_json::to_vec(&self.call)?,
            )?;
            std::fs::write(
                self.paths.run.join("invocations/1.started"),
                serde_json::to_vec(&serde_json::json!({"run_id":"run","target":"input"}))?,
            )
        }

        fn eligible(&self) -> bool {
            negative_compiler_generation(
                &self.paths,
                &self.data,
                &self.expected,
                &self.result,
                true,
                true,
                &[],
            )
        }
    }

    impl Drop for NegativeGeneration {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "The refusal matrix keeps all mutations of the same sealed full outcome together."
    )]
    fn negative_compiler_generation_requires_full_normal_current_and_complete_coverage()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut f = NegativeGeneration::new("refusals")?;
        assert!(f.eligible());
        assert!(!complete(&f.call.report));
        for currentness in [Currentness::Unknown, Currentness::Historical] {
            f.result.currentness = currentness;
            assert!(!f.eligible());
        }
        f.result.currentness = Currentness::Current;
        for exit in [None, Some(1), Some(2), Some(0)] {
            f.result.exit_code = exit;
            assert!(!f.eligible());
        }
        f.result.exit_code = Some(101);
        assert!(!negative_compiler_generation(
            &f.paths,
            &f.data,
            &f.expected,
            &f.result,
            false,
            true,
            &[]
        ));
        assert!(!negative_compiler_generation(
            &f.paths,
            &f.data,
            &f.expected,
            &f.result,
            false,
            false,
            &[]
        ));
        assert!(!negative_compiler_generation(
            &f.paths,
            &f.data,
            &f.expected,
            &f.result,
            true,
            true,
            &["--".to_owned(), "--verify-function=selected".to_owned()]
        ));
        f.expected.insert((
            "input".to_owned(),
            "unattempted".to_owned(),
            "bin".to_owned(),
        ));
        assert!(!f.eligible());
        f.expected.retain(|(_, name, _)| name == "input");
        let original_call = f.call.clone();
        for field in [
            "error",
            "errors",
            "verified",
            "exit",
            "success",
            "report-success",
        ] {
            f.call = original_call.clone();
            match field {
                "error" => {
                    f.call.report.error =
                        crate::report::setup_failure(&[], "SAIL3100", "fixture").error;
                }
                "errors" => f.call.report.crates[0].verus.errors = Some(2),
                "verified" => f.call.report.crates[0].verus.verified = Some(0),
                "exit" => f.call.report.exit_code = Some(101),
                "report-success" => f.call.report.crates[0].verus.success = Some(true),
                _ => f.call.success = true,
            }
            f.seal()?;
            assert!(!f.eligible(), "mismatched report {field}");
        }
        f.call = original_call;
        f.seal()?;
        for start in [
            b"{".as_slice(),
            b"{\"run_id\":\"other\",\"target\":\"input\"}".as_slice(),
        ] {
            std::fs::write(f.paths.run.join("invocations/1.started"), start)?;
            assert!(!f.eligible(), "malformed or wrong-run start");
        }
        f.seal()?;
        std::fs::remove_file(
            f.paths
                .run
                .join("invocations")
                .join(format!("{}.json", f.call.identity.key())),
        )?;
        assert!(!f.eligible(), "started invocation missing its completion");
        f.seal()?;
        let sidecar = f.paths.run.join("sidecars").join(f.call.identity.key());
        for field in [
            "encountered-vir-error",
            "is-verifying-entire-crate",
            "errors",
        ] {
            f.seal()?;
            let mut doc: Value =
                serde_json::from_slice(&std::fs::read(sidecar.join("verus.stdout"))?)?;
            doc["verification-results"][field] = match field {
                "encountered-vir-error" => Value::Bool(true),
                "errors" => Value::from(0),
                _ => Value::Bool(false),
            };
            std::fs::write(sidecar.join("verus.stdout"), serde_json::to_vec(&doc)?)?;
            assert!(!f.eligible(), "{field}");
        }
        f.seal()?;
        std::fs::write(sidecar.join("engine.events.jsonl"),
            b"{\"version\":1,\"event\":\"compiler-sealed\",\"phase\":\"full\",\"payload\":{\"success\":false,\"normal_termination\":false}}\n")?;
        assert!(!f.eligible());
        f.seal()?;
        std::fs::write(
            f.paths.run.join("invocations/2.started"),
            b"{\"run_id\":\"run\",\"target\":\"input\"}",
        )?;
        assert!(!f.eligible(), "one completion cannot cover two starts");
        std::fs::remove_file(f.paths.run.join("invocations/2.started"))?;
        std::fs::write(f.paths.run.join("invocations/pending.json"), b"{")?;
        assert!(!f.eligible());
        std::fs::remove_file(f.paths.run.join("invocations/pending.json"))?;
        assert!(f.eligible());
        Ok(())
    }

    /// A stock run has no engine seal: its whole document and normal exit
    /// stand in for it, and every other refusal still applies.
    #[test]
    fn stock_negative_generation_needs_a_complete_document_and_normal_exit()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut f = NegativeGeneration::new("stock")?;
        let stock = |f: &NegativeGeneration| {
            negative_compiler_generation(
                &f.paths,
                &f.data,
                &f.expected,
                &f.result,
                true,
                false,
                &[],
            )
        };
        let sidecar = f.paths.run.join("sidecars").join(f.call.identity.key());
        std::fs::remove_file(sidecar.join("engine.events.jsonl"))?;
        assert!(stock(&f), "stock needs no engine terminal");
        assert!(!f.eligible(), "the engine still needs its terminal");
        // A crash exit, a truncated or doubled document, or a document that
        // disagrees with the exit is not a completed stock outcome.
        let original_call = f.call.clone();
        f.call.report.exit_code = Some(101);
        f.seal()?;
        std::fs::remove_file(sidecar.join("engine.events.jsonl"))?;
        assert!(!stock(&f), "panic exit");
        f.call = original_call;
        f.seal()?;
        std::fs::remove_file(sidecar.join("engine.events.jsonl"))?;
        let document = std::fs::read(sidecar.join("verus.stdout"))?;
        for bytes in [
            document[..document.len() / 2].to_vec(),
            [document.as_slice(), document.as_slice()].concat(),
        ] {
            std::fs::write(sidecar.join("verus.stdout"), bytes)?;
            assert!(!stock(&f), "partial or repeated document");
        }
        let mut doc: Value = serde_json::from_slice(&document)?;
        doc["verification-results"]["success"] = Value::Bool(true);
        std::fs::write(sidecar.join("verus.stdout"), serde_json::to_vec(&doc)?)?;
        assert!(!stock(&f), "document contradicts the exit");
        std::fs::write(sidecar.join("verus.stdout"), &document)?;
        assert!(stock(&f));
        f.result.currentness = Currentness::Historical;
        assert!(!stock(&f), "only a Current run retains contracts");
        f.result.currentness = Currentness::Current;
        assert!(!negative_compiler_generation(
            &f.paths,
            &f.data,
            &f.expected,
            &f.result,
            true,
            false,
            &["--".to_owned(), "--verify-function=selected".to_owned()]
        ));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "One real Cargo dependency lifecycle brackets the negative generation and restored-input refusal."
    )]
    fn current_negative_generation_reuses_real_dependency_without_a_proof_receipt()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut f = NegativeGeneration::new("cargo")?;
        std::fs::create_dir_all(f.root.join("dependency/src"))?;
        std::fs::create_dir_all(f.root.join("src"))?;
        std::fs::write(
            f.root.join("Cargo.toml"),
            "[package]\nname='input'\nversion='0.1.0'\nedition='2024'\n[package.metadata.verus]\nverify=true\n[dependencies]\nordinary-dep={path='dependency'}\n[workspace]\n",
        )?;
        std::fs::write(
            f.root.join("src/lib.rs"),
            "pub fn input() { ordinary_dep::ordinary(); }\n",
        )?;
        std::fs::write(
            f.root.join("dependency/Cargo.toml"),
            "[package]\nname='ordinary-dep'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        std::fs::write(
            f.root.join("dependency/src/lib.rs"),
            "pub fn ordinary() {}\n",
        )?;
        std::fs::write(f.root.join("external.txt"), "good")?;
        std::fs::write(
            f.root.join("dependency/build.rs"),
            "fn main() { println!(\"cargo:rerun-if-changed=../external.txt\"); std::fs::write(std::env::var(\"OUT_DIR\").unwrap()+\"/generated\",std::fs::read(\"../external.txt\").unwrap()).unwrap(); }\n",
        )?;
        let cargo = |arguments: &[&str]| -> std::io::Result<std::process::Output> {
            Command::new("cargo")
                .current_dir(&f.root)
                .args(arguments)
                .arg("--target-dir")
                .arg(&f.paths.target)
                .env_remove("RUSTC_WRAPPER")
                .env_remove("RUSTC_WORKSPACE_WRAPPER")
                .output()
        };
        let built = cargo(&["build", "--offline", "--message-format=json"])?;
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let metadata = Command::new("cargo")
            .current_dir(&f.root)
            .args(["metadata", "--offline", "--format-version=1"])
            .output()?;
        assert!(metadata.status.success());
        f.data = serde_json::from_slice(&metadata.stdout)?;
        f.expected = expected_targets(
            &f.data,
            &[
                "--package".to_owned(),
                "input".to_owned(),
                "--lib".to_owned(),
            ],
        );
        let artifacts = f.paths.run.join("artifacts.jsonl");
        std::fs::write(&artifacts, &built.stdout)?;
        super::super::build_inputs::update(&f.paths.build_inputs, &artifacts, &f.data)?;
        let generation = CompilerGeneration {
            compiler: "compiler",
            script_coverage: true,
            source_engine: true,
            args: &[],
        };
        // Bootstrap through the unchanged successful publication branch.
        f.result.exit_code = Some(0);
        publish_compiler_generation(&f.paths, &f.data, &f.expected, &f.result, &generation);
        super::super::build_inputs::family::begin(
            &f.paths.compiler_inputs,
            "compiler",
            &f.paths.target,
            &f.paths.build_inputs,
            &f.data,
        )?;
        assert!(!f.paths.compiler_inputs.exists());
        let (clean, _, coverage) =
            super::super::build_inputs::stale(&f.paths.build_inputs, &f.data);
        assert_eq!(clean, Vec::<String>::new());
        assert!(coverage);
        f.result.exit_code = Some(101);
        assert!(f.eligible());
        let (targets, _) = target_records(&f.paths, &f.data, "negative", true, &f.expected, &[])?;
        assert_eq!(targets.len(), 1);
        assert!(!targets[0].complete && !targets[0].reused);
        assert_eq!(targets[0].report.exit_code, Some(1));
        assert_eq!(std::fs::read_dir(&f.paths.receipts)?.count(), 0);
        publish_compiler_generation(&f.paths, &f.data, &f.expected, &f.result, &generation);
        assert!(f.paths.compiler_inputs.is_file());
        let next_context = f.root.join("next-context-build-inputs.json");
        super::super::build_inputs::family::begin(
            &f.paths.compiler_inputs,
            "compiler",
            &f.paths.target,
            &next_context,
            &f.data,
        )?;
        assert_eq!(
            super::super::build_inputs::stale(&next_context, &f.data).0,
            Vec::<String>::new()
        );
        assert_eq!(
            super::super::build_inputs::family::recheck(&next_context, &f.data, &f.paths.target)?,
            Vec::<String>::new()
        );
        let cleaned = cargo(&["clean", "--offline", "-p", "input"])?;
        assert!(cleaned.status.success());
        let next = cargo(&["build", "--offline", "--message-format=json"])?;
        assert!(next.status.success());
        let records: Vec<Value> = next
            .stdout
            .split(|b| *b == b'\n')
            .filter_map(|line| serde_json::from_slice(line).ok())
            .collect();
        assert!(records.iter().any(|r| r["reason"] == "compiler-artifact"
            && r["target"]["name"] == "ordinary_dep"
            && r["fresh"] == true));
        assert!(records.iter().any(|r| r["reason"] == "compiler-artifact"
            && r["target"]["name"] == "input"
            && r["fresh"] == false));
        let sidecar = f.paths.run.join("sidecars").join(f.call.identity.key());
        for field in ["encountered-vir-error", "is-verifying-entire-crate"] {
            f.seal()?;
            let mut doc: Value =
                serde_json::from_slice(&std::fs::read(sidecar.join("verus.stdout"))?)?;
            doc["verification-results"][field] = Value::Bool(field == "encountered-vir-error");
            std::fs::write(sidecar.join("verus.stdout"), serde_json::to_vec(&doc)?)?;
            publish_compiler_generation(&f.paths, &f.data, &f.expected, &f.result, &generation);
            assert!(
                !f.paths.compiler_inputs.exists(),
                "{field} must leave consumed export absent"
            );
        }
        f.seal()?;
        std::fs::remove_file(sidecar.join("engine.events.jsonl"))?;
        publish_compiler_generation(&f.paths, &f.data, &f.expected, &f.result, &generation);
        assert!(
            !f.paths.compiler_inputs.exists(),
            "unfinished full invocation cannot publish"
        );
        std::fs::write(sidecar.join("engine.events.jsonl"),
            b"{\"version\":1,\"event\":\"compiler-sealed\",\"phase\":\"full\",\"payload\":{\"success\":false,\"normal_termination\":false}}\n")?;
        publish_compiler_generation(&f.paths, &f.data, &f.expected, &f.result, &generation);
        assert!(
            !f.paths.compiler_inputs.exists(),
            "abnormal termination cannot publish"
        );
        f.seal()?;
        let establishing = CompilerGeneration {
            script_coverage: false,
            ..generation
        };
        publish_compiler_generation(&f.paths, &f.data, &f.expected, &f.result, &establishing);
        assert!(
            !f.paths.compiler_inputs.exists(),
            "negative declaration establishment needs Cargo's completed build"
        );
        // A project's first run fails a proof (issue #123): Cargo finished its
        // build and ran every recorded script, so the dependencies' contracts
        // are kept and the next run finds vstd fresh.
        let output = String::from_utf8(built.stdout.clone())?.replace(
            "\"build-finished\",\"success\":true",
            "\"build-finished\",\"success\":false",
        );
        std::fs::write(f.paths.run.join("cargo.stdout"), &output)?;
        publish_compiler_generation(&f.paths, &f.data, &f.expected, &f.result, &establishing);
        assert!(
            f.paths.compiler_inputs.is_file(),
            "a complete negative establishing run keeps the contracts"
        );
        std::fs::remove_file(&f.paths.compiler_inputs)?;
        let unexecuted: String = output
            .lines()
            .filter(|line| !line.contains("build-script-executed"))
            .flat_map(|line| [line, "\n"])
            .collect();
        std::fs::write(&artifacts, unexecuted)?;
        publish_compiler_generation(&f.paths, &f.data, &f.expected, &f.result, &establishing);
        assert!(
            !f.paths.compiler_inputs.exists(),
            "a recorded script that Cargo did not run in the establishing run"
        );
        std::fs::write(&artifacts, &built.stdout)?;
        // The consumed generation does not survive an interrupted rewrite.
        let input = std::fs::File::open(f.root.join("external.txt"))?;
        let mtime = input.metadata()?.modified()?;
        std::fs::write(f.root.join("external.txt"), "fail")?;
        input.set_times(std::fs::FileTimes::new().set_modified(mtime))?;
        assert_eq!(
            super::super::build_inputs::family::recheck(&next_context, &f.data, &f.paths.target)?,
            vec!["ordinary-dep"]
        );
        f.result.currentness = Currentness::Unknown;
        publish_compiler_generation(&f.paths, &f.data, &f.expected, &f.result, &generation);
        assert!(!f.paths.compiler_inputs.exists());
        std::fs::write(f.root.join("external.txt"), "good")?;
        input.set_times(std::fs::FileTimes::new().set_modified(mtime))?;
        super::super::build_inputs::family::begin(
            &f.paths.compiler_inputs,
            "compiler",
            &f.paths.target,
            &f.paths.build_inputs,
            &f.data,
        )?;
        assert_eq!(
            super::super::build_inputs::stale(&f.paths.build_inputs, &f.data).0,
            vec!["ordinary-dep"]
        );
        Ok(())
    }

    /// A compile error in the selected crate, after Cargo completed every
    /// dependency, keeps the dependencies' build-script contracts, as plain
    /// Cargo keeps them fresh. The failing crate gets no receipt, and any
    /// doubt about what Cargo completed still withdraws the contracts.
    #[cfg(unix)]
    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "One real Cargo compile failure brackets the retained contracts and every refusal."
    )]
    fn confined_compile_failure_keeps_dependency_contracts_without_a_receipt()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut f = NegativeGeneration::new("compile-error")?;
        std::fs::create_dir_all(f.root.join("dependency/src"))?;
        std::fs::create_dir_all(f.root.join("src"))?;
        std::fs::write(
            f.root.join("Cargo.toml"),
            "[package]\nname='input'\nversion='0.1.0'\nedition='2024'\n[package.metadata.verus]\nverify=true\n[dependencies]\nordinary-dep={path='dependency'}\n[workspace]\n",
        )?;
        std::fs::write(
            f.root.join("src/lib.rs"),
            "pub fn input() -> u32 { ordinary_dep::ordinary(); true }\n",
        )?;
        std::fs::write(
            f.root.join("dependency/Cargo.toml"),
            "[package]\nname='ordinary-dep'\nversion='0.1.0'\nedition='2024'\n",
        )?;
        std::fs::write(
            f.root.join("dependency/src/lib.rs"),
            "pub fn ordinary() {}\n",
        )?;
        std::fs::write(f.root.join("external.txt"), "good")?;
        std::fs::write(
            f.root.join("dependency/build.rs"),
            "fn main() { println!(\"cargo:rerun-if-changed=../external.txt\"); std::fs::write(std::env::var(\"OUT_DIR\").unwrap()+\"/generated\",std::fs::read(\"../external.txt\").unwrap()).unwrap(); }\n",
        )?;
        let built = Command::new("cargo")
            .current_dir(&f.root)
            .args(["build", "--offline", "--message-format=json"])
            .arg("--target-dir")
            .arg(&f.paths.target)
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTC_WORKSPACE_WRAPPER")
            .output()?;
        assert_eq!(
            built.status.code(),
            Some(101),
            "the selected crate must not compile"
        );
        let metadata = Command::new("cargo")
            .current_dir(&f.root)
            .args(["metadata", "--offline", "--format-version=1"])
            .output()?;
        assert!(metadata.status.success());
        f.data = serde_json::from_slice(&metadata.stdout)?;
        f.expected = expected_targets(
            &f.data,
            &[
                "--package".to_owned(),
                "input".to_owned(),
                "--lib".to_owned(),
            ],
        );
        let artifacts = f.paths.run.join("artifacts.jsonl");
        std::fs::write(&artifacts, &built.stdout)?;
        std::fs::write(f.paths.run.join("cargo.stdout"), &built.stdout)?;
        super::super::build_inputs::update(&f.paths.build_inputs, &artifacts, &f.data)?;
        // The selected crate's driver stopped at a compile error: exit 1 and a
        // document with no proof errors that is not a completed verification.
        f.call.report.crates[0].verus.verified = Some(0);
        f.call.report.crates[0].verus.errors = Some(0);
        f.seal()?;
        let sidecar = f.paths.run.join("sidecars").join(f.call.identity.key());
        std::fs::write(
            sidecar.join("verus.stdout"),
            serde_json::to_vec(
                &serde_json::json!({"func-details":{},"verification-results":{
                "encountered-error":true,"encountered-vir-error":false,"success":false,
                "verified":0,"errors":0,"is-verifying-entire-crate":true}}),
            )?,
        )?;
        std::fs::remove_file(sidecar.join("engine.events.jsonl"))?;
        assert!(!crate::engine::completed_compiler_output(
            &f.paths.run,
            &f.call,
            false
        ));
        let stock = |f: &NegativeGeneration, args: &[String]| {
            negative_compiler_generation(
                &f.paths,
                &f.data,
                &f.expected,
                &f.result,
                true,
                false,
                args,
            )
        };
        assert!(stock(&f, &[]));
        assert!(f.eligible(), "the engine arm takes the same branch");
        let (targets, _) = target_records(&f.paths, &f.data, "negative", true, &f.expected, &[])?;
        assert_eq!(targets.len(), 1);
        assert!(!targets[0].complete && !targets[0].reused);
        assert_eq!(std::fs::read_dir(&f.paths.receipts)?.count(), 0);
        let generation = CompilerGeneration {
            compiler: "compiler",
            script_coverage: true,
            source_engine: false,
            args: &[],
        };
        publish_compiler_generation(&f.paths, &f.data, &f.expected, &f.result, &generation);
        assert!(f.paths.compiler_inputs.is_file());
        let next_context = f.root.join("next-context-build-inputs.json");
        super::super::build_inputs::family::begin(
            &f.paths.compiler_inputs,
            "compiler",
            &f.paths.target,
            &next_context,
            &f.data,
        )?;
        assert_eq!(
            super::super::build_inputs::stale(&next_context, &f.data).0,
            Vec::<String>::new(),
            "the next run keeps the dependency's script output"
        );

        // Every doubt about what Cargo completed refuses.
        let output = String::from_utf8(built.stdout.clone())?;
        let without = |reason: &str| -> String {
            output
                .lines()
                .filter(|line| !line.contains(reason))
                .flat_map(|line| [line, "\n"])
                .collect()
        };
        std::fs::write(f.paths.run.join("cargo.stdout"), without("build-finished"))?;
        assert!(!stock(&f, &[]), "an interrupted build");
        std::fs::write(
            f.paths.run.join("cargo.stdout"),
            format!("{output}{{\"reason\":\"build-finished\",\"success\":false}}\n"),
        )?;
        assert!(!stock(&f, &[]), "two builds");
        std::fs::write(
            f.paths.run.join("cargo.stdout"),
            output.replace(
                "\"build-finished\",\"success\":false",
                "\"build-finished\",\"success\":true",
            ),
        )?;
        assert!(!stock(&f, &[]), "a successful build contradicts the exit");
        std::fs::write(f.paths.run.join("cargo.stdout"), &output)?;
        std::fs::write(&artifacts, without("build-script-executed"))?;
        assert!(!stock(&f, &[]), "a recorded script that Cargo did not run");
        std::fs::write(&artifacts, &output)?;
        assert!(stock(&f, &[]));
        let invocation = |f: &NegativeGeneration, call: &Invocation| {
            std::fs::write(
                f.paths
                    .run
                    .join("invocations")
                    .join(format!("{}.json", call.identity.key())),
                serde_json::to_vec(call).unwrap_or_default(),
            )
        };
        let original_call = f.call.clone();
        for field in ["crash", "error", "success"] {
            let mut call = original_call.clone();
            match field {
                "crash" => call.report.exit_code = Some(101),
                "error" => {
                    call.report.error =
                        crate::report::setup_failure(&[], "SAIL3100", "fixture").error;
                }
                _ => call.success = true,
            }
            invocation(&f, &call)?;
            assert!(!stock(&f, &[]), "selected driver {field}");
        }
        invocation(&f, &original_call)?;
        assert!(stock(&f, &[]));
        // A dependency verified in this run must have succeeded.
        let mut dependency = original_call.clone();
        dependency.identity.name = "ordinary_dep".to_owned();
        invocation(&f, &dependency)?;
        std::fs::write(
            f.paths.run.join("invocations/2.started"),
            b"{\"run_id\":\"run\",\"target\":\"ordinary_dep\"}",
        )?;
        assert!(!stock(&f, &[]), "a failed dependency");
        std::fs::remove_file(
            f.paths
                .run
                .join("invocations")
                .join(format!("{}.json", dependency.identity.key())),
        )?;
        std::fs::remove_file(f.paths.run.join("invocations/2.started"))?;
        assert!(stock(&f, &[]));
        // A selected target that Cargo never started may have dependencies
        // that did not complete.
        let package = f
            .expected
            .first()
            .map(|(id, _, _)| id.clone())
            .unwrap_or_default();
        f.expected
            .insert((package, "unstarted".to_owned(), "bin".to_owned()));
        assert!(!stock(&f, &[]), "an unstarted selected target");
        f.expected.retain(|(_, name, _)| name != "unstarted");
        assert!(stock(&f, &[]));
        assert!(!stock(
            &f,
            &["--".to_owned(), "--verify-function=selected".to_owned()]
        ));
        for currentness in [Currentness::Historical, Currentness::Unknown] {
            f.result.currentness = currentness;
            assert!(!stock(&f, &[]), "only a Current run retains contracts");
        }
        f.result.currentness = Currentness::Current;
        // The first run of a project establishes the script declarations
        // (issue #123): the same Cargo evidence keeps them.
        let establishing = |f: &NegativeGeneration| {
            negative_compiler_generation(
                &f.paths,
                &f.data,
                &f.expected,
                &f.result,
                false,
                false,
                &[],
            )
        };
        assert!(establishing(&f));
        std::fs::write(&artifacts, without("build-script-executed"))?;
        assert!(
            !establishing(&f),
            "a recorded script that Cargo did not run"
        );
        std::fs::write(&artifacts, &output)?;
        std::fs::write(f.paths.run.join("cargo.stdout"), without("build-finished"))?;
        assert!(!establishing(&f), "an interrupted establishing build");
        Ok(())
    }

    #[test]
    fn shared_fresh_artifact_cannot_replace_a_focus_context_proof()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-compiler-proof-{}", std::process::id()));
        let config = compiler_fixture(&root)?;
        let (paths, ownership) = prepare(&config, &root, "missing", "compiler", "run")?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='input'\nversion='0.1.0'\n",
        )?;
        std::fs::write(root.join("lib.rs"), "pub fn input() {}")?;
        let data = serde_json::json!({"workspace_root":root,
            "sail_cache_directory":config.cache, "target_directory":paths.target,
            "workspace_members":["input"], "resolve":{"nodes":[{"id":"input","dependencies":[]}]},
            "packages":[{"id":"input","name":"input","manifest_path":root.join("Cargo.toml"),
                "source":null,"metadata":{"verus":{"verify":true}},
                "targets":[{"name":"input","kind":["lib"],"src_path":root.join("lib.rs")}]}]});
        std::fs::create_dir_all(&paths.target)?;
        let artifact = paths.target.join("input.rlib");
        std::fs::write(&artifact, "cached compiler artifact")?;
        std::fs::write(
            paths.run.join("artifacts.jsonl"),
            serde_json::to_vec(&serde_json::json!({
                "reason":"compiler-artifact", "package_id":"input", "fresh":true,
                "target":{"name":"input","kind":["lib"]}, "filenames":[artifact]
            }))?,
        )?;
        let expected = BTreeSet::from([("input".to_owned(), "input".to_owned(), "lib".to_owned())]);
        let (targets, missing) = target_records(&paths, &data, "missing", false, &expected, &[])?;
        assert_eq!(targets, Vec::<TargetResult>::new());
        assert_eq!(missing, BTreeSet::from(["input".to_owned()]));
        assert_eq!(std::fs::read_dir(&paths.receipts)?.count(), 0);
        assert_eq!(
            std::fs::read_dir(paths.run.join("pending-receipts"))?.count(),
            0
        );
        // Even an intact old focus receipt with identical inputs/artifacts
        // cannot authorize another selector's skipped proof.
        let receipt = compiler_receipt(&paths, &data, &artifact);
        assert_ne!(receipt.input_revision, "");
        let path = receipt_path(&paths, "input", "input", &["lib".to_owned()]);
        json(
            &paths
                .run
                .join("pending-receipts")
                .join(path.file_name().ok_or("receipt name")?),
            &receipt,
        )?;
        super::super::receipts::publish(&paths.run, &paths.receipts, "selected")?;
        assert!(receipt.valid("selected"));
        assert_eq!(receipt.input_revision, target_revision(&data, "input"));
        let (targets, missing) = target_records(&paths, &data, "selected", false, &expected, &[])?;
        assert_eq!(targets.len(), 1);
        assert!(targets[0].reused);
        assert!(missing.is_empty());
        for (context, force) in [("missing", false), ("selected", true)] {
            let (targets, missing) = target_records(&paths, &data, context, force, &expected, &[])?;
            assert_eq!(targets, Vec::<TargetResult>::new());
            assert_eq!(missing, BTreeSet::from(["input".to_owned()]));
        }
        drop(ownership);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn shipped_proof_origin_survives_receipt_and_store_reuse()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-shipped-origin-{}", std::process::id()));
        let config = compiler_fixture(&root)?;
        let (paths, ownership) = prepare(&config, &root, "selected", "compiler", "run")?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='input'\nversion='0.1.0'\n",
        )?;
        std::fs::write(root.join("lib.rs"), "pub fn input() {}")?;
        let data = serde_json::json!({"workspace_root":root,
            "sail_cache_directory":config.cache, "target_directory":paths.target,
            "workspace_members":[], "resolve":{"nodes":[{"id":"input","dependencies":[]}]},
            "packages":[{"id":"input","name":"input","manifest_path":root.join("Cargo.toml"),
                "source":"registry+https://github.com/rust-lang/crates.io-index",
                "metadata":{"verus":{"verify":true}},
                "targets":[{"name":"input","kind":["lib"],"src_path":root.join("lib.rs")}]}]});
        std::fs::create_dir_all(&paths.target)?;
        let artifact = paths.target.join("input.rlib");
        std::fs::write(&artifact, "shipped artifact")?;
        let artifact_record = |fresh| {
            serde_json::json!({
                "reason":"compiler-artifact", "package_id":"input", "fresh":fresh,
                "target":{"name":"input","kind":["lib"]}, "filenames":[artifact]
            })
        };
        json(&paths.run.join("artifacts.jsonl"), &artifact_record(false))?;
        let mut call = compiler_receipt(&paths, &data, &artifact).invocation;
        call.dependency_build = Some(sail_api::project::DependencyBuildOrigin {
            kind: sail_api::project::DependencyBuildKind::Release,
            identity: "build/entry".to_owned(),
            establishing_run: "shipped:build/entry".to_owned(),
        });
        let invocation = paths.run.join("invocations/call.json");
        json(&invocation, &call)?;
        let empty = BTreeSet::new();
        let (first, missing) = target_records(&paths, &data, "selected", false, &empty, &[])?;
        assert_eq!(missing, BTreeSet::<String>::new());
        assert_eq!(first.len(), 1);
        assert!(!first[0].reused);
        assert_eq!(first[0].establishing_run, "shipped:build/entry");
        assert_eq!(first[0].dependency_build, call.dependency_build);
        super::super::receipts::publish(&paths.run, &paths.receipts, "selected")?;
        std::fs::remove_file(invocation)?;
        json(&paths.run.join("artifacts.jsonl"), &artifact_record(true))?;
        let (reused, missing) = target_records(&paths, &data, "selected", false, &empty, &[])?;
        assert_eq!(missing, BTreeSet::<String>::new());
        assert_eq!(reused.len(), 1);
        assert!(reused[0].reused);
        assert_eq!(reused[0].establishing_run, first[0].establishing_run);
        assert_eq!(reused[0].dependency_build, first[0].dependency_build);
        let path = receipt_path(&paths, "input", "input", &["lib".to_owned()]);
        let receipt: Receipt = read(&path).ok_or("missing receipt")?;
        assert_eq!(receipt.invocation.run_id, "run");
        assert!(receipt.valid("selected"));
        std::fs::remove_file(&path)?;
        let seeds = [super::super::store::Seeded {
            key: "store-key".into(),
            receipt,
            placed: true,
            shown_stdout: Vec::new(),
        }];
        let (stored, missing) = target_records(&paths, &data, "other", false, &empty, &seeds)?;
        assert_eq!(missing, BTreeSet::<String>::new());
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].dependency_store.as_deref(), Some("store-key"));
        assert_eq!(stored[0].establishing_run, first[0].establishing_run);
        assert_eq!(stored[0].dependency_build, first[0].dependency_build);
        assert_eq!(
            seeds[0].receipt.invocation.establishing_run(),
            stored[0].establishing_run
        );
        super::super::receipts::adopt(
            &paths.run,
            &paths.receipts,
            path.file_name().ok_or("receipt name")?,
            "other",
            target_revision(&data, "input"),
            seeds[0].receipt.clone(),
        )?;
        let (adopted, _) = target_records(&paths, &data, "other", false, &empty, &[])?;
        assert_eq!(adopted.len(), 1);
        assert_eq!(adopted[0].establishing_run, first[0].establishing_run);
        assert_eq!(adopted[0].dependency_build, first[0].dependency_build);
        drop(ownership);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn runtime_identity_compares_stats_of_installed_files_and_bytes_of_configuration()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-runtime-stat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root)?;
        let installed = root.join("rustc");
        let configuration = root.join("config.toml");
        std::fs::write(&installed, b"binary")?;
        std::fs::write(&configuration, b"[build]")?;
        let identities = vec![
            (
                installed.to_string_lossy().into_owned(),
                super::super::files::stat_identity(&installed)?,
            ),
            (
                configuration.to_string_lossy().into_owned(),
                file_digest(&configuration)?,
            ),
        ];
        let state = root.join("runtime-identities");
        std::fs::create_dir_all(&state)?;
        let sha = digest(&serde_json::to_vec(&identities)?);
        json(&state.join(format!("{sha}.json")), &identities)?;
        json(&state.join("ctx.link"), &sha)?;
        assert!(runtime_current(&root, "ctx"));
        // A rewrite that changes the size or modification time changes it.
        std::fs::write(&installed, b"binary, rebuilt")?;
        assert!(!runtime_current(&root, "ctx"));
        json(
            &state.join(format!("{sha}.json")),
            &vec![
                identities[0].clone(),
                (identities[1].0.clone(), "forged".to_owned()),
            ],
        )?;
        assert!(
            !runtime_current(&root, "ctx"),
            "a changed record never matches"
        );
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn corrupt_engine_is_rejected_before_its_version_probe()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        use crate::source_build::{Manifest, SCHEMA_VERSION, artifact_hashes, checkout_dir};
        use std::os::unix::fs::PermissionsExt as _;
        let root =
            std::env::temp_dir().join(format!("sail-engine-no-probe-{}", std::process::id()));
        let runtime = checkout_dir(&root).join("source/target-verus/release");
        std::fs::create_dir_all(&runtime)?;
        let marker = root.join("engine-executed");
        for name in ["cargo-verus", "verus", "rust_verify", "z3", "vstd.vir"] {
            std::fs::write(runtime.join(name), name)?;
        }
        let launcher = runtime.join("verus");
        std::fs::write(
            &launcher,
            format!("#!/bin/sh\ntouch '{}'\nexit 1\n", marker.display()),
        )?;
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o700))?;
        let pins = crate::toolchain::Pins::embedded()?;
        let manifest = Manifest {
            schema_version: SCHEMA_VERSION,
            commit: pins.verus.commit.clone(),
            rust_toolchain: pins.verus.rust_toolchain.clone(),
            z3_sha256: pins
                .assets
                .get(crate::toolchain::host())
                .ok_or("host pin")?
                .z3_sha256
                .clone(),
            features: crate::engine::features(),
            artifacts: artifact_hashes(&root)?,
            build_seconds: 0,
            test_binaries: Vec::new(),
        };
        manifest.save(&root)?;
        let path = root.join(crate::source_build::MANIFEST);
        let request = crate::engine::Request {
            function: None,
            cargo_verus: runtime.join("cargo-verus"),
            manifest_sha256: file_digest(&path)?,
            manifest: path,
            stock_wrapper: launcher.clone(),
            artifacts: manifest.artifacts.keys().map(|p| root.join(p)).collect(),
        };
        let config = Config {
            workspace: root.clone(),
            cache: root.join("cache"),
            sail: launcher.clone(),
            cargo_verus: request.cargo_verus.clone(),
            cargo: launcher.clone(),
            cargo_toolchain: crate::managed::CargoToolchain::Caller,
            shim: launcher.clone(),
            z3: runtime.join("z3"),
            socket: None,
            args: Vec::new(),
            force: true,
            attribution_logs: true,
        };
        std::fs::write(runtime.join("z3"), b"tampered")?;
        let error = context(&config, Some(&request))
            .err()
            .ok_or("corrupt engine was accepted")?;
        assert_eq!(error.code, "SAIL3120");
        assert!(
            !marker.exists(),
            "unvalidated engine was executed for discovery"
        );
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn rebinding_declared_external_inputs_preserves_seals_and_rejects_corruption() {
        let root = std::env::temp_dir().join(format!(
            "sail-receipt-completed-inputs-{}",
            std::process::id()
        ));
        let app = root.join("app");
        let dependency = root.join("vstd");
        let run = root.join("run");
        for path in [&app, &dependency, &run.join("pending-receipts")] {
            assert!(std::fs::create_dir_all(path).is_ok());
        }
        let external = app.join("shared.rs");
        assert!(std::fs::write(&external, "consumed by vstd").is_ok());
        assert!(std::fs::write(app.join("Cargo.toml"), "app").is_ok());
        assert!(std::fs::write(dependency.join("Cargo.toml"), "dependency").is_ok());
        let mut data = serde_json::json!({"workspace_root":app,
            "packages":[{"id":"app","manifest_path":app.join("Cargo.toml")},
                        {"id":"vstd","manifest_path":dependency.join("Cargo.toml")}],
            "resolve":{"nodes":[{"id":"vstd","dependencies":[]}]}});
        let before = snapshot_metadata(&data, &[]);
        let receipt = Receipt {
            version: RECEIPT_VERSION,
            checksum: String::new(),
            package_id: "vstd".to_owned(),
            name: "vstd".to_owned(),
            kind: vec!["lib".to_owned()],
            context: "context".to_owned(),
            input_revision: target_revision(&data, "vstd"),
            invocation: Invocation {
                retained_from: None,
                identity: super::super::receipts::TargetIdentity {
                    manifest: dependency.join("Cargo.toml"),
                    name: "vstd".to_owned(),
                    crate_type: "lib".to_owned(),
                    rustc_args_sha256: "args".to_owned(),
                },
                run_id: "run".to_owned(),
                report: crate::report::setup_failure(&[], "SAIL3100", "fixture"),
                success: true,
                dependencies: BTreeMap::new(),
                dependency_build: None,
            },
            artifact_hashes: BTreeMap::new(),
            committed: run.join("committed.json"),
            declared_environment: BTreeMap::new(),
        }
        .seal();
        let path = run.join("pending-receipts/vstd.json");
        assert!(json(&path, &receipt).is_ok());
        data["sail_source_inputs"] = serde_json::json!([external]);
        data["sail_source_inputs_by_package"] = serde_json::json!({"vstd":[external]});
        assert_eq!(before.revision, snapshot_metadata(&data, &[]).revision);
        let rebound =
            completed_receipts(&run, &data).unwrap_or_else(|e| unreachable!("fixture: {e}"));
        assert_eq!(rebound.len(), 1);
        assert_ne!(rebound[0].1.input_revision, receipt.input_revision);
        assert_eq!(rebound[0].1.input_revision, target_revision(&data, "vstd"));
        assert_eq!(rebound[0].1.clone().seal().checksum, rebound[0].1.checksum);
        assert_eq!(
            read::<Receipt>(&path).map(|r| r.checksum),
            Some(receipt.checksum.clone())
        );
        let mut corrupt = receipt;
        corrupt.invocation.report.mode = "corrupted".to_owned();
        assert!(json(&path, &corrupt).is_ok());
        assert!(completed_receipts(&run, &data).is_err());
        assert!(!run.join("committed.json").exists());
        assert!(std::fs::remove_dir_all(root).is_ok());
    }

    /// Issue #120: serde's build script writes `OUT_DIR/private.rs`, which
    /// serde compiles with `include!`. A run that cleans and reruns the script
    /// rewrites the file (new inode and stamps, same bytes), and a cold run
    /// discovers it. Neither is an input change: the comparison across the
    /// run leaves out the managed target, while the complete snapshot (for
    /// receipts and read checks) still binds the generated content.
    #[cfg(unix)]
    #[test]
    fn generated_target_files_are_left_out_of_the_comparison_across_a_run()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-generated-{}", std::process::id()));
        let package = root.join("serde");
        let target = root.join("cache/target");
        let out = target.join("debug/build/serde-1/out");
        std::fs::create_dir_all(&package)?;
        std::fs::create_dir_all(&out)?;
        let root = root.canonicalize()?;
        let (package, target, out) = (
            root.join("serde"),
            root.join("cache/target"),
            root.join("cache/target/debug/build/serde-1/out"),
        );
        std::fs::write(package.join("Cargo.toml"), "manifest")?;
        let source = package.join("lib.rs");
        std::fs::write(
            &source,
            "include!(concat!(env!(\"OUT_DIR\"), \"/private.rs\"));",
        )?;
        let generated = out.join("private.rs");
        std::fs::write(&generated, "pub mod private {}")?;
        let mut data = serde_json::json!({
            "workspace_root":package, "sail_cache_directory":root.join("cache"),
            "packages":[{"id":"serde","manifest_path":package.join("Cargo.toml")}],
        });
        // A cold run: the include is discovered only after the run.
        let cold = snapshot_metadata(&data, &[]);
        data["sail_source_inputs"] = serde_json::json!([source, generated]);
        let discovered = snapshot_metadata(&data, &[]);
        assert_ne!(cold.revision, discovered.revision);
        let cold_view = without_generated(&cold, Some(&target));
        let discovered_view = without_generated(&discovered, Some(&target));
        assert_eq!(cold_view.digests, discovered_view.digests);
        assert!(
            discovered.digests.contains_key(&generated),
            "content stays bound"
        );
        // A rerun script replaces the file with the same bytes.
        let before = snapshot_metadata(&data, &[]);
        let before_view = without_generated(&before, Some(&target));
        let stamp = super::super::input::mutation_stamp(&before_view);
        let complete_stamp = super::super::input::mutation_stamp(&before);
        let replacement = out.join("private.rs.new");
        std::fs::write(&replacement, "pub mod private {}")?;
        std::fs::rename(&replacement, &generated)?;
        let after = snapshot_metadata(&data, &[]);
        let after_view = without_generated(&after, Some(&target));
        assert_eq!(before_view.digests, after_view.digests);
        assert_eq!(stamp, super::super::input::mutation_stamp(&after_view));
        assert_ne!(
            complete_stamp,
            super::super::input::mutation_stamp(&after),
            "the complete snapshot sees the rewrite"
        );
        // Changed generated bytes stay visible to receipts and read checks:
        // the package's receipt revision binds its generated include.
        data["sail_source_inputs_by_package"] =
            serde_json::json!({"serde": [source.clone(), generated.clone()]});
        let bound = target_revision(&data, "serde");
        assert_ne!(bound, String::new());
        std::fs::write(&generated, "pub mod private { pub fn changed() {} }")?;
        let changed = snapshot_metadata(&data, &[]);
        assert_ne!(
            changed.digests.get(&generated),
            after.digests.get(&generated)
        );
        assert_ne!(
            target_revision(&data, "serde"),
            bound,
            "a receipt goes dirty"
        );
        std::fs::write(&generated, "pub mod private {}")?;
        assert_eq!(
            target_revision(&data, "serde"),
            bound,
            "same bytes, same binding"
        );
        data["sail_source_inputs_by_package"] = serde_json::json!({"serde": [source.clone()]});
        assert_ne!(
            target_revision(&data, "serde"),
            bound,
            "the include is part of the binding"
        );
        // Every input outside the target is still compared, by content and stamp.
        std::fs::write(
            &source,
            "include!(concat!(env!(\"OUT_DIR\"), \"/other.rs\"));",
        )?;
        let edited = without_generated(&snapshot_metadata(&data, &[]), Some(&target));
        assert_ne!(edited.digests, after_view.digests);
        assert_ne!(stamp, super::super::input::mutation_stamp(&edited));
        // Without a canonical target nothing is left out.
        assert_eq!(without_generated(&after, None).digests, after.digests);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    /// The snapshot taken from the projection of the metadata before a run
    /// is the snapshot of any metadata that selects the same packages.
    #[test]
    fn projected_snapshot_is_the_snapshot_of_the_same_selection() {
        let root =
            std::env::temp_dir().join(format!("sail-projected-snapshot-{}", std::process::id()));
        let app = root.join("app");
        let dependency = root.join("registry/dep");
        let target = app.join("target");
        for path in [&app, &dependency, &target] {
            assert!(std::fs::create_dir_all(path.join("src")).is_ok());
            assert!(std::fs::write(path.join("Cargo.toml"), "manifest").is_ok());
            assert!(std::fs::write(path.join("src/lib.rs"), "source").is_ok());
        }
        let generated = root.join("generated.rs");
        assert!(std::fs::write(&generated, "generated").is_ok());
        let sources = serde_json::json!([generated]);
        let full = |extra: &str| {
            serde_json::json!({
                "workspace_root":app,"target_directory":target,
                "sail_invocation_directory":app,"sail_cache_directory":root.join("cache"),
                "sail_source_inputs":sources,"version":extra,
                "packages":[{"id":"app","name":"app","manifest_path":app.join("Cargo.toml"),
                             "targets":[{"kind":["lib"]}],"source":null},
                            {"id":"dep","name":"dep","manifest_path":dependency.join("Cargo.toml"),
                             "source":"registry"}],
                "resolve":{"nodes":[{"id":"app","dependencies":["dep"]}]}
            })
        };
        let before = full("before");
        let after = full("after");
        assert!(same_snapshot_selection(&before, &after));
        let extras = vec![root.join("registry")];
        let expected = snapshot_metadata(&after, &extras);
        let projected = snapshot_metadata(&snapshot_projection(&before, &sources), &extras);
        assert!(expected.complete && projected.complete);
        assert_eq!(projected.revision, expected.revision);
        assert_eq!(projected.digests, expected.digests);
        assert_eq!(projected.observation_paths, expected.observation_paths);
        assert_eq!(projected.roots, expected.roots);
        assert_eq!(projected.excluded, expected.excluded);
        assert_eq!(projected.workspace, expected.workspace);
        // A snapshot taken earlier stands only while nothing it observed
        // changed, even when the bytes are rewritten unchanged.
        let (stamped, stamp) = super::super::input::snapshot_metadata_stamped(
            &snapshot_projection(&before, &sources),
            &extras,
        );
        assert!(super::super::input::unchanged_since(
            &stamped,
            stamp.as_ref()
        ));
        assert!(std::fs::write(app.join("src/lib.rs"), "source").is_ok());
        assert!(!super::super::input::unchanged_since(
            &stamped,
            stamp.as_ref()
        ));
        assert!(!super::super::input::unchanged_since(&stamped, None));
        let mut fewer = after.clone();
        if let Some(packages) = fewer["packages"].as_array_mut() {
            packages.pop();
        }
        assert!(!same_snapshot_selection(&before, &fewer));
        let mut moved = after;
        moved["target_directory"] = Value::String("elsewhere".into());
        assert!(!same_snapshot_selection(&before, &moved));
        assert!(std::fs::remove_dir_all(root).is_ok());
    }

    /// `target_revision` as it was before it copied only the snapshot's
    /// fields: the whole metadata, narrowed to the closure's packages.
    fn whole_metadata_revision(data: &Value, package_id: &str) -> String {
        let closure = dependency_closure(data, [package_id]);
        let mut package_data = data.clone();
        let mut packages: Vec<Value> = data["packages"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|p| p["id"].as_str().is_some_and(|id| closure.contains(id)))
            .cloned()
            .collect();
        for package in &mut packages {
            package["source"] = Value::Null;
        }
        package_data["packages"] = Value::Array(packages);
        if let Some(observed) = data["sail_source_inputs_by_package"].as_object() {
            package_data["sail_source_inputs"] = Value::Array(
                closure
                    .iter()
                    .filter_map(|id| observed.get(id).and_then(Value::as_array))
                    .flatten()
                    .cloned()
                    .collect(),
            );
        }
        let snapshot = snapshot_metadata(&package_data, &[]);
        if snapshot.complete {
            snapshot.revision
        } else {
            String::new()
        }
    }

    #[test]
    fn discovered_workspace_sources_do_not_dirty_dependency_receipts() {
        let root =
            std::env::temp_dir().join(format!("sail-receipt-source-scope-{}", std::process::id()));
        let app = root.join("app");
        let dependency = root.join("registry/vstd");
        let target = root.join("target");
        for path in [&app, &dependency, &target] {
            assert!(std::fs::create_dir_all(path).is_ok());
            assert!(std::fs::write(path.join("Cargo.toml"), "manifest").is_ok());
        }
        let app_source = app.join("lib.rs");
        let dependency_source = dependency.join("lib.rs");
        assert!(std::fs::write(&app_source, "good").is_ok());
        assert!(std::fs::write(&dependency_source, "dependency").is_ok());
        let mut data = serde_json::json!({
            "workspace_root":app,"target_directory":target,
            "packages":[{"id":"app","manifest_path":app.join("Cargo.toml")},
                        {"id":"vstd","manifest_path":dependency.join("Cargo.toml")}],
            "resolve":{"nodes":[{"id":"app","dependencies":["vstd"]},
                                  {"id":"vstd","dependencies":[]}]}
        });
        let dependency_before = target_revision(&data, "vstd");
        let app_before = target_revision(&data, "app");
        let mut artifacts = Vec::new();
        for (id, source) in [("app", &app_source), ("vstd", &dependency_source)] {
            assert!(
                std::fs::write(
                    target.join(format!("{id}-123.d")),
                    format!("artifact.d: {}\n", source.display())
                )
                .is_ok()
            );
            artifacts.push(serde_json::json!({
                "reason":"compiler-artifact","package_id":id,
                "target":{"kind":["lib"]},
                "filenames":[target.join(format!("lib{id}-123.rmeta"))]
            }));
        }
        let artifact_path = root.join("artifacts.jsonl");
        assert!(
            std::fs::write(
                &artifact_path,
                artifacts
                    .iter()
                    .map(Value::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
            .is_ok()
        );
        let state = root.join("sources.json");
        let (paths, covered) = super::super::source_inputs::update(&state, &artifact_path, &data)
            .unwrap_or_else(|e| unreachable!("fixture: {e}"));
        assert!(covered && paths.len() == 2);
        data["sail_source_inputs"] = serde_json::to_value(paths).unwrap_or_default();
        data["sail_source_inputs_by_package"] =
            serde_json::to_value(super::super::source_inputs::packages(&state)).unwrap_or_default();
        assert_eq!(target_revision(&data, "vstd"), dependency_before);
        assert_eq!(target_revision(&data, "app"), app_before);
        for id in ["vstd", "app"] {
            assert_eq!(
                target_revision(&data, id),
                whole_metadata_revision(&data, id)
            );
        }
        assert!(std::fs::write(&app_source, "fail").is_ok());
        assert_eq!(target_revision(&data, "vstd"), dependency_before);
        assert_ne!(target_revision(&data, "app"), app_before);
        assert!(std::fs::write(&dependency_source, "changed dependency").is_ok());
        assert_ne!(target_revision(&data, "vstd"), dependency_before);
        assert_ne!(target_revision(&data, "app"), app_before);
        assert!(std::fs::remove_dir_all(root).is_ok());
    }

    /// Issue #177: a registry dependency's receipt (vstd) is bound to its
    /// resolved closure, its bytes and the `[profile]` table, not to the bytes
    /// of the workspace manifest and lockfile. Adding a dependency keeps it;
    /// anything that changes how its closure is built or resolved does not.
    #[test]
    fn a_new_workspace_dependency_keeps_a_registry_closure_receipt()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-foreign-closure-{}", std::process::id()));
        let app = root.join("app");
        let registry = root.join("registry");
        let local = root.join("local");
        for path in [&app, &registry.join("vstd"), &registry.join("syn"), &local] {
            std::fs::create_dir_all(path)?;
            std::fs::write(path.join("Cargo.toml"), "[package]\n")?;
            std::fs::write(path.join("lib.rs"), "source")?;
        }
        let manifest = app.join("Cargo.toml");
        let lockfile = app.join("Cargo.lock");
        std::fs::write(
            &manifest,
            "[package]\nname = \"app\"\n[dependencies]\nvstd = \"1\"\n",
        )?;
        std::fs::write(&lockfile, "version = 4\n")?;
        let source = "registry+https://github.com/rust-lang/crates.io-index";
        let mut data = serde_json::json!({
            "workspace_root": app, "workspace_members": ["app"],
            "packages": [
                {"id":"app","manifest_path":manifest},
                {"id":"vstd","source":source,"manifest_path":registry.join("vstd/Cargo.toml")},
                {"id":"syn","source":source,"manifest_path":registry.join("syn/Cargo.toml")},
                {"id":"local","manifest_path":local.join("Cargo.toml")}],
            "resolve": {"nodes": [
                {"id":"app","dependencies":["vstd","local"],"features":[]},
                {"id":"vstd","dependencies":["syn"],"features":["std"]},
                {"id":"syn","dependencies":[],"features":["full"]},
                {"id":"local","dependencies":[],"features":[]}]}
        });
        let vstd = target_revision(&data, "vstd");
        let app_revision = target_revision(&data, "app");
        let local_revision = target_revision(&data, "local");
        assert!(!vstd.is_empty() && !app_revision.is_empty() && !local_revision.is_empty());

        // The crate gains a registry dependency: new manifest and lockfile
        // bytes and a new package and node outside vstd's closure.
        std::fs::write(
            &manifest,
            "[package]\nname = \"app\"\n[dependencies]\nvstd = \"1\"\nserde = \"1\"\n",
        )?;
        std::fs::write(&lockfile, "version = 4\n[[package]]\nname = \"serde\"\n")?;
        std::fs::create_dir_all(registry.join("serde"))?;
        std::fs::write(registry.join("serde/Cargo.toml"), "[package]\n")?;
        data["packages"].as_array_mut().ok_or("packages")?.push(
            serde_json::json!({"id":"serde","source":source,"manifest_path":registry.join("serde/Cargo.toml")}),
        );
        data["resolve"]["nodes"]
            .as_array_mut()
            .ok_or("nodes")?
            .push(serde_json::json!({"id":"serde","dependencies":[],"features":[]}));
        data["resolve"]["nodes"][0]["dependencies"] = serde_json::json!(["vstd", "local", "serde"]);
        assert_eq!(
            target_revision(&data, "vstd"),
            vstd,
            "vstd keeps its receipt"
        );
        assert_ne!(
            target_revision(&data, "app"),
            app_revision,
            "the crate is verified again"
        );
        // A path dependency's closure is per workspace: it stays bound to the
        // workspace manifest and lockfile bytes, as before.
        assert_ne!(target_revision(&data, "local"), local_revision);

        // Anything that decides how vstd's closure is built changes its binding.
        let reused = target_revision(&data, "vstd");
        let changed = |data: &Value| target_revision(data, "vstd") != reused;
        let mut features = data.clone();
        features["resolve"]["nodes"][2]["features"] = serde_json::json!(["full", "extra-traits"]);
        assert!(changed(&features), "a closure package gains a feature");
        let mut version = data.clone();
        version["resolve"]["nodes"][1]["dependencies"] = serde_json::json!(["syn2"]);
        assert!(changed(&version), "the closure resolves differently");
        std::fs::write(registry.join("syn/lib.rs"), "edited")?;
        assert!(changed(&data), "a closure source is edited");
        std::fs::write(registry.join("syn/lib.rs"), "source")?;
        assert!(!changed(&data));
        let text = std::fs::read_to_string(&manifest)?;
        std::fs::write(&manifest, format!("{text}[profile.dev]\nopt-level = 1\n"))?;
        assert!(changed(&data), "the workspace profile changes");
        std::fs::write(&manifest, &text)?;
        assert!(!changed(&data));
        std::fs::write(app.join("rust-toolchain.toml"), "[toolchain]\n")?;
        assert!(changed(&data), "a toolchain file appears");
        std::fs::remove_file(app.join("rust-toolchain.toml"))?;
        assert!(!changed(&data));
        // A workspace manifest that cannot be read binds nothing reusable.
        std::fs::write(&manifest, "not toml [")?;
        assert!(changed(&data));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn expected_target_coverage_respects_packages_and_target_selection() {
        let data = serde_json::json!({"workspace_members":["a","b"],"workspace_default_members":["a","b"],"resolve":{"root":null,"nodes":[]},"packages":[
            {"id":"a","name":"a","metadata":{"verus":{"verify":true}},"targets":[{"name":"a","kind":["lib"]},{"name":"a","kind":["bin"]}]},
            {"id":"b","name":"b","metadata":{"verus":{"verify":true}},"targets":[{"name":"b","kind":["lib"]}]}
        ]});
        assert_eq!(expected_targets(&data, &[]).len(), 3);
        assert_eq!(
            expected_targets(
                &data,
                &["-p".to_owned(), "a".to_owned(), "--lib".to_owned()]
            ),
            BTreeSet::from([("a".to_owned(), "a".to_owned(), "lib".to_owned())])
        );
        assert_eq!(
            expected_targets(&data, &["--bin".to_owned(), "a".to_owned()]),
            BTreeSet::from([("a".to_owned(), "a".to_owned(), "bin".to_owned())])
        );
        assert!(expected_targets(&data, &["-p".to_owned(), "missing".to_owned()]).is_empty());
    }

    #[test]
    fn compact_package_specs_bind_expected_targets_and_forced_cleanup() {
        let data = serde_json::json!({"workspace_members":["a","b"],
            "workspace_default_members":["a"],"resolve":{"root":null,"nodes":[]},
            "packages":[
                {"id":"a","name":"a","version":"1.0.0",
                    "metadata":{"verus":{"verify":true}},
                    "targets":[{"name":"a","kind":["lib"]}]},
                {"id":"b","name":"b","version":"2.0.0",
                    "metadata":{"verus":{"verify":true}},
                    "targets":[{"name":"b","kind":["lib"]}]}]});
        let names = BTreeSet::from(["a".to_owned(), "b".to_owned()]);
        let wanted = BTreeSet::from([("b".to_owned(), "b".to_owned(), "lib".to_owned())]);
        for (argument, spec) in [("-pb", "b"), ("-p=b", "b"), ("-pb@2.0.0", "b@2.0.0")] {
            let args = vec![argument.to_owned(), "--lib".to_owned()];
            assert_eq!(expected_targets(&data, &args), wanted);
            assert_eq!(selected(&args, &names), vec![spec.to_owned()]);
        }
        assert_eq!(
            expected_targets(&data, &[]),
            BTreeSet::from([("a".to_owned(), "a".to_owned(), "lib".to_owned())])
        );
        let repeated = vec!["-pb".to_owned(), "--package=a".to_owned()];
        assert_eq!(expected_targets(&data, &repeated).len(), 2);
        assert_eq!(
            selected(&repeated, &names),
            vec!["b".to_owned(), "a".to_owned()]
        );
        let separated = vec!["-pb".to_owned(), "--".to_owned(), "-pa".to_owned()];
        assert_eq!(expected_targets(&data, &separated), wanted);
        assert_eq!(selected(&separated, &names), vec!["b".to_owned()]);
        assert!(expected_targets(&data, &["-punknown".to_owned()]).is_empty());
    }
    #[test]
    fn focused_proof_selection_is_never_whole_target_coverage() {
        assert!(focused(&[
            "--".to_owned(),
            "--verify-function=lemma".to_owned()
        ]));
        assert!(!focused(&["--features".to_owned(), "foo".to_owned()]));
    }

    #[test]
    fn coherent_scope_is_the_selected_closure_and_cargo_must_stay_inside()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let data = serde_json::json!({"resolve": {"nodes": [
            {"id": "lib", "dependencies": ["vstd"]},
            {"id": "vstd", "dependencies": ["syn"]},
            {"id": "syn", "dependencies": []},
            {"id": "bench", "dependencies": ["lib", "criterion"]},
            {"id": "criterion", "dependencies": []}]}});
        let expected = BTreeSet::from([("lib".to_owned(), "lib".to_owned(), "lib".to_owned())]);
        let closure = selected_closure(&data, &expected);
        assert_eq!(
            closure,
            Some(BTreeSet::from(["lib", "syn", "vstd"].map(str::to_owned)))
        );
        assert_eq!(selected_closure(&data, &BTreeSet::new()), None);
        let root = std::env::temp_dir().join(format!("sail-scope-units-{}", std::process::id()));
        let config = compiler_fixture(&root)?;
        let (paths, ownership) = prepare(&config, &root, "scope", "compiler", "run")?;
        let unit = |reason: &str, id: &str| {
            serde_json::json!({"reason": reason, "package_id": id}).to_string() + "\n"
        };
        let mut journal = unit("compiler-artifact", "lib")
            + &unit("build-script-executed", "syn")
            + &unit("compiler-message", "bench");
        std::fs::write(paths.run.join("artifacts.jsonl"), &journal)?;
        assert!(artifacts_within(&paths, closure.as_ref()));
        journal += &unit("compiler-artifact", "criterion");
        std::fs::write(paths.run.join("artifacts.jsonl"), &journal)?;
        assert!(!artifacts_within(&paths, closure.as_ref()));
        assert!(artifacts_within(&paths, None));
        std::fs::write(paths.run.join("artifacts.jsonl"), "{not json\n")?;
        assert!(!artifacts_within(&paths, closure.as_ref()));
        std::fs::remove_file(paths.run.join("artifacts.jsonl"))?;
        assert!(!artifacts_within(&paths, closure.as_ref()));
        drop(ownership);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    /// Regression for the focused `SAIL3107`: `cargo verus focus` builds in
    /// `<target>/verus-partial` with checksum freshness, so byte-identical
    /// source is fresh there. The forced and retry cleanups must clean that
    /// directory, or no verifier runs and no receipt can ever appear.
    #[test]
    fn focused_cleanup_rebuilds_byte_identical_partial_source()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-focus-clean-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src"))?;
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='input'\nversion='0.1.0'\nedition='2024'\n[package.metadata.verus]\nverify=true\n[workspace]\n",
        )?;
        let source = root.join("src/lib.rs");
        let bytes = b"pub fn input() -> u32 { 1 }\n";
        std::fs::write(&source, bytes)?;
        let target = root.join("owned-target");
        let config = |args: &[&str]| Config {
            workspace: root.clone(),
            cache: root.join("cache"),
            sail: PathBuf::from("sail"),
            cargo_verus: PathBuf::from("cargo-verus"),
            cargo: PathBuf::from("cargo"),
            cargo_toolchain: crate::managed::CargoToolchain::Caller,
            shim: PathBuf::from("sail-z3"),
            z3: PathBuf::from("z3"),
            socket: None,
            args: args.iter().map(|&a| a.to_owned()).collect(),
            force: true,
            attribution_logs: true,
        };
        let focus = config(&["-p", "input", "--", "--verify-function", "input"]);
        let full = config(&["-p", "input"]);
        assert_eq!(
            cargo_build_target(&target, &focus.args),
            target.join("verus-partial")
        );
        assert_eq!(cargo_build_target(&target, &full.args), target);
        // What cargo-verus runs, minus the verifier wrapper: `verify` checks
        // in the owned target, `focus` in its `verus-partial` child.
        let check_in = |dir: &Path| -> std::result::Result<bool, Box<dyn std::error::Error>> {
            let output = Command::new("cargo")
                .current_dir(&root)
                .args(["check", "--offline", "-p", "input", "--message-format=json"])
                .arg("--target-dir")
                .arg(dir)
                .env("CARGO_UNSTABLE_CHECKSUM_FRESHNESS", "true")
                .env("RUSTC_BOOTSTRAP", "1")
                .env_remove("RUSTC_WRAPPER")
                .env_remove("RUSTC_WORKSPACE_WRAPPER")
                .output()?;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            output
                .stdout
                .split(|b| *b == b'\n')
                .filter_map(|line| serde_json::from_slice::<Value>(line).ok())
                .find(|r| r["reason"] == "compiler-artifact" && r["target"]["name"] == "input")
                .and_then(|r| r["fresh"].as_bool())
                .ok_or_else(|| "no artifact for the selected package".into())
        };
        let check = || check_in(&target.join("verus-partial"));
        // A full verification shares the owned target.
        assert!(!check_in(&target)?, "full build compiles");
        let rewrite = || -> std::io::Result<()> {
            // Byte-identical content with a later mtime: an edit that repeats
            // the content last compiled for this focus context.
            let file = std::fs::OpenOptions::new().write(true).open(&source)?;
            file.set_len(0)?;
            std::io::Write::write_all(&mut &file, bytes)?;
            let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
            file.set_times(std::fs::FileTimes::new().set_modified(later))
        };
        assert!(!check()?, "first focused build compiles");
        rewrite()?;
        assert!(check()?, "checksum freshness keeps repeated content fresh");
        // The old behaviour cleaned the outer target only; the build stays fresh.
        clean(&full, &target, &["input".to_owned()])?;
        rewrite()?;
        assert!(
            check()?,
            "cleaning the outer target cannot reach the focus build"
        );
        // The focused cleanup addresses the directory focus actually uses.
        clean(&focus, &target, &["input".to_owned()])?;
        assert!(!check()?, "focused cleanup must force a fresh compilation");
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
    #[test]
    fn lifecycle_only_job_environment_does_not_partition_proofs() {
        for key in [
            "SAIL_JOB_EVENTS",
            "SAIL_JOB_GUARD_PARENT",
            "SAIL_MANAGED_FORCE",
            "SAIL_MANAGED_PROXY",
        ] {
            assert!(!input_environment_key(std::ffi::OsStr::new(key)));
        }
        for key in ["RUSTFLAGS", "RUSTC", "SAIL_CACHE_DIR"] {
            assert!(input_environment_key(std::ffi::OsStr::new(key)));
        }
        let original = BTreeMap::from([("RUSTFLAGS", "-C opt-level=0")]);
        let mut observed = original.clone();
        observed.insert("SAIL_JOB_EVENTS", "events.jsonl");
        let inputs = |values: BTreeMap<&str, &str>| {
            values
                .into_iter()
                .filter(|(key, _)| input_environment_key(std::ffi::OsStr::new(key)))
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect::<BTreeMap<_, _>>()
        };
        assert_eq!(inputs(original), inputs(observed));
    }
    #[cfg(unix)]
    #[test]
    fn resolved_compiler_preserves_dispatch_alias_name() {
        let root =
            std::env::temp_dir().join(format!("sail-runtime-alias-test-{}", std::process::id()));
        assert!(std::fs::create_dir_all(&root).is_ok());
        let executable = root.join("rustup");
        let alias = root.join("rustc");
        assert!(std::fs::write(&executable, "fixture").is_ok());
        assert!(std::os::unix::fs::symlink(&executable, &alias).is_ok());
        let resolved = resolve_program(alias.as_os_str(), &root).unwrap_or_default();
        assert_eq!(resolved, alias, "execution must retain argv[0] role");
        assert_eq!(
            actual_wrapper(&alias).ok(),
            executable
                .canonicalize()
                .ok()
                .map(|p| p.with_file_name("verus"))
        );
        let mut hashes = super::super::files::FileDigests::default();
        assert_eq!(hashes.hash(&alias), hashes.hash(&executable));
        assert!(std::fs::write(&executable, "changed").is_ok());
        assert_ne!(
            hashes.hash(&alias),
            super::super::files::FileDigests::default().hash(&alias),
            "independent phases must re-read content"
        );
        assert!(std::fs::remove_dir_all(root).is_ok());
    }
}
