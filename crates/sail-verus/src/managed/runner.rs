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

fn context(config: &Config, data: &Value) -> Result<String> {
    let mut identity = Vec::new();
    for path in [
        &config.sail,
        &config.cargo_verus,
        &config.cargo,
        &config.z3,
        &config.shim,
    ] {
        identity.push((path.to_string_lossy().into_owned(), file_digest(path)?));
    }
    let wrapper = config.cargo_verus.with_file_name("verus");
    identity.push((
        wrapper.to_string_lossy().into_owned(),
        file_digest(&wrapper)?,
    ));
    let mut environment: BTreeMap<String, String> = std::env::vars_os()
        .filter(|(k, _)| {
            let key = k.to_string_lossy();
            ![
                "SAIL_JOB_EVENTS",
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
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.to_string_lossy().into_owned(),
            )
        })
        .collect();
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
                environment.insert(path.to_string_lossy().into_owned(), file_digest(&path)?);
            }
        }
    }
    Ok(digest(
        &serde_json::to_vec(&(1, &config.args, identity, environment))
            .map_err(|e| Error::new("SAIL3104", e.to_string()))?,
    ))
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
            let _ = pipe.read_to_end(&mut bytes);
            bytes
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
    let stdout = stdout.and_then(|h| h.join().ok()).unwrap_or_default();
    std::fs::write(paths.run.join("cargo.stdout"), stdout)
        .and_then(|()| std::fs::write(paths.run.join("cargo.stderr"), captured))
        .map_err(|e| Error::new("SAIL3106", e.to_string()))?;
    Ok(code)
}

fn focused(args: &[String]) -> bool {
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
        let path = receipt_path(paths, package_id, name, &kind);
        let call = calls.iter().find(|c| {
            c.identity.manifest == Path::new(package["manifest_path"].as_str().unwrap_or_default())
                && c.identity.name == name.replace('-', "_")
                && kind.iter().any(|k| {
                    k == &c.identity.crate_type || (k == "lib" && c.identity.crate_type == "rlib")
                })
        });
        if let Some(call) = call {
            let artifact_paths: Vec<PathBuf> = artifact["filenames"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(PathBuf::from)
                .collect();
            let hashes = artifact_hashes(&artifact_paths)?;
            let reusable = call.success && complete(&call.report) && !force;
            if reusable {
                json(
                    &path,
                    &Receipt {
                        version: 1,
                        package_id: package_id.to_owned(),
                        name: name.to_owned(),
                        kind: kind.clone(),
                        context: context.to_owned(),
                        invocation: call.clone(),
                        artifact_hashes: hashes,
                    },
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
                complete: call.success,
                report: call.report.clone(),
            });
        } else if let Some(receipt) = read::<Receipt>(&path)
            .filter(|r| artifact["fresh"] == true && !force && r.valid(context))
        {
            event(
                &serde_json::json!({"event":"target-reused","target":name,"establishing_run":receipt.invocation.run_id}),
            );
            results.push(TargetResult {
                package_id: package_id.to_owned(),
                target: name.to_owned(),
                kind,
                establishing_run: receipt.invocation.run_id,
                reused: true,
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
            r.establishing_run == call.run_id && r.target.replace('-', "_") == call.identity.name
        }) {
            let package = data["packages"].as_array().into_iter().flatten().find(|p| {
                p["manifest_path"]
                    .as_str()
                    .is_some_and(|s| Path::new(s) == call.identity.manifest)
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
                complete: call.success,
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
        s == "--target-dir"
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
    let data = metadata(&config.workspace, &config.args, &config.cargo)?;
    let before = snapshot_metadata(&data, &[]);
    let hash = context(config, &data)?;
    let run_id = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    );
    let (paths, _lock) = prepare(config, &before.workspace, &hash, &run_id)?;
    let forced = config.force
        || std::env::var("SAIL_MANAGED_FORCE").as_deref() == Ok("1")
        || focused(&config.args)
        || !before.complete;
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
    let (mut targets, missing) = target_records(&paths, &data, &hash, forced)?;
    if exit == Some(0) && !missing.is_empty() {
        clean(
            config,
            &paths.target,
            &missing.into_iter().collect::<Vec<_>>(),
        )?;
        std::fs::write(paths.run.join("artifacts.jsonl"), [])
            .map_err(|e| Error::new("SAIL3106", e.to_string()))?;
        exit = invoke(config, &paths, err)?;
        let (retried, missing) = target_records(&paths, &data, &hash, forced)?;
        targets = retried;
        if !missing.is_empty() {
            return Err(Error::new(
                "SAIL3107",
                "selected Cargo targets did not produce proof receipts after a fresh verification",
            ));
        }
    }
    let after = snapshot_metadata(
        &metadata(&config.workspace, &config.args, &config.cargo)?,
        &[],
    );
    let currentness = if !before.complete || !after.complete {
        Currentness::Unknown
    } else if before.revision == after.revision {
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
    let verifier_invocations = u64::try_from(invocations(&paths).len()).unwrap_or(u64::MAX);
    let reused_targets =
        u64::try_from(targets.iter().filter(|t| t.reused).count()).unwrap_or(u64::MAX);
    let mut notes = Vec::new();
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
