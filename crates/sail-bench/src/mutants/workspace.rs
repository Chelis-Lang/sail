//! One persistent working copy of a corpus unit, verified the way a user
//! verifies it: stock `cargo verus verify` and `sail verify` in the same
//! source tree, each with its own Cargo target directory, so the edit loop
//! keeps Cargo's and Sail's incremental state between mutants.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use sail_api::protocol::StatsSnapshot;
use sail_verus::corpus::{Build, Unit};
use sail_verus::run::{RunRecord, copy_tree, prepare_unit};
use sail_verus::toolchain::Installed;
use serde_json::Value;

/// Verus flags both sides pass. They are the two flags `sail verify`
/// itself adds, so neither changes what Sail's managed driver accepts for
/// proof reuse, and neither changes a byte sent to Z3.
pub const FLAGS: &[&str] = &["--output-json", "--time"];

/// Environment that would make Cargo behave differently from a clean shell.
const CARGO_ENV_REMOVED: &[&str] = &[
    "CARGO",
    "CARGO_BUILD_RUSTC",
    "CARGO_BUILD_RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_RUSTFLAGS",
    "CARGO_BUILD_TARGET_DIR",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_MAKEFLAGS",
    "CARGO_MANIFEST_DIR",
    "CARGO_PKG_NAME",
    "CARGO_PRIMARY_PACKAGE",
    "CARGO_TARGET_DIR",
    "RUSTC",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTC_WRAPPER",
    "RUSTDOC",
    "RUSTFLAGS",
    "RUSTUP_TOOLCHAIN",
];

/// A prepared working copy.
#[derive(Debug, Clone)]
pub struct Workspace {
    /// Scratch root: `src/`, `target-stock/`, `target-sail/`.
    pub root: PathBuf,
    /// The project's source copy.
    pub src: PathBuf,
    /// Where Cargo runs (the package directory).
    pub package: PathBuf,
    /// `cargo verus verify` arguments before `--`.
    pub cargo_args: Vec<String>,
    /// Verus arguments after `--`.
    pub verus_args: Vec<String>,
    /// The unit's extra environment.
    pub env: BTreeMap<String, String>,
}

/// The library package that wraps a `verus` unit (a single crate root
/// run with `verus` directly) so that both `cargo verus verify` and
/// `sail verify` can verify it, as `sail-bench repair` does for
/// human-eval-verus.
fn wrapper_manifest(name: &str, root: &str, vstd: &Path) -> Result<String, String> {
    let quoted = serde_json::to_string(&vstd.display().to_string()).map_err(|e| e.to_string())?;
    let root = serde_json::to_string(root).map_err(|e| e.to_string())?;
    Ok(format!(
        "[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[lib]\npath = {root}\n\n[package.metadata.verus]\nverify = true\n\n[workspace]\n\n[dependencies]\nvstd = {{ path = {quoted} }}\n"
    ))
}

impl Workspace {
    /// Copies and prepares `unit` under `root` (emptied first).
    ///
    /// # Errors
    ///
    /// A message if the copy, the dependency patch or Cargo's dependency
    /// resolution fails, or for an unsupported build kind.
    pub fn prepare(
        installed: &Installed,
        unit: &Unit,
        verus_source: &Path,
        root: &Path,
        registry_lock: Option<&Path>,
    ) -> Result<Self, String> {
        let root = std::path::absolute(root).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).map_err(|e| format!("{}: {e}", root.display()))?;
        match unit.build {
            Build::CargoVerus => {
                let prepared = if let Some(lock) = registry_lock {
                    let mut prepared = unit.clone();
                    prepared.source_dir = root.join("src");
                    copy_tree(&unit.source_dir, &prepared.source_dir).map_err(|e| e.to_string())?;
                    pin_registry(
                        &prepared.source_dir.join(&unit.root),
                        &prepared.source_dir,
                        lock,
                        &root,
                    )?;
                    prepared
                } else {
                    prepare_unit(installed, unit, Some(verus_source), &root)
                        .map_err(|e| e.to_string())?
                };
                if registry_lock.is_some() {
                    metadata_at(installed, &prepared.source_dir.join(&prepared.root), &root)?;
                }
                Ok(Self {
                    src: prepared.source_dir.clone(),
                    package: prepared.source_dir.join(&prepared.root),
                    root,
                    cargo_args: unit.cargo_args.iter().skip(1).cloned().collect(),
                    verus_args: unit.args.clone(),
                    env: unit.env.clone(),
                })
            }
            Build::Verus => {
                let src = root.join("src");
                copy_tree(&unit.source_dir, &src)
                    .map_err(|e| format!("cannot copy {}: {e}", unit.source_dir.display()))?;
                if src.join("Cargo.toml").exists() {
                    return Err(format!(
                        "{}: the source already has a Cargo.toml; wrap it by hand",
                        unit.id
                    ));
                }
                let name: String = unit
                    .project
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                    .collect();
                let manifest = wrapper_manifest(
                    &format!("{name}_mutants"),
                    &unit.root,
                    &verus_source.join("source/vstd"),
                )?;
                std::fs::write(src.join("Cargo.toml"), manifest).map_err(|e| e.to_string())?;
                let ws = Self {
                    package: src.clone(),
                    src,
                    root,
                    cargo_args: Vec::new(),
                    // Cargo builds a library already.
                    verus_args: unit
                        .args
                        .iter()
                        .filter(|a| *a != "--crate-type=lib")
                        .cloned()
                        .collect(),
                    env: unit.env.clone(),
                };
                if let Some(lock) = registry_lock {
                    pin_registry(&ws.package, &ws.src, lock, &ws.root)?;
                }
                metadata_at(installed, &ws.package, &ws.root)?;
                Ok(ws)
            }
            Build::LibcVerus | Build::Script => Err(format!(
                "{}: build {:?} is not supported by the mutation runner",
                unit.id, unit.build
            )),
        }
    }

    /// The stock `cargo verus verify` command.
    #[must_use]
    pub fn stock(&self, installed: &Installed) -> Command {
        let mut cmd = Command::new(&installed.cargo_verus);
        clean_cargo_env(&mut cmd);
        scrub_sail_env(&mut cmd);
        cmd.arg("verus")
            .arg("verify")
            .args(&self.cargo_args)
            .arg("--")
            .args(&self.verus_args)
            .args(FLAGS)
            .current_dir(&self.package)
            .env("RUSTUP_TOOLCHAIN", &installed.pins.verus.rust_toolchain)
            .env("CARGO_TARGET_DIR", self.root.join("target-stock"))
            .envs(&self.env);
        if !installed.z3_bundled {
            cmd.env("VERUS_Z3_PATH", &installed.z3);
        }
        cmd
    }

    /// Removes the package's own build products from the stock target
    /// directory (`cargo clean -p`), so the next stock run rebuilds and
    /// re-verifies the crate. Rewriting or touching an unchanged file is
    /// not enough: Cargo keeps a crate that last built successfully fresh
    /// when its content is unchanged.
    ///
    /// # Errors
    ///
    /// A message if the package name cannot be read or `cargo clean` fails.
    pub fn clean_stock(&self, installed: &Installed) -> Result<(), String> {
        let manifest = self.package.join("Cargo.toml");
        let name = std::fs::read_to_string(&manifest)
            .ok()
            .and_then(|t| t.parse::<toml::Table>().ok())
            .and_then(|m| {
                m.get("package")?
                    .get("name")?
                    .as_str()
                    .map(ToOwned::to_owned)
            })
            .ok_or_else(|| format!("no package name in {}", manifest.display()))?;
        let mut cmd = Command::new("cargo");
        clean_cargo_env(&mut cmd);
        let out = cmd
            .args(["clean", "-p", &name])
            .current_dir(&self.package)
            .env("RUSTUP_TOOLCHAIN", &installed.pins.verus.rust_toolchain)
            .env("CARGO_TARGET_DIR", self.root.join("target-stock"))
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("cannot run cargo clean: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "cargo clean -p {name} failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ))
        }
    }

    /// The `sail verify` command, with Sail's cache (and so its daemon) in
    /// `cache`.
    #[must_use]
    pub fn sail(&self, sail: &Path, cache: &Path, verus_build: Option<&Path>) -> Command {
        let mut cmd = Command::new(sail);
        clean_cargo_env(&mut cmd);
        scrub_sail_env(&mut cmd);
        cmd.arg("verify")
            .args(&self.cargo_args)
            .arg("--")
            .args(&self.verus_args)
            .args(FLAGS)
            .current_dir(&self.package)
            .env("SAIL_CACHE_DIR", cache)
            .env("CARGO_TARGET_DIR", self.root.join("target-sail"))
            .envs(&self.env);
        if let Some(build) = verus_build {
            cmd.env("SAIL_VERUS_BUILD", build);
        }
        cmd
    }
}

/// Quiet, deterministic Cargo output with no inherited build overrides.
fn clean_cargo_env(cmd: &mut Command) {
    for var in CARGO_ENV_REMOVED {
        cmd.env_remove(var);
    }
    cmd.env("CARGO_TERM_QUIET", "true")
        .env("CARGO_TERM_COLOR", "never")
        .env("CARGO_TERM_PROGRESS_WHEN", "never");
}

/// Removes every inherited `SAIL_*` and `VERUS_*` variable: each would
/// change what is measured.
fn scrub_sail_env(cmd: &mut Command) {
    for (k, _) in std::env::vars_os() {
        let k = k.to_string_lossy();
        if k.starts_with("SAIL_") || k.starts_with("VERUS_") {
            cmd.env_remove(k.as_ref());
        }
    }
}

/// Sail's Verus for the Sail side: `SAIL_VERUS_BUILD`, else
/// `SAIL_ENGINE_BUILD` (the CI image's prebuilt runtime), else none (Sail
/// then finds its bundled or cached runtime).
#[must_use]
pub fn verus_build_from_env() -> Option<PathBuf> {
    std::env::var_os("SAIL_VERUS_BUILD")
        .or_else(|| std::env::var_os("SAIL_ENGINE_BUILD"))
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Splits stdout into its JSON documents and whatever follows the last one.
fn parse_outputs(stdout: &[u8]) -> (Vec<Value>, String) {
    let mut stream = serde_json::Deserializer::from_slice(stdout).into_iter::<Value>();
    let mut docs = Vec::new();
    let mut end = 0;
    while let Some(Ok(doc)) = stream.next() {
        docs.push(doc);
        end = stream.byte_offset();
    }
    let rest = stdout.get(end..).unwrap_or_default();
    (docs, String::from_utf8_lossy(rest).trim().to_owned())
}

/// Removes Verus's long-running progress notes (`note: ... has been running
/// for N seconds` and `note: ... finished in N seconds`, each a complete
/// message up to its blank line). They depend on wall time, and so on the
/// machine's load, never on a verdict; every other byte is kept.
#[must_use]
pub fn drop_progress_notes(stderr: &str) -> String {
    let mut out = String::with_capacity(stderr.len());
    let mut skipping = false;
    for line in stderr.split_inclusive('\n') {
        if skipping {
            if line.trim().is_empty() {
                skipping = false;
            }
            continue;
        }
        if line.starts_with("note: ") && is_progress(line.trim_end()) {
            skipping = true;
            continue;
        }
        out.push_str(line);
    }
    out
}

fn is_progress(line: &str) -> bool {
    let seconds = |rest: &str| {
        rest.strip_suffix(" seconds")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    };
    line.split_once(" has been running for ")
        .is_some_and(|(_, rest)| seconds(rest))
        || line
            .rsplit_once(" finished in ")
            .is_some_and(|(_, rest)| seconds(rest))
}

/// Runs `cmd` to completion, retaining its streams in `log` and stopping
/// process-group descendants even when the group leader exits first.
///
/// # Errors
///
/// A message if it cannot start, capture output, or finish within `limit`:
/// an unfinished invocation is never a verification outcome.
pub fn execute(cmd: &mut Command, limit: Duration, log: &Path) -> Result<RunRecord, String> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        cmd.process_group(0);
    }
    std::fs::create_dir_all(log).map_err(|e| format!("{}: {e}", log.display()))?;
    let stdout_path = log.join("stdout.txt");
    let stderr_path = log.join("stderr.txt");
    let stdout = std::fs::File::create(&stdout_path).map_err(|e| e.to_string())?;
    let stderr = std::fs::File::create(&stderr_path).map_err(|e| e.to_string())?;
    let started = Instant::now();
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .map_err(|e| format!("cannot run {}: {e}", Path::new(cmd.get_program()).display()))?;
    let deadline = started + limit;
    let outcome = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            Ok(None) => break Err(format!("did not finish within {} s", limit.as_secs())),
            Err(e) => break Err(format!("lost the child process: {e}")),
        }
    };
    // Files rather than pipes prevent a descendant retaining stdout from
    // blocking collection after the command or its deadline has finished.
    #[cfg(unix)]
    let _ = Command::new("kill")
        .args(["-s", "KILL", "--", &format!("-{}", child.id())])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if outcome.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let wall_ms = started.elapsed().as_secs_f64() * 1000.0;
    let status = outcome?;
    let stdout = std::fs::read(stdout_path).map_err(|e| e.to_string())?;
    let stderr = std::fs::read(stderr_path).map_err(|e| e.to_string())?;
    let (outputs, stdout_rest) = parse_outputs(&stdout);
    Ok(RunRecord {
        exit_code: status.code(),
        stderr: drop_progress_notes(&String::from_utf8_lossy(&stderr)),
        outputs,
        stdout_rest,
        wall_ms,
        user_s: None,
        sys_s: None,
        max_rss_bytes: None,
    })
}

/// Published retained-proof receipts, identified by receipt path and run.
/// Reading only project/context/receipts avoids walking compiler build trees.
///
/// # Errors
///
/// A message if existing provenance cannot be read or parsed.
pub fn retained_receipts(cache: &Path) -> Result<BTreeSet<(PathBuf, String)>, String> {
    let projects = cache.join("projects");
    let mut found = BTreeSet::new();
    if !projects.exists() {
        return Ok(found);
    }
    let entries = |path: &Path| -> Result<Vec<PathBuf>, String> {
        std::fs::read_dir(path)
            .map_err(|e| e.to_string())?
            .map(|entry| entry.map(|e| e.path()).map_err(|e| e.to_string()))
            .collect()
    };
    for workspace in entries(&projects)? {
        if !workspace.is_dir() {
            continue;
        }
        for context in entries(&workspace)? {
            let receipts = context.join("receipts");
            if !receipts.is_dir() {
                continue;
            }
            for path in entries(&receipts)? {
                if path.extension().is_none_or(|extension| extension != "json") {
                    continue;
                }
                let receipt: Value =
                    serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?;
                if receipt
                    .pointer("/invocation/retained_from")
                    .and_then(Value::as_str)
                    .is_none_or(str::is_empty)
                {
                    continue;
                }
                let run = receipt
                    .pointer("/invocation/run_id")
                    .and_then(Value::as_str)
                    .ok_or("retained receipt has no run id")?;
                let committed = receipt
                    .get("committed")
                    .and_then(Value::as_str)
                    .ok_or("retained receipt has no commit marker")?;
                let marker: Value =
                    serde_json::from_slice(&std::fs::read(committed).map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?;
                let checksum = receipt
                    .get("checksum")
                    .and_then(Value::as_str)
                    .ok_or("retained receipt has no checksum")?;
                if receipt.pointer("/invocation/success") == Some(&Value::Bool(true))
                    && marker.get("run_id").and_then(Value::as_str) == Some(run)
                    && marker
                        .get("receipt_checksums")
                        .and_then(Value::as_array)
                        .is_some_and(|values| {
                            values.iter().any(|value| value.as_str() == Some(checksum))
                        })
                {
                    found.insert((path, run.to_owned()));
                }
            }
        }
    }
    Ok(found)
}

fn metadata_at(installed: &Installed, package: &Path, root: &Path) -> Result<(), String> {
    let mut command = Command::new("cargo");
    clean_cargo_env(&mut command);
    let output = command
        .args(["metadata", "--format-version", "1"])
        .current_dir(package)
        .env("RUSTUP_TOOLCHAIN", &installed.pins.verus.rust_toolchain)
        .env("CARGO_TARGET_DIR", root.join("metadata-target"))
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(root.join("metadata-target"));
    if !output.status.success() {
        return Err(format!(
            "workspace metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

/// Pin scratch-copy Verus dependencies to a release's registry versions.
fn pin_registry(package: &Path, source: &Path, lock: &Path, root: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(lock).map_err(|e| format!("{}: {e}", lock.display()))?;
    let versions = registry_versions(&text)?;
    let changes =
        sail_verus::patch::pin_verus_deps_to_registry(package, source, &versions, &BTreeSet::new())
            .map_err(|e| e.to_string())?;
    std::fs::write(root.join("vstd-adjustments.txt"), changes.join("\n"))
        .map_err(|e| e.to_string())?;
    // Preserve the exact input lock alongside the source adjustments. This is
    // release setup, not a request to replace unrelated project dependencies.
    std::fs::write(root.join("vstd-release.lock"), text).map_err(|e| e.to_string())?;
    Ok(())
}

fn registry_versions(text: &str) -> Result<BTreeMap<String, String>, String> {
    let document: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
    let packages = document
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or("release lock has no packages")?;
    let mut versions = BTreeMap::new();
    for package in packages {
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .ok_or("package has no name")?;
        if name != "vstd" && !name.starts_with("verus_") {
            continue;
        }
        if !package
            .get("source")
            .and_then(toml::Value::as_str)
            .is_some_and(|value| value.starts_with("registry+"))
        {
            return Err(format!("release package {name} is not from the registry"));
        }
        let version = package
            .get("version")
            .and_then(toml::Value::as_str)
            .ok_or("package has no version")?;
        if versions
            .insert(name.to_owned(), version.to_owned())
            .is_some()
        {
            return Err(format!("release lock contains multiple versions of {name}"));
        }
    }
    if !versions.contains_key("vstd") {
        return Err("release lock has no registry vstd".to_owned());
    }
    Ok(versions)
}

/// The cache daemon serving `cache`, as `sail status --json` reports it.
#[derive(Debug, Clone, Default)]
pub struct DaemonState {
    /// Its process id.
    pub pid: Option<u64>,
    /// Its counters.
    pub stats: Option<StatsSnapshot>,
}

/// Asks `sail status --json` about the daemon for `cache`.
#[must_use]
pub fn daemon_state(sail: &Path, cache: &Path) -> DaemonState {
    let mut cmd = Command::new(sail);
    scrub_sail_env(&mut cmd);
    let Ok(out) = cmd
        .args(["status", "--json"])
        .env("SAIL_CACHE_DIR", cache)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    else {
        return DaemonState::default();
    };
    let Ok(v) = serde_json::from_slice::<Value>(&out.stdout) else {
        return DaemonState::default();
    };
    if v.get("running").and_then(Value::as_bool) != Some(true) {
        return DaemonState::default();
    }
    DaemonState {
        pid: v.pointer("/info/pid").and_then(Value::as_u64),
        stats: v
            .get("stats")
            .and_then(|s| serde_json::from_value(s.clone()).ok()),
    }
}

/// Stops the daemon for `cache` (and anything else started on it).
pub fn stop_daemon(sail: &Path, cache: &Path) {
    if let Some(pid) = daemon_state(sail, cache).pid {
        let _ = Command::new("kill").arg(pid.to_string()).status();
    }
    let _ = Command::new("pkill")
        .arg("-f")
        .arg("--")
        .arg(cache.display().to_string())
        .status();
}

/// Writes `text` to `file`, moving its modification time forward. (An
/// unchanged text after a successful build still needs
/// [`Workspace::clean_stock`] for a rebuild.)
///
/// # Errors
///
/// Any I/O error.
pub fn write_fresh(file: &Path, text: &str) -> std::io::Result<()> {
    let before = std::fs::metadata(file).and_then(|m| m.modified()).ok();
    std::fs::write(file, text)?;
    if let Some(before) = before {
        let now = std::time::SystemTime::now();
        let at = if now > before {
            now
        } else {
            before + Duration::from_millis(10)
        };
        std::fs::File::options()
            .write(true)
            .open(file)?
            .set_modified(at)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn deadline_keeps_partial_logs_and_is_not_an_outcome() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory =
            std::env::temp_dir().join(format!("sail-mutant-timeout-{}", std::process::id()));
        let mut command = Command::new("sh");
        command.args(["-c", "printf started; printf diagnostic >&2; sleep 30"]);
        let result = execute(&mut command, Duration::from_millis(100), &directory);
        assert!(result.is_err());
        assert_eq!(std::fs::read(directory.join("stdout.txt"))?, b"started");
        assert_eq!(std::fs::read(directory.join("stderr.txt"))?, b"diagnostic");
        std::fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn leader_exit_stops_descendants_holding_output() -> Result<(), Box<dyn std::error::Error>> {
        let directory =
            std::env::temp_dir().join(format!("sail-mutant-descendant-{}", std::process::id()));
        std::fs::create_dir_all(&directory)?;
        let mut command = Command::new("sh");
        command
            .current_dir(&directory)
            .args(["-c", "sleep 30 & echo $! > child.pid; printf done"]);
        let record = execute(&mut command, Duration::from_secs(3), &directory)?;
        assert_eq!(record.exit_code, Some(0));
        let pid = std::fs::read_to_string(directory.join("child.pid"))?;
        let status = Command::new("ps")
            .args(["-o", "stat=", "-p", pid.trim()])
            .output()?;
        let state = String::from_utf8_lossy(&status.stdout);
        assert!(
            state.trim().is_empty() || state.trim().starts_with('Z'),
            "descendant still live: {state}"
        );
        std::fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[test]
    fn retained_share_requires_a_published_retained_receipt()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory =
            std::env::temp_dir().join(format!("sail-mutant-receipts-{}", std::process::id()));
        let receipts = directory.join("projects/workspace/context/receipts");
        std::fs::create_dir_all(&receipts)?;
        let marker = directory.join("committed.json");
        let path = receipts.join("target.json");
        let receipt = serde_json::json!({"checksum":"abc", "committed":marker,
            "invocation":{"retained_from":"original", "run_id":"current", "success":true}});
        std::fs::write(&path, serde_json::to_vec(&receipt)?)?;
        std::fs::write(
            &marker,
            br#"{"run_id":"current","receipt_checksums":["abc"]}"#,
        )?;
        assert_eq!(retained_receipts(&directory)?.len(), 1);
        std::fs::write(
            &marker,
            br#"{"run_id":"other","receipt_checksums":["abc"]}"#,
        )?;
        assert!(retained_receipts(&directory)?.is_empty());
        std::fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[test]
    fn drops_only_progress_notes() {
        let s = "error: x\n --> a.rs:1:1\n\nnote: function body check has been running for 2 seconds\nreporting errors as they are discovered (they may not be in source order)\n --> a.rs:3:1\n  |\n\nnote: function body check finished in 3 seconds\n --> a.rs:3:1\n\nnote: kept finished in time\n --> a.rs:5:1\n\nend\n";
        assert_eq!(
            drop_progress_notes(s),
            "error: x\n --> a.rs:1:1\n\nnote: kept finished in time\n --> a.rs:5:1\n\nend\n"
        );
    }

    #[test]
    fn parses_json_then_rest() {
        let (docs, rest) = parse_outputs(b"{\"a\":1}\n{\"b\":2}\ntail");
        assert_eq!(docs.len(), 2);
        assert_eq!(rest, "tail");
    }

    #[test]
    fn registry_setup_requires_one_registry_version_per_verus_crate() {
        let lock = r#"[[package]]
name = "vstd"
version = "0.0.1"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;
        assert_eq!(
            registry_versions(lock)
                .ok()
                .and_then(|mut versions| versions.remove("vstd")),
            Some("0.0.1".to_owned())
        );
        assert!(registry_versions(&format!("{lock}\n{lock}")).is_err());
        assert!(registry_versions(&lock.replace("registry+", "git+")).is_err());
        assert!(
            registry_versions(
                r#"[[package]]
name = "other"
version = "1"
"#
            )
            .is_err()
        );
    }

    #[test]
    fn the_wrapper_is_a_library() {
        let m = wrapper_manifest("x", "tasks/lib.rs", Path::new("/v/source/vstd"));
        let parsed: Option<toml::Table> = m.ok().and_then(|t| t.parse().ok());
        let path = parsed
            .as_ref()
            .and_then(|t| t.get("lib")?.get("path")?.as_str());
        assert_eq!(path, Some("tasks/lib.rs"));
    }
}
