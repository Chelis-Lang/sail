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
fn context(config: &Config, data: &Value) -> Result<String> {
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
    let wrapper = config.cargo_verus.with_file_name("verus");
    identity.push((
        wrapper.to_string_lossy().into_owned(),
        hashed.hash(&wrapper)?,
    ));
    let mut environment: BTreeMap<Vec<u8>, Vec<u8>> = std::env::vars_os()
        .filter(|(k, _)| {
            let key = k.to_string_lossy();
            ![
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
            .contains(&key.as_ref())
                && !key.starts_with("SAIL_MANAGED_")
        })
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
    let sysroot = PathBuf::from(String::from_utf8_lossy(&actual.stdout).trim());
    for path in [
        sysroot.join("bin/rustc"),
        sysroot.join("bin/cargo"),
        config.cargo_verus.with_file_name("rust_verify"),
    ] {
        if path.is_file() {
            identity.push((path.to_string_lossy().into_owned(), hashed.hash(&path)?));
        }
    }
    for directory in [
        sysroot.join("lib"),
        sysroot
            .join("lib/rustlib")
            .join(rust_host(rustc.as_os_str(), &config.workspace)?)
            .join("lib"),
    ] {
        for item in std::fs::read_dir(directory).into_iter().flatten().flatten() {
            let name = item.file_name().to_string_lossy().into_owned();
            if name.starts_with("librustc_driver")
                || name.starts_with("libLLVM")
                || name.starts_with("librustc_codegen_llvm")
            {
                identity.push((
                    item.path().to_string_lossy().into_owned(),
                    hashed.hash(&item.path())?,
                ));
            }
        }
    }
    // The exact Cargo configuration bytes join the context, not only its path.
    let workspace = Path::new(data["workspace_root"].as_str().unwrap_or_default());
    for parent in workspace.ancestors() {
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

fn rust_host(rustc: &std::ffi::OsStr, workspace: &Path) -> Result<String> {
    let output = Command::new(rustc)
        .arg("-vV")
        .current_dir(workspace)
        .output()
        .map_err(|e| Error::new("SAIL3104", e.to_string()))?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host: ").map(str::to_owned))
        .ok_or_else(|| Error::new("SAIL3104", "cannot identify compiler host target"))
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
            let reusable =
                call.success && complete(&call.report) && !force && !input_revision.is_empty();
            if reusable {
                json(
                    &path,
                    &Receipt {
                        version: 1,
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
    snapshot.roots.extend(extras);
    snapshot.roots.extend(super::input::configuration_paths(
        workspace,
        home.as_deref(),
    ));
    snapshot.roots.extend(runtime_paths(cache, context_sha256));
    snapshot.roots.sort();
    snapshot.roots.dedup();
    let source_coverage = super::source_inputs::load(&context.join("source-inputs.json")).1;
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
    }) {
        return Err(Error::new(
            "SAIL3100",
            "incremental verification owns its target and logs and requires Cargo verification",
        ));
    }
    let started = Instant::now();
    let mut data = metadata(&config.workspace, &config.args, &config.cargo)?;
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
    let hash = context(config, &data)?;

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
    let (stale_scripts, mut extras, script_coverage) =
        super::build_inputs::stale(&paths.build_inputs, &data);
    let script_roots = script_roots(&data);
    extras.extend(script_roots.clone());
    let mut before = snapshot_metadata(&data, &extras);
    before.complete &= source_coverage;
    before.roots.extend(super::input::configuration_paths(
        &before.workspace,
        std::env::var_os("CARGO_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")))
            .as_deref(),
    ));
    before.roots.extend(runtime_paths(&config.cache, &hash));
    if !stale_scripts.is_empty() {
        clean(config, &paths.target, &stale_scripts)?;
    }
    let monitor = super::observe::Observation::begin(&before, &extras, &config.cache);
    // Establish the revision again after the watcher is live, closing the setup
    // interval between metadata collection and native observation.
    let monitored = snapshot_metadata(&data, &extras);
    before.revision = monitored.revision;
    before.complete &= monitored.complete;
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
        "complete":before.complete && script_coverage && monitor.complete()
    }));
    let forced = config.force
        || std::env::var("SAIL_MANAGED_FORCE").as_deref() == Ok("1")
        || focused(&config.args)
        || !before.complete
        || !monitor.complete();
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
        original_wrapper: config.cargo_verus.with_file_name("verus"),
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
    after_data["sail_source_inputs"] = serde_json::to_value(sources).unwrap_or_default();
    json(
        &paths.build_inputs.with_file_name("input-state.json"),
        &after_data,
    )?;
    let after = snapshot_metadata(&after_data, &after_extras);
    // Flush native coalesced notifications before currentness publication. This
    // small bound is independent of solver runtime and never delays forwarding.
    std::thread::sleep(std::time::Duration::from_millis(100));
    let currentness = if !before.complete
        || !after.complete
        || !after_coverage
        || !source_coverage
        || !monitor.complete()
        || !runtime_current(&config.cache, &hash)
    {
        Currentness::Unknown
    } else if before.revision == after.revision && !monitor.changed() {
        Currentness::Current
    } else {
        Currentness::Historical
    };
    if currentness != Currentness::Current {
        for target in &targets {
            let _ = std::fs::remove_file(receipt_path(
                &paths,
                &target.package_id,
                &target.target,
                &target.kind,
            ));
        }
    }
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
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expected_target_coverage_respects_packages_and_target_selection() {
        let data = serde_json::json!({"workspace_members":["a","b"],"workspace_default_members":["a","b"],"resolve":{"root":null,"nodes":[]},"packages":[
            {"id":"a","name":"a","metadata":{"verus":{"verify":true}},"targets":[{"name":"a","kind":["lib"]},{"name":"app","kind":["bin"]}]},
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
            expected_targets(&data, &["--bin".to_owned(), "app".to_owned()]),
            BTreeSet::from([("a".to_owned(), "app".to_owned(), "bin".to_owned())])
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
}
