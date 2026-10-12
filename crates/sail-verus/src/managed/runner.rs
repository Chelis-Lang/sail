//! Owned Cargo invocation, artifact validation, and complete report reuse.

use super::diagnostics::{Diagnostics, Phase};
use super::files::{digest, event, file_digest, json};
use super::input::{metadata, snapshot_metadata};
use super::receipts::{Invocation, Receipt, Runtime, artifact_hashes, complete, read};
use crate::{Error, Result};
use sail_api::project::{Currentness, Execution, ProjectResult, SCHEMA_VERSION, TargetResult};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

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
}

fn input_environment_key(key: &std::ffi::OsStr) -> bool {
    ![
        "SAIL_PROOF_EVENTS",
        "SAIL_ENGINE_EVENTS",
        "SAIL_ENGINE_FOCUS_REPORT",
        "SAIL_ENGINE",
        "SAIL_ENGINE_FUNCTION",
        "SAIL_ENGINE_CONTEXT",
        "SAIL_JOB_EVENTS",
        "SAIL_JOB_RPC_TIMINGS",
        "SAIL_JOB_GUARD_PARENT",
        "SAIL_MANAGED_FORCE",
        "SAIL_DAEMON_SOCKET",
        "SAIL_PROVENANCE_DIR",
        "SAIL_TRACE_DIR",
        "SAIL_PROJECT_SOCKET",
        "PWD",
        "OLDPWD",
        "SHLVL",
        "_",
    ]
    .iter()
    .any(|excluded| key == *excluded)
        && !key.as_encoded_bytes().starts_with(b"SAIL_MANAGED_")
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
fn context(
    config: &Config,
    engine: Option<&crate::engine::Request>,
    diagnostics: &Diagnostics,
) -> Result<Context> {
    let binaries = diagnostics.phase(Phase::RuntimeBinaries);
    let mut identity = Vec::new();
    let mut hashed = super::files::FileDigests::default();
    if let Some(engine) = engine {
        // Shares only this phase's actual byte reads: no retained metadata or
        // digest cache can authorize a different invocation's engine.
        crate::engine::validate_request(engine, |path| hashed.hash(path))?;
    }
    for path in [
        &config.sail,
        &config.cargo_verus,
        &config.cargo,
        &config.z3,
        &config.shim,
    ] {
        identity.push((path.to_string_lossy().into_owned(), hashed.hash(path)?));
    }
    if let Some(engine) = engine {
        for path in [&engine.manifest, &engine.stock_wrapper]
            .into_iter()
            .chain(engine.artifacts.iter())
        {
            identity.push((path.to_string_lossy().into_owned(), hashed.hash(path)?));
        }
    }
    drop(binaries);
    let helper = diagnostics.phase(Phase::HelperIdentity);
    // Python is an optional Linux freshness dependency. Its exact interpreter,
    // loaded stdlib and ELF closure join the context; unavailable coverage still
    // permits fresh verification but the coherent guard cannot publish Current.
    #[cfg(target_os = "linux")]
    if let Some(helper) = super::lease::helper_identity() {
        identity.extend(helper);
    }
    drop(helper);
    let toolchains = diagnostics.phase(Phase::ToolchainProbes);
    let wrapper = actual_wrapper(&config.cargo_verus)?;
    identity.push((
        wrapper.to_string_lossy().into_owned(),
        hashed.hash(&wrapper)?,
    ));
    let mut environment: BTreeMap<Vec<u8>, Vec<u8>> = std::env::vars_os()
        .filter(|(k, _)| input_environment_key(k))
        .map(|(k, v)| (k.as_encoded_bytes().to_vec(), v.as_encoded_bytes().to_vec()))
        .collect();
    let rustc = resolve_program(
        &std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()),
        &config.workspace,
    )?;
    identity.push((rustc.to_string_lossy().into_owned(), hashed.hash(&rustc)?));
    let actual = Command::new(&rustc)
        .args(["--print", "sysroot"])
        .current_dir(&config.workspace)
        .output()
        .map_err(|e| Error::new("SAIL3104", format!("cannot identify Rust toolchain: {e}")))?;
    if !actual.status.success() {
        return Err(Error::new(
            "SAIL3104",
            String::from_utf8_lossy(&actual.stderr),
        ));
    }
    let rustup = resolve_program(std::ffi::OsStr::new("rustup"), &config.workspace)?;
    identity.push((rustup.to_string_lossy().into_owned(), hashed.hash(&rustup)?));
    let cargo_sysroot = PathBuf::from(String::from_utf8_lossy(&actual.stdout).trim());
    let verifier_sysroot = verifier_sysroot(&wrapper, &config.workspace)?;
    let installed_sysroots = BTreeSet::from([cargo_sysroot, verifier_sysroot]);
    identity.push((
        config
            .cargo_verus
            .with_file_name("rust_verify")
            .to_string_lossy()
            .into_owned(),
        hashed.hash(&wrapper.with_file_name("rust_verify"))?,
    ));
    drop(toolchains);
    let guard = diagnostics.phase(Phase::SysrootGuard);
    // Observe all installed-target namespaces before inventory collection and
    // acquire kernel leases before the first eligible byte read. Earlier hashes
    // remain ineligible: this owner cannot authorize a pre-guard digest.
    let mut sysroots =
        super::sysroot::Baseline::begin(&installed_sysroots, |path| hashed.contains(path));
    drop(guard);
    #[cfg(target_os = "linux")]
    let (handoff_scope, imported) = {
        let phase = diagnostics.phase(Phase::SysrootHandoff);
        let scope = config.socket.as_ref().and_then(|_| {
            let selection =
                super::sysroot_selection::selection(&config.args, engine, &identity, &environment)?;
            super::sysroot_cache::scope(&installed_sysroots, selection)
        });
        let imported = config
            .socket
            .as_ref()
            .zip(scope.as_ref())
            .is_some_and(|(socket, scope)| {
                let request = sail_api::sysroot::Request::Lookup {
                    scope: scope.clone(),
                };
                match sail_api::sysroot::call(socket, &request) {
                    Ok(sail_api::sysroot::Response::Lookup(Some(snapshot))) => {
                        sysroots.import(scope, &snapshot)
                    }
                    _ => false,
                }
            });
        drop(phase);
        (scope, imported)
    };
    let sysroot_bytes = diagnostics.phase(Phase::SysrootHash);
    for path in sysroots.paths().to_vec() {
        // Remote rows can populate only the independently protected Baseline,
        // never the phase-local FileDigests cache or pre-guard executable aliases.
        let hash = match sysroots.captured(&path) {
            Some(hash) => hash.to_owned(),
            None => hashed.hash(&path)?,
        };
        sysroots.capture(&path, &hash);
        identity.push((path.to_string_lossy().into_owned(), hash));
    }
    drop(sysroot_bytes);
    let barrier = diagnostics.phase(Phase::SysrootBarrier);
    let sysroots_complete = sysroots.complete();
    drop(barrier);
    #[cfg(target_os = "linux")]
    if sysroots_complete
        && !imported
        && let Some((socket, scope)) = config.socket.as_ref().zip(handoff_scope)
    {
        let phase = diagnostics.phase(Phase::SysrootFillHint);
        // Independent construction starts only after this caller's original
        // complete hash barrier. No request contains caller-computed digests.
        let _ = sail_api::sysroot::call(socket, &sail_api::sysroot::Request::Prepare { scope });
        drop(phase);
    }
    #[cfg(not(target_os = "linux"))]
    let _ = sysroots_complete;
    let configuration = diagnostics.phase(Phase::ContextConfiguration);
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
    let (hash, compiler_hash) = context_hashes(
        &config.args,
        engine,
        &identity,
        &environment.into_iter().collect::<Vec<_>>(),
    )?;
    super::files::json_metadata(&state.join(format!("{hash}.link")), &identity_sha)?;
    drop(configuration);
    Ok(Context {
        hash,
        compiler_hash,
        sysroots,
    })
}

struct Context {
    hash: String,
    compiler_hash: String,
    sysroots: super::sysroot::Baseline,
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
    // Only the driver-local focus selector is omitted. Compiler/solver bytes,
    // arguments, configuration and byte-exact environment remain bound. Cargo
    // freshness supplies ordinary artifacts; only exact proof receipts can
    // authorize skipped verification targets.
    let compiler_hash = digest(
        &serde_json::to_vec(&(
            "sail-managed-compiler-artifacts-v1",
            args,
            compiler_engine,
            identity,
            environment,
        ))
        .map_err(|e| Error::new("SAIL3104", e.to_string()))?,
    );
    Ok((hash, compiler_hash))
}

fn actual_wrapper(cargo_verus: &Path) -> Result<PathBuf> {
    // Cargo-Verus uses current_exe, not its invoked symlink's directory.
    cargo_verus
        .canonicalize()
        .map(|p| p.with_file_name("verus"))
        .map_err(|e| Error::new("SAIL3104", e.to_string()))
}

fn verifier_sysroot(wrapper: &Path, workspace: &Path) -> Result<PathBuf> {
    let output = Command::new(wrapper)
        .args(["--version", "--output-json"])
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
    if path.is_file() && file_digest(path)? == file_digest(original)? {
        return Ok(());
    }
    let _ = std::fs::remove_file(path);
    std::fs::hard_link(original, path)
        .or_else(|_| std::fs::copy(original, path).map(|_| ()))
        .map_err(|e| Error::new("SAIL3104", e.to_string()))
}

struct Ownership {
    _context: File,
    _compiler: File,
}

fn ownership_lock(directory: &Path) -> Result<File> {
    std::fs::create_dir_all(directory).map_err(|e| Error::new("SAIL3104", e.to_string()))?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join("ownership.lock"))
        .map_err(|e| Error::new("SAIL3104", e.to_string()))?;
    lock.try_lock()
        .map_err(|e| Error::new("SAIL3104", format!("managed target is in use: {e}")))?;
    Ok(lock)
}

fn prepare(
    config: &Config,
    workspace: &Path,
    hash: &str,
    compiler_hash: &str,
    run_id: &str,
    engine: Option<&crate::engine::Request>,
) -> Result<(Paths, Ownership)> {
    let root = config
        .cache
        .join("projects")
        .join(digest(workspace.as_os_str().as_encoded_bytes()));
    let context = root.join(hash);
    let compiler = root.join("compiler-artifacts").join(compiler_hash);
    // Both owners survive clean, invocation and receipt publication. Different
    // focus contexts cannot concurrently clean or modify the shared target.
    // Acquisition is nonblocking, always context then compiler, with no reverse
    // edge or change to the caller's deadline.
    let ownership = Ownership {
        _context: ownership_lock(&context)?,
        _compiler: ownership_lock(&compiler)?,
    };
    // A repair selector keeps its certified dependency bytes alive when another
    // selector verifies the same family. Ordinary and full-engine calls retain
    // their existing shared target; proof keys and family ownership are unchanged.
    let repair = engine.is_some_and(|request| request.function.is_some());
    let generation = if repair {
        compiler.join("repair-targets-v1").join(hash)
    } else {
        compiler.clone()
    };
    let observations = if repair {
        context.join("repair-observations-v1")
    } else {
        context.clone()
    };
    let paths = Paths {
        target: generation.join("target"),
        cargo_alias: compiler.join("sail-cargo"),
        proxy_alias: compiler.join("sail-verus-driver"),
        receipts: context.join("receipts"),
        runtime: context.join("runtime.json"),
        build_inputs: observations.join("build-inputs.json"),
        input_state: context.join("input-state.json"),
        compiler_inputs: generation.join("build-inputs.json"),
        run: context.join("runs").join(run_id),
    };
    for dir in [
        &observations,
        &paths.receipts,
        &paths.run,
        &paths.run.join("invocations"),
        &paths.run.join("sidecars"),
        &paths.run.join("pending-receipts"),
    ] {
        std::fs::create_dir_all(dir).map_err(|e| Error::new("SAIL3104", e.to_string()))?;
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

fn clean(config: &Config, target: &Path, packages: &[String]) -> Result<()> {
    if packages.is_empty() {
        return Ok(());
    }
    let mut cmd = Command::new(&config.cargo);
    cmd.current_dir(&config.workspace)
        .args(["clean", "--target-dir"])
        .arg(cargo_build_target(target, &config.args));
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

fn invoke(config: &Config, paths: &Paths, err: &mut dyn Write) -> Result<Option<i32>> {
    let split = config
        .args
        .iter()
        .position(|a| a == "--")
        .unwrap_or(config.args.len());
    let mut cmd = Command::new(&config.cargo_verus);
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
        .args(&config.args[split..])
        .env("RUSTC_BOOTSTRAP", "1")
        .env("CARGO", &paths.cargo_alias)
        .env("SAIL_MANAGED_CARGO", &config.cargo)
        .env("SAIL_MANAGED_PROXY", &paths.proxy_alias)
        .env("SAIL_MANAGED_RUNTIME", &paths.runtime)
        .env("SAIL_MANAGED_ARTIFACTS", paths.run.join("artifacts.jsonl"))
        .env("VERUS_Z3_PATH", &config.shim)
        .env("SAIL_Z3_PATH", &config.z3)
        .env("SAIL_SOLVER_MODE", "default")
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
    let mut child = cmd
        .spawn()
        .map_err(|e| Error::new("SAIL3106", e.to_string()))?;
    let stdout = child.stdout.take().map(|mut pipe| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).map(|_| bytes)
        })
    });
    let mut captured = Vec::new();
    if let Some(mut pipe) = child.stderr.take() {
        let mut buf = vec![0; 65536];
        loop {
            let n = pipe
                .read(&mut buf)
                .map_err(|e| Error::new("SAIL3106", e.to_string()))?;
            if n == 0 {
                break;
            }
            captured.extend_from_slice(&buf[..n]);
            err.write_all(&buf[..n])
                .map_err(|e| Error::new("SAIL3106", e.to_string()))?;
        }
    }
    let code = child
        .wait()
        .map_err(|e| Error::new("SAIL3106", e.to_string()))?
        .code();
    let stdout = stdout
        .ok_or_else(|| Error::new("SAIL3106", "missing Cargo output pipe"))?
        .join()
        .map_err(|_| Error::new("SAIL3106", "Cargo output reader failed"))?
        .map_err(|e| Error::new("SAIL3106", e.to_string()))?;
    std::fs::write(paths.run.join("cargo.stdout"), stdout)
        .and_then(|()| std::fs::write(paths.run.join("cargo.stderr"), captured))
        .map_err(|e| Error::new("SAIL3106", e.to_string()))?;
    Ok(code)
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
fn negative_compiler_generation(
    paths: &Paths,
    data: &Value,
    expected: &BTreeSet<(String, String, String)>,
    result: &ProjectResult,
    script_coverage: bool,
    engine: bool,
    args: &[String],
) -> bool {
    if !engine
        || !script_coverage
        || focused(args)
        || result.currentness != Currentness::Current
        || result.exit_code != Some(101)
        || result.error.is_some()
        || expected.is_empty()
    {
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
        let mut names: Vec<_> = calls
            .values()
            .map(|call| call.identity.name.clone())
            .collect();
        names.sort();
        starts.sort();
        if names.is_empty()
            || names != starts
            || !calls.values().any(|call| call.report.exit_code == Some(1))
            || !calls
                .values()
                .all(|call| crate::engine::completed_compiler_output(&paths.run, call))
        {
            return Some(false);
        }
        Some(expected.iter().all(|(id, name, kind)| {
            data["packages"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|package| {
                    package["id"] == *id
                        && calls
                            .values()
                            .filter(|call| {
                                package["manifest_path"].as_str().is_some_and(|manifest| {
                                    Path::new(manifest) == call.identity.manifest
                                }) && name.replace('-', "_") == call.identity.name
                                    && (kind == &call.identity.crate_type
                                        || (kind == "lib" && call.identity.crate_type == "rlib"))
                            })
                            .count()
                            == 1
                })
        }))
    };
    check() == Some(true)
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
) -> Result<(Vec<TargetResult>, BTreeSet<String>)> {
    let mut results = Vec::new();
    let mut missing = BTreeSet::new();
    let calls = invocations(paths);
    for artifact in artifacts(paths) {
        let Some(package) = data["packages"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|p| p["id"] == artifact["package_id"])
        else {
            continue;
        };
        if package
            .pointer("/metadata/verus/verify")
            .and_then(Value::as_bool)
            != Some(true)
        {
            continue;
        }
        if artifact["target"]["kind"]
            .as_array()
            .is_some_and(|k| k.iter().any(|v| v == "custom-build"))
        {
            continue;
        }
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
        let input_revision = target_revision(data, package_id);
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
            let hashes = artifact_hashes(&artifact_paths)?;
            let reusable = call.success && complete(&call.report) && !input_revision.is_empty();
            if reusable {
                json(
                    &paths
                        .run
                        .join("pending-receipts")
                        .join(path.file_name().unwrap_or_default()),
                    &Receipt {
                        version: 2,
                        committed: paths.run.join("committed.json"),
                        checksum: String::new(),
                        package_id: package_id.to_owned(),
                        name: name.to_owned(),
                        kind: kind.clone(),
                        context: context.to_owned(),
                        input_revision: input_revision.clone(),
                        invocation: call.clone(),
                        artifact_hashes: hashes,
                    }
                    .seal(),
                )?;
            } else {
                let _ = std::fs::remove_file(&path);
            }
            results.push(TargetResult {
                package_id: package_id.to_owned(),
                target: name.to_owned(),
                kind,
                establishing_run: call.run_id.clone(),
                reused: false,
                requested,
                complete: call.success && complete(&call.report),
                report: call.report.clone(),
            });
        } else if let Some(receipt) = read::<Receipt>(&path).filter(|r| {
            let mut observed: Vec<PathBuf> = artifact["filenames"]
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
            artifact["fresh"] == true
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
                && r.artifact_hashes.keys().cloned().collect::<BTreeSet<_>>()
                    == observed.into_iter().collect()
                && r.valid(context)
        }) {
            event(
                &serde_json::json!({"event":"target-reused","target":name,"establishing_run":receipt.invocation.run_id}),
            );
            results.push(TargetResult {
                package_id: package_id.to_owned(),
                target: name.to_owned(),
                kind,
                establishing_run: receipt.invocation.run_id,
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
            r.establishing_run == call.run_id
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
                package_id: package
                    .and_then(|p| p["id"].as_str())
                    .unwrap_or_default()
                    .to_owned(),
                target: call.identity.name,
                kind: vec![call.identity.crate_type],
                establishing_run: call.run_id,
                reused: false,
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
fn dependency_closure<'a>(
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
    let mut package_data = data.clone();
    let mut packages: Vec<Value> = data["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["id"].as_str().is_some_and(|id| closure.contains(id)))
        .cloned()
        .collect();
    // Immutable registry/git packages are still content-bound; local edits or
    // corruption of a fetched crate cannot silently retain an old proof.
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

/// Rechecks a persisted result's declared inputs without starting Cargo or Verus.
/// Caller environment is explicit so daemon status never substitutes its own configuration.
///
/// # Errors
/// Returns `SAIL3108` for missing observation state; callers should report unknown freshness.
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

/// Refreshes a running context's persisted input declaration without invoking Cargo.
///
/// # Errors
/// Returns `SAIL3108` until the worker has persisted its context and declarations.
pub fn refresh_context(
    cache: &Path,
    workspace: &Path,
    context_sha256: &str,
    environment: &BTreeMap<String, String>,
) -> Result<super::input::InputSnapshot> {
    refresh_context_validated(cache, workspace, context_sha256, environment, || {
        runtime_current(cache, context_sha256)
    })
}

fn record_observation_layout(data: &mut Value, engine: Option<&crate::engine::Request>) {
    if engine.is_some_and(|request| request.function.is_some()) {
        data["sail_managed_observation_layout"] = Value::from(1);
    }
}

fn observation_directory(context: &Path, data: &Value) -> Result<PathBuf> {
    match data.get("sail_managed_observation_layout") {
        None => Ok(context.to_owned()),
        Some(version) if version.as_u64() == Some(1) => Ok(context.join("repair-observations-v1")),
        Some(_) => Err(Error::new(
            "SAIL3108",
            "unsupported ordinary observation layout",
        )),
    }
}

pub(super) fn refresh_context_validated(
    cache: &Path,
    workspace: &Path,
    context_sha256: &str,
    environment: &BTreeMap<String, String>,
    runtime_valid: impl FnOnce() -> bool,
) -> Result<super::input::InputSnapshot> {
    let context = cache
        .join("projects")
        .join(digest(workspace.as_os_str().as_encoded_bytes()))
        .join(context_sha256);
    let data: Value = read(&context.join("input-state.json"))
        .ok_or_else(|| Error::new("SAIL3108", "declared input observation is unavailable"))?;
    let observations = observation_directory(&context, &data)?;
    let (_, mut extras, covered) = super::build_inputs::stale_environment(
        &observations.join("build-inputs.json"),
        &data,
        Some(environment),
    );
    extras.extend(script_roots(&data));
    let home = environment
        .get("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            environment
                .get("HOME")
                .map(|h| PathBuf::from(h).join(".cargo"))
        });
    let mut snapshot = super::input::snapshot_metadata_home(&data, &extras, home.as_deref());
    let invocation = Path::new(
        data["sail_invocation_directory"]
            .as_str()
            .unwrap_or_else(|| workspace.to_str().unwrap_or_default()),
    );
    snapshot.roots.extend(extras);
    snapshot.roots.extend(super::input::configuration_paths(
        invocation,
        home.as_deref(),
    ));
    snapshot.roots.extend(runtime_paths(cache, context_sha256));
    snapshot.roots.sort();
    snapshot.roots.dedup();
    let source_state = observations.join("source-inputs.json");
    let source_coverage = source_state.is_file() && super::source_inputs::load(&source_state).1;
    let runtime_complete = runtime_valid();
    snapshot.complete &= covered && source_coverage && runtime_complete;
    Ok(snapshot)
}

#[cfg(target_os = "linux")]
pub(super) fn observation_plan(
    cache: &Path,
    workspace: &Path,
    context_sha256: &str,
    environment: &BTreeMap<String, String>,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>)> {
    let context = cache
        .join("projects")
        .join(digest(workspace.as_os_str().as_encoded_bytes()))
        .join(context_sha256);
    let data: Value = read(&context.join("input-state.json"))
        .ok_or_else(|| Error::new("SAIL3108", "declared input observation is unavailable"))?;
    let home = environment
        .get("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            environment
                .get("HOME")
                .map(|p| PathBuf::from(p).join(".cargo"))
        });
    let invocation = Path::new(
        data["sail_invocation_directory"]
            .as_str()
            .unwrap_or_else(|| workspace.to_str().unwrap_or_default()),
    );
    let mut roots: Vec<PathBuf> = data["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| {
            p["manifest_path"]
                .as_str()
                .and_then(|p| Path::new(p).parent())
        })
        .map(Path::to_owned)
        .collect();
    roots.extend(
        data["sail_source_inputs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(PathBuf::from),
    );
    let observations = observation_directory(&context, &data)?;
    roots.extend(super::build_inputs::observation_paths(
        &observations.join("build-inputs.json"),
    ));
    roots.extend(script_roots(&data));
    roots.extend(super::input::configuration_paths(
        invocation,
        home.as_deref(),
    ));
    roots.extend(runtime_paths(cache, context_sha256));
    roots.push(context.join("input-state.json"));
    for name in ["source-inputs.json", "build-inputs.json"] {
        roots.push(observations.join(name));
    }
    let identities = cache.join("runtime-identities");
    let link = identities.join(format!("{context_sha256}.link"));
    if let Some(sha) = read::<String>(&link) {
        roots.push(identities.join(format!("{sha}.json")));
    }
    roots.push(link);
    roots.sort();
    roots.dedup();
    let excluded = ["target_directory", "sail_cache_directory"]
        .into_iter()
        .filter_map(|key| data[key].as_str())
        .map(|p| invocation.join(p))
        .chain([cache.to_owned()])
        .collect();
    Ok((roots, excluded))
}

#[cfg(target_os = "linux")]
pub(super) fn runtime_identity(cache: &Path, context: &str) -> Option<(String, Vec<PathBuf>)> {
    let state = cache.join("runtime-identities");
    let sha = read::<String>(&state.join(format!("{context}.link")))?;
    let identities = read::<Vec<(String, String)>>(&state.join(format!("{sha}.json")))?;
    if sha != digest(&serde_json::to_vec(&identities).ok()?) || identities.is_empty() {
        return None;
    }
    Some((
        sha,
        identities
            .into_iter()
            .map(|(path, _)| PathBuf::from(path))
            .collect(),
    ))
}

fn runtime_paths(cache: &Path, context: &str) -> Vec<PathBuf> {
    let state = cache.join("runtime-identities");
    read::<String>(&state.join(format!("{context}.link")))
        .and_then(|sha| read::<Vec<(String, String)>>(&state.join(format!("{sha}.json"))))
        .unwrap_or_default()
        .into_iter()
        .map(|(path, _)| PathBuf::from(path))
        .collect()
}

pub(super) fn runtime_current(cache: &Path, context: &str) -> bool {
    let mut hashed = super::files::FileDigests::default();
    runtime_matches(cache, context, |path| hashed.hash(path))
}

fn runtime_matches(
    cache: &Path,
    context: &str,
    mut hash: impl FnMut(&Path) -> Result<String>,
) -> bool {
    let state = cache.join("runtime-identities");
    let identity_sha = read::<String>(&state.join(format!("{context}.link")));
    let identities = identity_sha
        .as_ref()
        .and_then(|sha| read::<Vec<(String, String)>>(&state.join(format!("{sha}.json"))));
    identities.as_ref().is_some_and(|paths| {
        identity_sha
            .as_ref()
            .is_some_and(|sha| *sha == digest(&serde_json::to_vec(paths).unwrap_or_default()))
            && paths
                .iter()
                .all(|(path, expected)| hash(Path::new(path)).as_ref() == Ok(expected))
    })
}

fn proof_runtime_current(
    cache: &Path,
    context: &str,
    sysroots: &mut super::sysroot::Baseline,
) -> (bool, bool) {
    let mut hashed = super::files::FileDigests::default();
    proof_runtime_current_with(cache, context, sysroots, |path| hashed.hash(path))
}

fn proof_runtime_current_with(
    cache: &Path,
    context: &str,
    sysroots: &mut super::sysroot::Baseline,
    mut hash: impl FnMut(&Path) -> Result<String>,
) -> (bool, bool) {
    if !sysroots.clean() {
        return (runtime_current(cache, context), sysroots.complete());
    }
    let matches = runtime_matches(cache, context, |path| {
        sysroots
            .captured(path)
            .map_or_else(|| hash(path), |hash| Ok(hash.clone()))
    });
    if sysroots.complete() {
        (matches, true)
    } else {
        // A lost barrier never certifies reuse, even if restored bytes happen to
        // match. Rehash through the original oracle and retain uncertainty.
        (runtime_current(cache, context), false)
    }
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

#[allow(
    clippy::too_many_lines,
    reason = "The complete proof lifecycle and final input sealing remain together."
)]
pub(crate) fn run_engine(
    config: &Config,
    err: &mut dyn Write,
    engine: Option<crate::engine::Request>,
) -> Result<ProjectResult> {
    let mut owned = config.clone();
    owned.workspace =
        std::path::absolute(&owned.workspace).map_err(|e| Error::new("SAIL3100", e.to_string()))?;
    owned.cache = owned.workspace.join(&owned.cache);
    if let Some(engine) = &engine {
        owned.cargo_verus.clone_from(&engine.cargo_verus);
    }
    let config = &owned;
    if config.args.iter().any(|s| {
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
        .any(|flag| s == flag || s.starts_with(&format!("{flag}=")))
            || s == "--target-dir"
            || s.starts_with("--target-dir=")
            || s == "--no-verify"
            || s.starts_with("--log")
            || s == "--verus-source"
            || s == "--sysroot"
            || s.starts_with("--sysroot=")
    }) {
        return Err(Error::new(
            "SAIL3100",
            "incremental verification owns its target and logs and requires Cargo verification",
        ));
    }
    let started = Instant::now();
    let diagnostics = Diagnostics::new();
    let metadata_phase = diagnostics.phase(Phase::Metadata);
    let invocation = std::path::absolute(&config.workspace)
        .map_err(|e| Error::new("SAIL3100", e.to_string()))?;
    super::configuration::check(&invocation, &config.args)?;
    let mut data = metadata(&config.workspace, &config.args, &config.cargo)?;
    data["sail_invocation_directory"] = Value::String(invocation.to_string_lossy().into_owned());
    data["sail_cache_directory"] = Value::String(config.cache.to_string_lossy().into_owned());
    let workspace_root = PathBuf::from(data["workspace_root"].as_str().unwrap_or_default());
    if std::env::var_os("RUSTC_WORKSPACE_WRAPPER").is_some() {
        return Err(Error::new(
            "SAIL3100",
            "incremental verification does not support a chained workspace rustc wrapper",
        ));
    }
    let expected = expected_targets(&data, &config.args);
    if expected.is_empty() {
        return Err(Error::new(
            "SAIL3100",
            "no selected Cargo targets declare package.metadata.verus.verify=true",
        ));
    }
    drop(metadata_phase);
    let context_phase = diagnostics.phase(Phase::RuntimeContext);
    let Context {
        hash,
        compiler_hash,
        mut sysroots,
    } = context(config, engine.as_ref(), &diagnostics)?;
    drop(context_phase);

    let run_id = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    );
    let owned_paths = diagnostics.phase(Phase::OwnedPaths);
    let (paths, _ownership) = prepare(
        config,
        &workspace_root,
        &hash,
        &compiler_hash,
        &run_id,
        engine.as_ref(),
    )?;
    drop(owned_paths);
    record_observation_layout(&mut data, engine.as_ref());
    let initial_snapshot = diagnostics.phase(Phase::InitialSnapshot);
    let generation_available = super::build_inputs::family::begin(
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
    let mut script_bootstrap = if generation_available
        || !super::build_inputs::family::bootstrap::Bootstrap::supported_arguments(&config.args)
        || !super::build_inputs::family::bootstrap::Bootstrap::eligible_workload(&data)
    {
        diagnostics.bootstrap(
            super::diagnostics::BootstrapStage::OwnerEligibility,
            super::diagnostics::BootstrapOutcome::Skipped,
        );
        None
    } else {
        let mut runtime = runtime_paths(&config.cache, &hash);
        let state = config.cache.join("runtime-identities");
        let link = state.join(format!("{hash}.link"));
        if let Some(identity) = read::<String>(&link) {
            runtime.push(state.join(format!("{identity}.json")));
            runtime.push(link);
        }
        let bootstrap_candidate = super::build_inputs::family::bootstrap::Bootstrap::begin_traced(
            &data,
            &extras,
            &runtime,
            Some(&diagnostics),
        );
        bootstrap_candidate.filter(|candidate| {
            let matches = candidate.runtime_matches(&config.cache, &hash);
            diagnostics.bootstrap(
                super::diagnostics::BootstrapStage::InitialRuntime,
                if matches {
                    super::diagnostics::BootstrapOutcome::Ready
                } else {
                    super::diagnostics::BootstrapOutcome::Refused
                },
            );
            matches
        })
    };
    let mut before = snapshot_metadata(&data, &extras);
    before.complete &= source_coverage;
    before.roots.extend(super::input::configuration_paths(
        &invocation,
        std::env::var_os("CARGO_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")))
            .as_deref(),
    ));
    before.roots.extend(runtime_paths(&config.cache, &hash));
    if !stale_scripts.is_empty() {
        clean(config, &paths.target, &stale_scripts)?;
    }
    drop(initial_snapshot);
    let guards = diagnostics.phase(Phase::InputGuards);
    let phase = diagnostics.phase(Phase::GuardAbsent);
    let absent = super::absent::Observer::begin(&before, &paths.run.join("observer"));
    drop(phase);
    let phase = diagnostics.phase(Phase::GuardWatch);
    let monitor = super::observe::Observation::begin(&before, &extras, &config.cache);
    // Proof-time kernel coverage spans the selected targets' dependency
    // closure. Cargo compiles nothing outside it for this selection, which is
    // checked against Cargo's own unit records after every invocation; the
    // whole-workspace revision and stamps still bind every other package.
    // Build-script coverage likewise treats scripts outside an explicit
    // selection's closure as never scheduled; the same unit check binds it.
    let scope = selected_closure(&data, &expected);
    let script_scope = super::build_inputs::selection_closure(&data);
    let units_in_scope = |paths: &Paths| {
        artifacts_within(paths, scope.as_ref()) && artifacts_within(paths, script_scope.as_ref())
    };
    drop(phase);
    let phase = diagnostics.phase(Phase::GuardCoherent);
    let mut coherent =
        super::coherent::Guard::begin(&super::input::closure_scope(&before, &data, scope.as_ref()));
    drop(phase);
    drop(guards);
    let monitored_snapshot = diagnostics.phase(Phase::MonitoredSnapshot);
    // Establish the revision again after the watcher is live, closing the setup
    // interval between metadata collection and native observation.
    let monitored = snapshot_metadata(&data, &extras);
    before.revision = monitored.revision;
    before.observation_paths = monitored.observation_paths;
    drop(monitored_snapshot);
    let input_barrier = diagnostics.phase(Phase::InputBarrier);
    before.complete &= monitored.complete && coherent.ready_at(super::coherent::Barrier::Input);
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
    clean(config, &paths.target, &late_scripts)?;
    let stamp_before = super::input::mutation_stamp(&before);
    super::configuration::check(&invocation, &config.args)?;
    super::files::json_metadata(&paths.input_state, &data)?;
    event(&serde_json::json!({
        "kind":"context-ready", "workspace":before.workspace, "run_id":run_id,
        "targets":expected.iter().map(|(package,name,kind)| serde_json::json!({"package_id":package,"name":name,"kind":[kind]})).collect::<Vec<_>>(),
        "context_sha256":hash, "input_revision":before.revision,
        "complete":before.complete && script_coverage && monitor.complete() && absent.ready(),
        "source_observation":source_state.is_file()
    }));
    drop(input_barrier);
    let cleanup = diagnostics.phase(Phase::TargetCleanup);
    let forced = config.force
        || std::env::var("SAIL_MANAGED_FORCE").as_deref() == Ok("1")
        || focused(&config.args)
        || !before.complete
        || !monitor.complete()
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
    if forced {
        clean(config, &paths.target, &selected(&config.args, &names))?;
    }
    let dirty: Vec<String> = std::fs::read_dir(&paths.receipts)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| read::<Receipt>(&e.path()))
        .filter(|r| r.input_revision != target_revision(&data, &r.package_id))
        .map(|r| r.package_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !dirty.is_empty() {
        clean(config, &paths.target, &dirty)?;
    }
    drop(cleanup);
    let seal = diagnostics.phase(Phase::InvocationSeal);
    let source_engine = engine.is_some();
    let runtime = Runtime {
        run_id: run_id.clone(),
        args: config.args.clone(),
        z3: config.z3.clone(),
        z3_sha256: file_digest(&config.z3)?,
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
        engine,
    };
    json(&paths.runtime, &runtime)?;
    drop(seal);
    let invocation_phase = diagnostics.phase(Phase::Invocation);
    let mut exit = invoke(config, &paths, err)?;
    if !units_in_scope(&paths) {
        coherent.refuse_scope();
    }
    let (mut targets, missing) = target_records(&paths, &data, &hash, forced, &expected)?;
    if exit == Some(0) && !missing.is_empty() {
        let mut retry_packages = missing.into_iter().collect::<Vec<_>>();
        if script_bootstrap.is_some() {
            // The retry replaces the artifact journal. Preserve independent
            // compilation authority by actually rebuilding supported scripts,
            // rather than treating the retry's reused fresh=true binaries as
            // fresh compilation. The same optional input owner stays alive.
            retry_packages = super::build_inputs::family::bootstrap::Bootstrap::retry_packages(
                &data,
                retry_packages,
            );
        }
        clean(config, &paths.target, &retry_packages)?;
        std::fs::write(paths.run.join("artifacts.jsonl"), [])
            .map_err(|e| Error::new("SAIL3106", e.to_string()))?;
        std::fs::remove_dir_all(paths.run.join("pending-receipts"))
            .and_then(|()| std::fs::create_dir_all(paths.run.join("pending-receipts")))
            .map_err(|e| Error::new("SAIL3107", e.to_string()))?;
        exit = invoke(config, &paths, err)?;
        if !units_in_scope(&paths) {
            coherent.refuse_scope();
        }
        let (retried, missing) = target_records(&paths, &data, &hash, forced, &expected)?;
        targets = retried;
        if !missing.is_empty() {
            return Err(Error::new(
                "SAIL3107",
                "selected Cargo targets did not produce proof receipts after a fresh verification",
            ));
        }
    }
    drop(invocation_phase);
    let post_observations = diagnostics.phase(Phase::PostObservations);
    super::build_inputs::update(
        &paths.build_inputs,
        &paths.run.join("artifacts.jsonl"),
        &data,
    )?;
    let (sources, source_coverage) =
        super::source_inputs::update(&source_state, &paths.run.join("artifacts.jsonl"), &data)?;
    let (_, mut after_extras, after_coverage) =
        super::build_inputs::stale(&paths.build_inputs, &data);
    after_extras.extend(script_roots);
    drop(post_observations);
    let post_metadata = diagnostics.phase(Phase::PostMetadata);
    let mut after_data = metadata(&config.workspace, &config.args, &config.cargo)?;
    drop(post_metadata);
    let post_snapshot = diagnostics.phase(Phase::PostSnapshot);
    record_observation_layout(&mut after_data, runtime.engine.as_ref());
    after_data["sail_cache_directory"] = data["sail_cache_directory"].clone();
    after_data["sail_invocation_directory"] = data["sail_invocation_directory"].clone();
    after_data["sail_source_inputs"] = serde_json::to_value(sources).unwrap_or_default();
    after_data["sail_source_inputs_by_package"] =
        serde_json::to_value(super::source_inputs::packages(&source_state)).unwrap_or_default();
    super::files::json_metadata(&paths.input_state, &after_data)?;
    let mut after = snapshot_metadata(&after_data, &after_extras);
    after.roots.extend(super::input::configuration_paths(
        &invocation,
        std::env::var_os("CARGO_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")))
            .as_deref(),
    ));
    after.roots.extend(runtime_paths(&config.cache, &hash));
    // Resolve fresh receipt bindings while the input stamp still brackets every
    // content read. A newly discovered include may already belong to another
    // observed package, so global currentness alone does not imply that the
    // pre-invocation target revision contained that particular include.
    let pending = completed_receipts(&paths.run, &after_data)?;
    let stamp_after = super::input::mutation_stamp(&after);
    drop(post_snapshot);
    let final_barriers = diagnostics.phase(Phase::FinalBarriers);
    let absent = absent.finish();
    let coherent_complete = coherent.ready_at(super::coherent::Barrier::PostVerification);
    let mut sysroot_complete = sysroots.complete();
    let currentness = if !before.complete
        || !after.complete
        || !after_coverage
        || !source_coverage
        || !monitor.complete()
        || !absent.complete
        || {
            let (runtime_complete, covered) =
                proof_runtime_current(&config.cache, &hash, &mut sysroots);
            sysroot_complete = covered;
            !runtime_complete || !covered
        }
        || !coherent_complete
        || stamp_before.is_none()
        || stamp_after.is_none()
    {
        Currentness::Unknown
    } else if before.revision == after.revision
        && stamp_before == stamp_after
        && !monitor.changed()
        && !absent.changed
    {
        Currentness::Current
    } else {
        Currentness::Historical
    };
    drop(final_barriers);
    if exit == Some(0)
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
    let mut notes = Vec::new();
    if !coherent_complete {
        notes.push(format!("SAIL3114: coherent kernel leases or native input coverage were unavailable or broken during verification ({}); no current proof receipt is published", coherent.failure_description()));
    }
    if !sysroot_complete {
        notes.push(format!("SAIL3114: sysroot byte identities lost continuous kernel coverage ({}); no current proof receipt is published", sysroots.failure_description()));
    }
    if !script_coverage {
        notes.push("Build-script input declarations were established during this run".to_owned());
    }
    if focused(&config.args) {
        notes.push(
            "Requested function/module scope is partial; whole-target reuse is disabled".to_owned(),
        );
    }
    let fallbacks = engine_fallbacks(&targets);
    if fallbacks > 0 {
        notes.push(format!(
            "SAIL3123: the interactive engine faulted for {fallbacks} target(s); their exact stock Verus results are reported and publish no proof receipt"
        ));
    }
    let mut result = ProjectResult {
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
    };
    let result_publication = diagnostics.phase(Phase::ResultPublication);
    json(&paths.run.join("result.json"), &result)?;
    drop(result_publication);
    if result.currentness == Currentness::Current {
        let _receipt_publication = diagnostics.phase(Phase::ReceiptPublication);
        for (path, receipt) in pending {
            json(&path, &receipt)?;
        }
        if !super::receipts::publish_guarded(
            &paths.run,
            &paths.receipts,
            &result.context_sha256,
            || sysroots.complete(),
        )? {
            // Coverage may have ended during the marker write, after the
            // earlier guarded comparison. Recheck actual runtime bytes through
            // the original oracle; uncertainty remains regardless of equality.
            let _ = runtime_current(&config.cache, &result.context_sha256);
            result.currentness = Currentness::Unknown;
            result.notes.push(format!("SAIL3114: sysroot kernel coverage ended before receipt commit ({}); no current proof receipt is published", sysroots.failure_description()));
            json(&paths.run.join("result.json"), &result)?;
        }
    }
    let family_publication = diagnostics.phase(Phase::CompilerFamilyPublication);
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
    if result.exit_code == Some(0)
        && result.error.is_none()
        && !focused(&config.args)
        && result
            .targets
            .iter()
            .filter(|target| target.requested)
            .all(|target| target.complete)
    {
        // The report owns the unchanged full proof-context hash at this point.
        let mut final_runtime = None;
        if let Some(certificate) = script_bootstrap.as_mut().filter(|candidate| {
            let matches = candidate.runtime_matches(&config.cache, &result.context_sha256);
            final_runtime = Some(matches);
            matches
        }) {
            certificate.publish_traced(
                &paths.compiler_inputs,
                &compiler_hash,
                &paths.target,
                &paths.build_inputs,
                &after_data,
                &paths.run.join("artifacts.jsonl"),
                Some(&diagnostics),
            );
        }
        // Observe only after the original runtime/input checks and publication
        // attempt: the positive check-to-emission interval contains no sink I/O.
        if let Some(matches) = final_runtime {
            diagnostics.bootstrap(
                super::diagnostics::BootstrapStage::FinalRuntime,
                if matches {
                    super::diagnostics::BootstrapOutcome::Ready
                } else {
                    super::diagnostics::BootstrapOutcome::Refused
                },
            );
        }
    }
    drop(family_publication);
    Ok(result)
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
    use super::*;

    #[cfg(target_os = "linux")]
    include!("runner_artifact_isolation_tests.rs");
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
    fn compiler_artifact_identity_omits_only_the_engine_function()
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
        assert_ne!(
            compiler,
            context_hashes(&[], Some(&selected), &identity, &environment)?.1
        );
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
            shim: sail.clone(),
            z3: sail,
            socket: None,
            args: Vec::new(),
            force: true,
        })
    }

    #[test]
    fn shared_compiler_target_has_exclusive_ownership_and_private_proof_paths()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            std::env::temp_dir().join(format!("sail-compiler-family-{}", std::process::id()));
        let config = compiler_fixture(&root)?;
        let (selected, ownership) = prepare(&config, &root, "selected", "compiler", "first", None)?;
        std::fs::create_dir_all(&selected.target)?;
        let dependency = selected.target.join("ordinary-dependency.rlib");
        std::fs::write(&dependency, "ordinary dependency")?;
        let rejected = prepare(&config, &root, "missing", "compiler", "second", None);
        assert_eq!(
            rejected
                .err()
                .ok_or("concurrent family owner admitted")?
                .code,
            "SAIL3104"
        );
        drop(ownership);
        // Failed family acquisition must also release the new proof-context lock.
        let (missing, ownership) = prepare(&config, &root, "missing", "compiler", "second", None)?;
        assert_eq!(selected.target, missing.target);
        assert_eq!(selected.cargo_alias, missing.cargo_alias);
        assert_eq!(selected.proxy_alias, missing.proxy_alias);
        assert_eq!(std::fs::read(&dependency)?, b"ordinary dependency");
        assert_ne!(selected.receipts, missing.receipts);
        assert_ne!(selected.runtime, missing.runtime);
        assert_ne!(selected.run, missing.run);
        assert_ne!(selected.build_inputs, missing.build_inputs);
        let (isolated, isolated_owner) =
            prepare(&config, &root, "different", "different", "third", None)?;
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
                command.env("SAIL_TEST_COMPILER_FAMILY_BUSY", "1");
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
        let (_, ownership) = prepare(&config, &root, "context", "compiler", "run", None)?;
        probe(true)?;
        drop(ownership);
        probe(false)?;
        let mut broken = config.clone();
        broken.sail = root.join("missing-executable");
        assert!(prepare(&broken, &root, "context", "compiler", "error", None).is_err());
        let outcome = std::panic::catch_unwind(|| {
            let owner = prepare(&config, &root, "context", "compiler", "unwind", None);
            assert!(owner.is_ok());
            std::panic::resume_unwind(Box::new("controlled ownership unwind"));
        });
        assert!(outcome.is_err());
        let (_, ownership) = prepare(&config, &root, "context", "compiler", "after", None)?;
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
            version: 2,
            checksum: String::new(),
            committed: paths.run.join("committed.json"),
            package_id: "input".to_owned(),
            name: "input".to_owned(),
            kind: vec!["lib".to_owned()],
            context: "selected".to_owned(),
            input_revision: target_revision(data, "input"),
            invocation: Invocation {
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
            },
            artifact_hashes: artifact_hashes(&[artifact.to_owned()]).unwrap_or_default(),
        }
        .seal()
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
            let root = std::env::temp_dir().join(format!(
                "sail-negative-generation-{label}-{}",
                std::process::id()
            ));
            let config = compiler_fixture(&root)?;
            let (paths, ownership) = prepare(&config, &root, "negative", "compiler", "run", None)?;
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
            true,
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

    #[cfg(target_os = "linux")]
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
        let (targets, _) = target_records(&f.paths, &f.data, "negative", true, &f.expected)?;
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
            "negative declaration establishment cannot publish"
        );
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

    #[test]
    fn shared_fresh_artifact_cannot_replace_a_focus_context_proof()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!("sail-compiler-proof-{}", std::process::id()));
        let config = compiler_fixture(&root)?;
        let (paths, ownership) = prepare(&config, &root, "missing", "compiler", "run", None)?;
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
        let (targets, missing) = target_records(&paths, &data, "missing", false, &expected)?;
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
        let (targets, missing) = target_records(&paths, &data, "selected", false, &expected)?;
        assert_eq!(targets.len(), 1);
        assert!(targets[0].reused);
        assert!(missing.is_empty());
        for (context, force) in [("missing", false), ("selected", true)] {
            let (targets, missing) = target_records(&paths, &data, context, force, &expected)?;
            assert_eq!(targets, Vec::<TargetResult>::new());
            assert_eq!(missing, BTreeSet::from(["input".to_owned()]));
        }
        drop(ownership);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn guarded_sysroot_reads_skip_only_owned_digests_and_fall_back_on_changes()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = PathBuf::from("/tmp").join(format!("sail-runtime-guard-{}", std::process::id()));
        let sysroot = root.join("sysroot");
        std::fs::create_dir_all(sysroot.join("bin"))?;
        std::fs::create_dir_all(sysroot.join("lib/rustlib/host/lib"))?;
        let rustc = sysroot.join("bin/rustc");
        let library = sysroot.join("lib/rustlib/host/lib/std.rlib");
        let other = root.join("z3");
        for path in [&rustc, &library, &other] {
            std::fs::write(path, b"good")?;
        }
        let mut owner =
            super::super::sysroot::Baseline::begin(&BTreeSet::from([sysroot]), |path| {
                path == rustc
            });
        assert!(owner.complete());
        let mut identities = Vec::new();
        for path in [&rustc, &library, &other] {
            let hash = file_digest(path)?;
            owner.capture(path, &hash);
            identities.push((path.to_string_lossy().into_owned(), hash));
        }
        let state = root.join("runtime-identities");
        std::fs::create_dir_all(&state)?;
        let sha = digest(&serde_json::to_vec(&identities)?);
        let identity_path = state.join(format!("{sha}.json"));
        json(&identity_path, &identities)?;
        json(&state.join("ctx.link"), &sha)?;
        let mut read_paths = Vec::new();
        assert_eq!(
            proof_runtime_current_with(&root, "ctx", &mut owner, |path| {
                read_paths.push(path.to_owned());
                file_digest(path)
            }),
            (true, true)
        );
        assert_eq!(read_paths, vec![rustc, other]);
        identities[1].1 = "forged-digest".to_owned();
        json(&identity_path, &identities)?;
        assert_eq!(
            proof_runtime_current(&root, "ctx", &mut owner),
            (false, true)
        );
        identities[1].1 = file_digest(&library)?;
        json(&identity_path, &identities)?;
        // Restore the actual bytes after a write, but deliberately poison the
        // old capture. Lost coverage must read fresh bytes, not this old value.
        std::fs::write(&library, b"fail")?;
        std::fs::write(&library, b"good")?;
        owner.capture(&library, "poisoned-capture");
        assert_eq!(
            proof_runtime_current(&root, "ctx", &mut owner),
            (true, false)
        );
        drop(owner);
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
            shim: launcher.clone(),
            z3: runtime.join("z3"),
            socket: None,
            args: Vec::new(),
            force: true,
        };
        std::fs::write(runtime.join("z3"), b"tampered")?;
        let error = context(&config, Some(&request), &Diagnostics::default())
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
            version: 2,
            checksum: String::new(),
            package_id: "vstd".to_owned(),
            name: "vstd".to_owned(),
            kind: vec!["lib".to_owned()],
            context: "context".to_owned(),
            input_revision: target_revision(&data, "vstd"),
            invocation: Invocation {
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
            },
            artifact_hashes: BTreeMap::new(),
            committed: run.join("committed.json"),
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
        assert!(std::fs::write(&app_source, "fail").is_ok());
        assert_eq!(target_revision(&data, "vstd"), dependency_before);
        assert_ne!(target_revision(&data, "app"), app_before);
        assert!(std::fs::write(&dependency_source, "changed dependency").is_ok());
        assert_ne!(target_revision(&data, "vstd"), dependency_before);
        assert_ne!(target_revision(&data, "app"), app_before);
        assert!(std::fs::remove_dir_all(root).is_ok());
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
        let (paths, ownership) = prepare(&config, &root, "scope", "compiler", "run", None)?;
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
            shim: PathBuf::from("sail-z3"),
            z3: PathBuf::from("z3"),
            socket: None,
            args: args.iter().map(|&a| a.to_owned()).collect(),
            force: true,
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
            "SAIL_JOB_RPC_TIMINGS",
            "SAIL_JOB_GUARD_PARENT",
            "SAIL_MANAGED_FORCE",
            "SAIL_MANAGED_PROXY",
        ] {
            assert!(!input_environment_key(std::ffi::OsStr::new(key)));
        }
        for key in ["RUSTFLAGS", "RUSTC", "PATH", "SAIL_CACHE_DIR"] {
            assert!(input_environment_key(std::ffi::OsStr::new(key)));
        }
        let original = BTreeMap::from([("RUSTFLAGS", "-C opt-level=0")]);
        let mut observed = original.clone();
        observed.insert("SAIL_JOB_RPC_TIMINGS", "1");
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
