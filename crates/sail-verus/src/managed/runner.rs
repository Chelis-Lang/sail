//! Owned Cargo invocation, artifact validation, and complete report reuse.

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
}

fn input_environment_key(key: &std::ffi::OsStr) -> bool {
    ![
        // Observation-only output paths set by the job worker.
        "SAIL_PROOF_EVENTS",
        "SAIL_JOB_EVENTS",
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
fn context(config: &Config) -> Result<String> {
    let mut identity = Vec::new();
    let mut hashed = super::files::FileDigests::default();
    for path in [
        &config.sail,
        &config.cargo_verus,
        &config.cargo,
        &config.z3,
        &config.shim,
    ] {
        identity.push((path.to_string_lossy().into_owned(), hashed.hash(path)?));
    }
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
    let sysroots = BTreeSet::from([cargo_sysroot, verifier_sysroot]);
    identity.push((
        config
            .cargo_verus
            .with_file_name("rust_verify")
            .to_string_lossy()
            .into_owned(),
        hashed.hash(&wrapper.with_file_name("rust_verify"))?,
    ));
    for sysroot in sysroots {
        for path in [sysroot.join("bin/rustc"), sysroot.join("bin/cargo")] {
            if path.is_file() {
                identity.push((path.to_string_lossy().into_owned(), hashed.hash(&path)?));
            }
        }
        // Include every installed target's implicit standard/compiler libraries.
        // Cargo may choose its target through ancestor configuration or environment,
        // so host-only binding would miss cross-target standard library mutations.
        let mut directories = vec![sysroot.join("lib")];
        for entry in std::fs::read_dir(sysroot.join("lib/rustlib"))
            .into_iter()
            .flatten()
            .flatten()
        {
            let library = entry.path().join("lib");
            if library.is_dir() {
                directories.push(library);
            }
        }
        for directory in directories {
            for item in std::fs::read_dir(directory).into_iter().flatten().flatten() {
                let path = item.path();
                if path.is_file() {
                    identity.push((path.to_string_lossy().into_owned(), hashed.hash(&path)?));
                }
            }
        }
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
    let hash = digest(
        &serde_json::to_vec(&(
            1,
            &config.args,
            identity,
            environment.into_iter().collect::<Vec<_>>(),
        ))
        .map_err(|e| Error::new("SAIL3104", e.to_string()))?,
    );
    json(&state.join(format!("{hash}.link")), &identity_sha)?;
    Ok(hash)
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

fn prepare(config: &Config, workspace: &Path, hash: &str, run_id: &str) -> Result<(Paths, File)> {
    let root = config
        .cache
        .join("projects")
        .join(digest(workspace.as_os_str().as_encoded_bytes()));
    let context = root.join(hash);
    std::fs::create_dir_all(&context).map_err(|e| Error::new("SAIL3104", e.to_string()))?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(context.join("ownership.lock"))
        .map_err(|e| Error::new("SAIL3104", e.to_string()))?;
    lock.try_lock()
        .map_err(|e| Error::new("SAIL3104", format!("managed target is in use: {e}")))?;
    let paths = Paths {
        target: context.join("target"),
        cargo_alias: context.join("sail-cargo"),
        proxy_alias: context.join("sail-verus-driver"),
        receipts: context.join("receipts"),
        runtime: context.join("runtime.json"),
        build_inputs: context.join("build-inputs.json"),
        run: context.join("runs").join(run_id),
    };
    for dir in [
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
    Ok((paths, lock))
}

fn clean(config: &Config, target: &Path, packages: &[String]) -> Result<()> {
    if packages.is_empty() {
        return Ok(());
    }
    let mut cmd = Command::new(&config.cargo);
    cmd.current_dir(&config.workspace)
        .args(["clean", "--target-dir"])
        .arg(target);
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
        .args(&config.args[..split])
        .arg("--target-dir")
        .arg(&paths.target)
        .arg("--message-format=json-render-diagnostics")
        .args(&config.args[split..])
        .env("CARGO", &paths.cargo_alias)
        .env("SAIL_MANAGED_CARGO", &config.cargo)
        .env("SAIL_MANAGED_PROXY", &paths.proxy_alias)
        .env("SAIL_MANAGED_RUNTIME", &paths.runtime)
        .env("SAIL_MANAGED_ARTIFACTS", paths.run.join("artifacts.jsonl"))
        .env("VERUS_Z3_PATH", &config.shim)
        .env("SAIL_Z3_PATH", &config.z3)
        .env("SAIL_SOLVER_MODE", "default")
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

fn target_revision(data: &Value, package_id: &str) -> String {
    let mut closure = BTreeSet::from([package_id.to_owned()]);
    let mut pending = vec![package_id.to_owned()];
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
    let context = cache
        .join("projects")
        .join(digest(workspace.as_os_str().as_encoded_bytes()))
        .join(context_sha256);
    let data: Value = read(&context.join("input-state.json"))
        .ok_or_else(|| Error::new("SAIL3108", "declared input observation is unavailable"))?;
    let (_, mut extras, covered) = super::build_inputs::stale_environment(
        &context.join("build-inputs.json"),
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
    let source_state = context.join("source-inputs.json");
    let source_coverage = source_state.is_file() && super::source_inputs::load(&source_state).1;
    let runtime_complete = runtime_current(cache, context_sha256);
    snapshot.complete &= covered && source_coverage && runtime_complete;
    Ok(snapshot)
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

fn runtime_current(cache: &Path, context: &str) -> bool {
    let mut hashed = super::files::FileDigests::default();
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
                .all(|(path, hash)| hashed.hash(Path::new(path)).as_ref() == Ok(hash))
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
    let mut owned = config.clone();
    owned.workspace =
        std::path::absolute(&owned.workspace).map_err(|e| Error::new("SAIL3100", e.to_string()))?;
    owned.cache = owned.workspace.join(&owned.cache);
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
    let hash = context(config)?;

    let run_id = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    );
    let (paths, _lock) = prepare(config, &workspace_root, &hash, &run_id)?;
    let source_state = paths.build_inputs.with_file_name("source-inputs.json");
    let (sources, source_coverage) = super::source_inputs::load(&source_state);
    data["sail_source_inputs"] = serde_json::to_value(&sources).unwrap_or_default();
    data["sail_source_inputs_by_package"] =
        serde_json::to_value(super::source_inputs::packages(&source_state)).unwrap_or_default();
    let (stale_scripts, mut extras, script_coverage) =
        super::build_inputs::stale(&paths.build_inputs, &data);
    let script_roots = script_roots(&data);
    extras.extend(script_roots.clone());
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
    let absent = super::absent::Observer::begin(&before, &paths.run.join("observer"));
    let monitor = super::observe::Observation::begin(&before, &extras, &config.cache);
    // Establish the revision again after the watcher is live, closing the setup
    // interval between metadata collection and native observation.
    let monitored = snapshot_metadata(&data, &extras);
    before.revision = monitored.revision;
    before.observation_paths = monitored.observation_paths;
    before.complete &= monitored.complete;
    let stamp_before = super::input::mutation_stamp(&before);
    super::configuration::check(&invocation, &config.args)?;
    json(
        &paths
            .run
            .parent()
            .and_then(Path::parent)
            .unwrap_or(&paths.run)
            .join("input-state.json"),
        &data,
    )?;
    event(&serde_json::json!({
        "kind":"context-ready", "workspace":before.workspace,
        "context_sha256":hash, "input_revision":before.revision,
        "complete":before.complete && script_coverage && monitor.complete() && absent.ready(),
        "source_observation":source_state.is_file()
    }));
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
    };
    json(&paths.runtime, &runtime)?;
    let mut exit = invoke(config, &paths, err)?;
    let (mut targets, missing) = target_records(&paths, &data, &hash, forced, &expected)?;
    if exit == Some(0) && !missing.is_empty() {
        clean(
            config,
            &paths.target,
            &missing.into_iter().collect::<Vec<_>>(),
        )?;
        std::fs::write(paths.run.join("artifacts.jsonl"), [])
            .map_err(|e| Error::new("SAIL3106", e.to_string()))?;
        std::fs::remove_dir_all(paths.run.join("pending-receipts"))
            .and_then(|()| std::fs::create_dir_all(paths.run.join("pending-receipts")))
            .map_err(|e| Error::new("SAIL3107", e.to_string()))?;
        exit = invoke(config, &paths, err)?;
        let (retried, missing) = target_records(&paths, &data, &hash, forced, &expected)?;
        targets = retried;
        if !missing.is_empty() {
            return Err(Error::new(
                "SAIL3107",
                "selected Cargo targets did not produce proof receipts after a fresh verification",
            ));
        }
    }
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
    let mut after_data = metadata(&config.workspace, &config.args, &config.cargo)?;
    after_data["sail_cache_directory"] = data["sail_cache_directory"].clone();
    after_data["sail_invocation_directory"] = data["sail_invocation_directory"].clone();
    after_data["sail_source_inputs"] = serde_json::to_value(sources).unwrap_or_default();
    after_data["sail_source_inputs_by_package"] =
        serde_json::to_value(super::source_inputs::packages(&source_state)).unwrap_or_default();
    json(
        &paths.build_inputs.with_file_name("input-state.json"),
        &after_data,
    )?;
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
    let absent = absent.finish();
    let currentness = if !before.complete
        || !after.complete
        || !after_coverage
        || !source_coverage
        || !monitor.complete()
        || !absent.complete
        || !runtime_current(&config.cache, &hash)
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
    if !script_coverage {
        notes.push("Build-script input declarations were established during this run".to_owned());
    }
    if focused(&config.args) {
        notes.push(
            "Requested function/module scope is partial; whole-target reuse is disabled".to_owned(),
        );
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
    };
    json(&paths.run.join("result.json"), &result)?;
    if result.currentness == Currentness::Current {
        for (path, receipt) in pending {
            json(&path, &receipt)?;
        }
        super::receipts::publish(&paths.run, &paths.receipts, &result.context_sha256)?;
    }
    Ok(result)
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
    fn focused_proof_selection_is_never_whole_target_coverage() {
        assert!(focused(&[
            "--".to_owned(),
            "--verify-function=lemma".to_owned()
        ]));
        assert!(!focused(&["--features".to_owned(), "foo".to_owned()]));
    }
    #[test]
    fn lifecycle_only_job_environment_does_not_partition_proofs() {
        for key in [
            "SAIL_PROOF_EVENTS",
            "SAIL_JOB_EVENTS",
            "SAIL_JOB_GUARD_PARENT",
            "SAIL_MANAGED_FORCE",
            "SAIL_MANAGED_PROXY",
        ] {
            assert!(!input_environment_key(std::ffi::OsStr::new(key)));
        }
        for key in ["RUSTFLAGS", "RUSTC", "PATH", "SAIL_CACHE_DIR"] {
            assert!(input_environment_key(std::ffi::OsStr::new(key)));
        }
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
