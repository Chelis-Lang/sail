//! Direct-file verification with compiler-bound inputs and bounded proof history.
//!
//! No Cargo project is synthesized. Only complete successful verify-only calls
//! with complete compiler observations can skip the verifier.

use super::files::{digest, file_digest};
use super::{Config, InputSnapshot, ghost_environment, receipts, runner};
use crate::{Error, Result, attribution};
use sail_api::project::{Currentness, Execution, ProjectResult, SCHEMA_VERSION, TargetResult};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read as _, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const VERSION: u32 = 2;
const MAX_RECORD: u64 = 64 * 1024 * 1024;

/// An explicit first positional Rust file selects direct verification.
#[must_use]
pub fn source(args: &[String]) -> Option<&str> {
    args.first()
        .filter(|s| !s.starts_with('-') && Path::new(s).extension().is_some_and(|e| e == "rs"))
        .map(String::as_str)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Inputs {
    files: BTreeMap<PathBuf, String>,
    namespace: BTreeMap<PathBuf, Vec<String>>,
    links: BTreeMap<PathBuf, PathBuf>,
    origins: BTreeMap<PathBuf, PathBuf>,
}

#[derive(Serialize, Deserialize)]
struct Record {
    version: u32,
    context: String,
    root: PathBuf,
    inputs: Inputs,
    environment: BTreeMap<String, String>,
    observed_environment: Option<ghost_environment::Environment>,
    result: ProjectResult,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn root_identity(config: &Config) -> (&Path, &[String]) {
    (&config.workspace, &config.args)
}

fn fail(message: impl Into<String>) -> Error {
    Error::new("SAIL3150", message)
}
fn encode(value: &impl Serialize) -> Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(|e| fail(e.to_string()))
}
fn record(path: &Path) -> Option<Record> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(MAX_RECORD + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= MAX_RECORD).then_some(())?;
    let (checksum, body) = bytes.split_at_checked(65)?;
    if checksum.get(64) != Some(&b'\n') || digest(body).as_bytes() != &checksum[..64] {
        return None;
    }
    serde_json::from_slice::<Record>(body)
        .ok()
        .filter(|r| r.version == VERSION)
}
fn environment() -> BTreeMap<String, String> {
    std::env::vars_os()
        .filter_map(|(key, value)| Some((key.into_string().ok()?, value.into_string().ok()?)))
        .filter(|(key, _)| {
            !matches!(
                key.as_str(),
                "SAIL_JOB_EVENTS"
                    | "SAIL_PROOF_EVENTS"
                    | "SAIL_JOB_GUARD_PARENT"
                    | "SAIL_MANAGED_FORCE"
            )
        })
        .collect()
}
fn context_path(cache: &Path, context: &str) -> PathBuf {
    cache
        .join("standalone/contexts")
        .join(format!("{context}.json"))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
enum PathKind {
    Missing,
    File,
    Directory,
    Other,
}

fn path_kind(path: &Path) -> Result<PathKind> {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(PathKind::File),
        Ok(metadata) if metadata.is_dir() => Ok(PathKind::Directory),
        Ok(_) => Ok(PathKind::Other),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(PathKind::Missing)
        }
        Err(error) => Err(fail(error.to_string())),
    }
}

fn namespace(path: &Path, cache: &Path, out: &mut Inputs, recurse: bool) -> Result<()> {
    if path.components().count() > 128 || out.namespace.len() >= 100_000 {
        return Err(fail(
            "standalone source namespace exceeds the observation bound",
        ));
    }
    let mut names = Vec::new();
    let mut entries = std::fs::read_dir(path)
        .map_err(|e| fail(e.to_string()))?
        .take(100_001)
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|e| fail(e.to_string()))?;
    if entries.len() > 100_000 {
        return Err(fail("standalone directory exceeds the observation bound"));
    }
    entries.sort_by_key(std::fs::DirEntry::file_name);
    let entry_names = entries
        .iter()
        .map(std::fs::DirEntry::file_name)
        .collect::<BTreeSet<_>>();
    for entry in entries {
        let child = entry.path();
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| fail("standalone input paths must be UTF-8"))?;
        // The cache and Git administration are not source discovery roots.
        // An actual compiler read beneath either refuses receipt admission.
        if child == cache || name == ".git" {
            continue;
        }
        let resolved_kind = path_kind(&child)?;
        // Rust also probes `leaf/mod.rs` when considering `leaf.rs`. This
        // observes that candidate even inside an unread or symlinked folder.
        // Following metadata also binds broken source links becoming files.
        let sibling = format!("{name}.rs");
        let module = if resolved_kind == PathKind::Directory
            && entry_names.contains(std::ffi::OsStr::new(&sibling))
        {
            path_kind(&child.join("mod.rs"))?
        } else {
            PathKind::Missing
        };
        names.push(
            serde_json::to_string(&(name, resolved_kind, module))
                .map_err(|error| fail(error.to_string()))?,
        );
        let kind = entry.file_type().map_err(|e| fail(e.to_string()))?;
        if kind.is_symlink() {
            out.links.insert(
                child.clone(),
                std::fs::read_link(&child).map_err(|e| fail(e.to_string()))?,
            );
        } else if kind.is_dir() && recurse {
            namespace(&child, cache, out, true)?;
        }
    }
    out.namespace.insert(path.to_owned(), names);
    Ok(())
}
fn observe(root: &Path, cache: &Path, files: impl Iterator<Item = PathBuf>) -> Result<Inputs> {
    let mut result = Inputs {
        files: BTreeMap::new(),
        namespace: BTreeMap::new(),
        links: BTreeMap::new(),
        origins: BTreeMap::new(),
    };
    namespace(root, cache, &mut result, true)?;
    for file in files {
        result.origins.insert(
            file.clone(),
            file.canonicalize().map_err(|e| fail(e.to_string()))?,
        );
        if file.starts_with(cache) || file.components().any(|c| c.as_os_str() == ".git") {
            return Err(fail("compiler input overlaps excluded administration"));
        }
        result.files.insert(file.clone(), file_digest(&file)?);
        for parent in file.ancestors() {
            if std::fs::symlink_metadata(parent).is_ok_and(|m| m.is_symlink()) {
                result.links.insert(
                    parent.to_owned(),
                    std::fs::read_link(parent).map_err(|e| fail(e.to_string()))?,
                );
            }
        }
        // Inline modules can introduce intermediate discovery directories
        // without reading another source file there. Observe every ancestor,
        // stopping at the already recursive workspace boundary. Otherwise an
        // external `leaf/mod.rs` could hide a newly ambiguous `leaf.rs`.
        for parent in file.parent().into_iter().flat_map(Path::ancestors) {
            if !result.namespace.contains_key(parent) {
                namespace(parent, cache, &mut result, false)?;
            }
            if parent == root {
                break;
            }
        }
    }
    Ok(result)
}
fn unchanged(before: &Inputs, after: &Inputs) -> bool {
    before.namespace == after.namespace
        && before.links == after.links
        && before.origins.iter().all(|(path, origin)| {
            after
                .origins
                .get(path)
                .is_none_or(|actual| actual == origin)
        })
        && before
            .files
            .iter()
            .all(|(path, hash)| after.files.get(path).is_none_or(|actual| actual == hash))
}

fn input_snapshot(
    root: &Path,
    cache: &Path,
    inputs: &Inputs,
    complete: bool,
) -> Result<InputSnapshot> {
    Ok(InputSnapshot {
        workspace: root.to_owned(),
        revision: digest(&encode(inputs)?),
        roots: inputs.namespace.keys().cloned().collect(),
        excluded: vec![cache.to_owned(), root.join(".git")],
        complete,
        observation_paths: inputs
            .files
            .keys()
            .chain(inputs.namespace.keys())
            .cloned()
            .collect(),
        digests: inputs.files.clone(),
        bindings: inputs.links.clone(),
        capture: None,
    })
}

/// A source-root observation for watch; starts neither Cargo nor Verus.
///
/// # Errors
/// Returns `SAIL3150` when the namespace cannot be completely read.
pub fn snapshot(root: &Path, cache: &Path, args: &[String]) -> Result<InputSnapshot> {
    let file = source(args).ok_or_else(|| fail("expected a Rust source file"))?;
    let inputs = observe(root, cache, std::iter::once(root.join(file)))?;
    input_snapshot(root, cache, &inputs, true)
}

/// Read-time currentness for a recorded standalone context.
///
/// # Errors
/// Returns `SAIL3150` for a missing, malformed or unbound context.
pub fn refresh(
    cache: &Path,
    root: &Path,
    context: &str,
    expected_environment: &BTreeMap<String, String>,
) -> Result<InputSnapshot> {
    let record = record(&context_path(cache, context))
        .ok_or_else(|| fail("standalone context has no complete observation"))?;
    if record.context != context || record.root != root || !runner::runtime_current(cache, context)
    {
        return Err(fail("standalone runtime context changed"));
    }
    let same_environment = record
        .environment
        .iter()
        .all(|(key, value)| expected_environment.get(key) == Some(value))
        && expected_environment
            .iter()
            .filter(|(key, _)| {
                !matches!(
                    key.as_str(),
                    "SAIL_JOB_EVENTS"
                        | "SAIL_PROOF_EVENTS"
                        | "SAIL_JOB_GUARD_PARENT"
                        | "SAIL_MANAGED_FORCE"
                )
            })
            .count()
            == record.environment.len();
    let same_environment = same_environment
        && record
            .observed_environment
            .as_ref()
            .is_some_and(|observed| {
                observed
                    .iter()
                    .all(|(key, value)| expected_environment.get(key) == value.as_ref())
            });
    let inputs = observe(root, cache, record.inputs.files.keys().cloned())?;
    input_snapshot(root, cache, &inputs, same_environment)
}

fn reusable(args: &[String]) -> bool {
    // Unknown flags remain supported by fresh Verus; they cannot suppress
    // required side effects or change the source/dependency search boundary.
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--rlimit" | "--num-threads" | "--crate-type" | "--crate-name" | "--edition" => {
                if it.next().is_none() {
                    return false;
                }
            }
            "--output-json"
            | "--time"
            | "--time-expanded"
            | "--no-report-long-running"
            | "--no-vstd" => {}
            _ if [
                "--rlimit=",
                "--num-threads=",
                "--crate-type=",
                "--crate-name=",
                "--edition=",
            ]
            .iter()
            .any(|prefix| arg.starts_with(prefix)) => {}
            _ => return false,
        }
    }
    true
}

/// Verify an original source file, or reuse an exactly bound successful proof.
///
/// # Errors
/// Returns a stable setup/persistence error; observation failures run fresh.
#[allow(
    clippy::too_many_lines,
    reason = "One invocation keeps capture, proof and closing observations in order."
)]
pub fn run(
    config: &Config,
    out: &mut dyn Write,
    err: &mut dyn Write,
    show: bool,
) -> Result<ProjectResult> {
    run_with_engine(config, out, err, show, None)
}

#[allow(
    clippy::too_many_lines,
    reason = "The direct compiler lifecycle keeps input and terminal sealing together."
)]
fn run_with_engine(
    config: &Config,
    out: &mut dyn Write,
    err: &mut dyn Write,
    show: bool,
    engine: Option<&crate::engine::Request>,
) -> Result<ProjectResult> {
    let started = Instant::now();
    let file = source(&config.args).ok_or_else(|| fail("expected a Rust source file"))?;
    let source = config.workspace.join(file);
    let mut args = config.args[1..].to_vec();
    if let Some(separator) = args.iter().position(|arg| arg == "--") {
        args.remove(separator);
    }
    if args
        .windows(2)
        .any(|pair| pair[0] == "-V" && pair[1].split('=').next() == Some("cvc5"))
        || args.iter().any(|arg| {
            arg.strip_prefix("-V")
                .is_some_and(|option| option.split('=').next() == Some("cvc5"))
        })
    {
        return Err(fail(
            "standalone verification supports the pinned Z3 solver; use ordinary Verus for cvc5",
        ));
    }
    let mut bound_config = config.clone();
    bound_config.args[0] = source.display().to_string();
    let runtime = runner::context(&bound_config, engine)?;
    let context = digest(&encode(&(
        VERSION,
        runtime.hash.clone(),
        root_identity(config),
        config.attribution_logs,
    ))?);
    let identity_dir = config.cache.join("runtime-identities");
    if let Ok(identity) = std::fs::read(identity_dir.join(format!("{}.link", runtime.hash))) {
        super::files::bytes_if_changed(&identity_dir.join(format!("{context}.link")), &identity)?;
    }
    let root = &config.workspace;
    let base = config
        .cache
        .join("standalone")
        .join(digest(root.as_os_str().as_encoded_bytes()))
        .join(&context);
    std::fs::create_dir_all(&base).map_err(|e| fail(e.to_string()))?;
    let lock = std::fs::File::options()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(base.join("lock"))
        .map_err(|e| fail(e.to_string()))?;
    lock.lock().map_err(|e| fail(e.to_string()))?;
    let run_id = format!(
        "file-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| fail(e.to_string()))?
            .as_nanos()
    );
    let run = base.join("runs").join(&run_id);
    std::fs::create_dir_all(&run).map_err(|e| fail(e.to_string()))?;
    let environment = environment();
    let known = record(&context_path(&config.cache, &context));
    let mut reads: BTreeSet<_> = known.as_ref().map_or_else(
        || BTreeSet::from([source.clone()]),
        |r| r.inputs.files.keys().cloned().collect(),
    );
    reads.insert(source.clone());
    let before = observe(root, &config.cache, reads.iter().cloned()).ok();
    let eligible = std::env::vars_os()
        .all(|(key, value)| key.into_string().is_ok() && value.into_string().is_ok())
        && reusable(&args)
        && !super::configuration::LOADER_VARIABLES
            .iter()
            .any(|name| std::env::var_os(name).is_some());
    if eligible
        && !config.force
        && let Some(before) = &before
    {
        let revision = digest(&encode(before)?);
        if let Some(old) = record(&base.join("proofs").join(format!("{revision}.json")))
            && old.context == context
            && old.root == *root
            && old.environment == environment
            && old.observed_environment.as_ref().is_some_and(|observed| {
                observed
                    .iter()
                    .all(|(key, value)| std::env::var(key).ok().as_ref() == value.as_ref())
            })
            && old.inputs == *before
            && old.result.context_sha256 == context
            && old.result.input_revision == revision
            && old.result.targets.len() == 1
            && old.result.currentness == Currentness::Current
            && old.result.exit_code == Some(0)
            && old.result.targets.iter().all(|t| t.complete && t.requested)
            && runner::runtime_current(&config.cache, &context)
            && observe(root, &config.cache, reads.iter().cloned())
                .is_ok_and(|after| after == *before)
        {
            if show {
                out.write_all(&old.stdout)
                    .map_err(|e| fail(e.to_string()))?;
            }
            err.write_all(&old.stderr)
                .map_err(|e| fail(e.to_string()))?;
            let mut result = old.result.clone();
            result.run_id = run_id;
            result.execution = Execution {
                wall_ms: started.elapsed().as_secs_f64() * 1000.0,
                reused_targets: 1,
                ..Execution::default()
            };
            for target in &mut result.targets {
                target.reused = true;
            }
            write_record(&context_path(&config.cache, &context), &old)?;
            return Ok(result);
        }
    }
    let mut observation = ghost_environment::Observation::begin(&run, &run_id, &context);
    let flags = crate::verus_stdout::UserFlags::of_verus(args.iter());
    let executable = engine.map_or_else(
        || config.cargo_verus.with_file_name("verus"),
        |request| request.cargo_verus.with_file_name("verus"),
    );
    let mut command = Command::new(executable);
    command
        .current_dir(root)
        .arg(file)
        .args(&args)
        .args(flags.sail_flags())
        .env("VERUS_Z3_PATH", &config.shim)
        .env("SAIL_Z3_PATH", &config.z3)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Match the managed runner: ambient experiment controls never select
    // an interactive engine or an opt-in cache namespace for a default call.
    command.env("SAIL_SOLVER_MODE", "default");
    for key in [
        "SAIL_ENGINE",
        "SAIL_ENGINE_FUNCTION",
        "SAIL_ENGINE_EVENTS",
        "SAIL_ENGINE_FOCUS_REPORT",
        "SAIL_ENGINE_CONTEXT",
    ] {
        command.env_remove(key);
    }
    if let Some(engine) = engine {
        command
            .env("SAIL_ENGINE", "interactive")
            .env("SAIL_ENGINE_EVENTS", run.join("engine.events.jsonl"))
            .env("SAIL_ENGINE_CONTEXT", &context)
            .env("SAIL_ENGINE_FOCUS_REPORT", run.join("focus.source.json"));
        if let Some(function) = &engine.function {
            command.env("SAIL_ENGINE_FUNCTION", function);
        }
    }
    if !args.iter().any(|a| a.contains("checksum-hash-algorithm")) {
        command.args(ghost_environment::OPTIONS);
    }
    if let Some(socket) = &config.socket {
        command.env("SAIL_DAEMON_SOCKET", socket);
    } else {
        command.env_remove("SAIL_DAEMON_SOCKET");
    }
    if let Some(observation) = &observation {
        observation.configure(&mut command);
    }
    let trace = run.join("trace");
    std::fs::create_dir_all(&trace).map_err(|e| fail(e.to_string()))?;
    command.env("SAIL_TRACE_DIR", &trace);
    let provenance_dir = session_records(&mut command, &run, config.socket.is_some())?;
    let logs = run.join("logs");
    let log_request = config.attribution_logs && !args.iter().any(|arg| arg.starts_with("--log"));
    if log_request {
        command
            .args(["--log", "air", "--log", "smt-transcript", "--log-dir"])
            .arg(&logs);
    }
    let child = command
        .spawn()
        .map_err(|e| fail(format!("cannot run Verus: {e}")))?;
    if let Some(observation) = &mut observation {
        observation.started(child.id());
    }
    let output = child.wait_with_output().map_err(|e| fail(e.to_string()))?;
    if engine.is_some()
        && !crate::engine_events::completed(
            &run.join("engine.events.jsonl"),
            &output.stdout,
            output.status.code(),
        )
    {
        let _ = std::fs::write(run.join("engine.failed.stdout"), &output.stdout);
        let _ = std::fs::write(run.join("engine.failed.stderr"), &output.stderr);
        return Err(Error::new(
            "SAIL3123",
            "standalone engine did not establish a normal complete compiler seal",
        ));
    }
    let stdout = crate::verus_stdout::rebuild(&output.stdout, flags);
    if show {
        out.write_all(&stdout).map_err(|e| fail(e.to_string()))?;
    }
    err.write_all(&output.stderr)
        .map_err(|e| fail(e.to_string()))?;
    let (docs, _) = attribution::json_documents(&output.stdout);
    let log_files = log_request
        .then(|| {
            attribution::read_logs(&if engine.is_some() {
                logs.join("full")
            } else {
                logs.clone()
            })
            .ok()
        })
        .flatten();
    let provenance = if config.socket.is_some() {
        attribution::Provenance::Records(attribution::read_session_records(&provenance_dir))
    } else {
        attribution::Provenance::NoCache
    };
    let attributed = attribution::attribute(
        &docs,
        log_files.as_deref(),
        &provenance,
        Some("standalone obligation logs unavailable"),
    );
    let diagnostics = crate::diagnostics::parse(&String::from_utf8_lossy(&output.stderr));
    let mut report = crate::report::build(&crate::report::Inputs {
        args: &config.args,
        exit_code: output.status.code(),
        solver_path: Some(config.z3.display().to_string()),
        solver_sha256: Some(file_digest(&config.z3)?),
        daemon: config.socket.is_some(),
        attribution: &attributed,
        diagnostics: &diagnostics,
    });
    if let Some(engine) = engine {
        report.engine = Some(engine.provenance(sail_api::output::EngineEstablishment::Engine));
    }
    let observed = observation.and_then(|o| o.observed());
    let mut actual_reads: BTreeSet<_> = observed
        .as_ref()
        .map(|o| o.files.keys().map(|p| root.join(p)).collect())
        .unwrap_or(reads);
    actual_reads.insert(source.clone());
    let after = observe(root, &config.cache, actual_reads.iter().cloned()).ok();
    let inputs_match = observed
        .as_ref()
        .zip(after.as_ref())
        .is_some_and(|(observed, after)| {
            let root_read = source.canonicalize().is_ok_and(|root| {
                observed.files.keys().any(|path| {
                    config.workspace.join(path).canonicalize().ok().as_ref() == Some(&root)
                })
            });
            root_read
                && observed.files.iter().all(|(path, original)| {
                    original.as_ref().is_some_and(|original| {
                        after
                            .files
                            .get(&root.join(path))
                            .is_some_and(|hash| original.checksum == format!("sha256={hash}"))
                            && std::fs::metadata(root.join(path))
                                .is_ok_and(|m| m.len() == original.size)
                    })
                })
                && observed
                    .environment
                    .iter()
                    .all(|(key, value)| std::env::var(key).ok().as_ref() == value.as_ref())
        });
    let namespace_match = before
        .as_ref()
        .zip(after.as_ref())
        .is_some_and(|(a, b)| unchanged(a, b));
    let closing_match = after.as_ref().is_some_and(|a| {
        observe(root, &config.cache, a.files.keys().cloned()).is_ok_and(|b| *a == b)
    });
    let current = eligible
        && inputs_match
        && namespace_match
        && closing_match
        && runner::runtime_current(&config.cache, &context);
    // A report with a check the cache never recorded is the run's genuine
    // result but never a reusable proof (#683).
    let sourced = receipts::sourced(&report);
    let complete = output.status.success()
        && receipts::complete(&report)
        && !super::partial(&config.args)
        && !args.iter().any(|a| a == "--no-verify");
    let revision = after
        .as_ref()
        .map(|i| encode(i).map(|b| digest(&b)))
        .transpose()?
        .unwrap_or_default();
    let mut result = ProjectResult {
        kind: "sail-project-result".into(),
        schema_version: SCHEMA_VERSION,
        run_id: run_id.clone(),
        workspace: root.display().to_string(),
        context_sha256: context.clone(),
        input_revision: revision.clone(),
        currentness: if current {
            Currentness::Current
        } else {
            Currentness::Unknown
        },
        exit_code: output.status.code(),
        targets: vec![TargetResult {
            package_id: format!(
                "standalone:{}",
                digest(source.as_os_str().as_encoded_bytes())
            ),
            target: file.to_owned(),
            kind: vec!["standalone".into()],
            establishing_run: run_id,
            reused: false,
            scope_reuse: None,
            requested: true,
            complete,
            report,
            dependency_store: None,
            dependency_build: None,
        }],
        execution: Execution {
            wall_ms: started.elapsed().as_secs_f64() * 1000.0,
            verifier_invocations: 1,
            ..Execution::default()
        },
        notes: if current {
            Vec::new()
        } else {
            vec!["SAIL3151: standalone input coverage is incomplete; fresh verification produced no reusable proof".into()]
        },
        error: None,
        engine: engine.map(crate::engine::Request::identity),
        no_reuse: None,
    };
    if let Some(inputs) = after {
        let record = Record {
            version: VERSION,
            context: context.clone(),
            root: root.clone(),
            inputs,
            environment,
            observed_environment: observed.as_ref().map(|o| o.environment.clone()),
            result: result.clone(),
            stdout,
            stderr: output.stderr,
        };
        let saved = write_record(&context_path(&config.cache, &context), &record).and_then(|()| {
            if current && complete && sourced && eligible && !config.force {
                save_proof(&base, &revision, &record)
            } else {
                Ok(())
            }
        });
        if let Err(error) = saved {
            result.notes.push(format!("SAIL3151: fresh verification completed but its proof record could not be retained: {error}"));
        }
    }
    Ok(result)
}

/// Where the shim writes this run's session records, which join cached
/// answers to the obligation logs (ADR 0014). The shim writes them only under
/// `SAIL_PROVENANCE_DIR`; without a cache there are none (#684).
fn session_records(command: &mut Command, run: &Path, cached: bool) -> Result<PathBuf> {
    let dir = run.join("provenance");
    if cached {
        std::fs::create_dir_all(&dir).map_err(|e| fail(e.to_string()))?;
        command.env("SAIL_PROVENANCE_DIR", &dir);
    } else {
        command.env_remove("SAIL_PROVENANCE_DIR");
    }
    Ok(dir)
}

fn write_record(path: &Path, record: &Record) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| fail(e.to_string()))?;
    }
    let body = encode(record)?;
    if body.len() as u64 + 65 > MAX_RECORD {
        return Ok(());
    }
    let mut bytes = format!("{}\n", digest(&body)).into_bytes();
    bytes.extend(body);
    super::files::bytes_if_changed(path, &bytes)
}

fn save_proof(base: &Path, revision: &str, record: &Record) -> Result<()> {
    let proofs = base.join("proofs");
    let path = proofs.join(format!("{revision}.json"));
    let bytes = encode(record)?;
    if bytes.len() as u64 > MAX_RECORD {
        return Ok(());
    }
    write_record(&path, record)?;
    let baseline = base.join("baseline");
    if !baseline.exists() {
        super::files::bytes_if_changed(&baseline, revision.as_bytes())?;
    }
    let pinned = std::fs::read_to_string(baseline).unwrap_or_default();
    let mut entries = std::fs::read_dir(&proofs)
        .map_err(|e| fail(e.to_string()))?
        .filter_map(std::result::Result::ok)
        .collect::<Vec<_>>();
    entries.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
    let mut total = entries
        .iter()
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum::<u64>();
    let mut count = entries.len();
    for entry in entries {
        if count <= 16 && total <= 1024 * 1024 * 1024 {
            break;
        }
        if entry.file_name().to_string_lossy() == format!("{pinned}.json") || entry.path() == path {
            continue;
        }
        let size = entry.metadata().map_err(|e| fail(e.to_string()))?.len();
        std::fs::remove_file(entry.path()).map_err(|e| fail(e.to_string()))?;
        total = total.saturating_sub(size);
        count -= 1;
    }
    Ok(())
}

/// Focus one exact standalone function and independently verify the complete file.
///
/// # Errors
/// Returns engine availability, selection or ordinary standalone setup errors.
pub fn repair(
    config: &Config,
    function: &str,
    err: &mut dyn Write,
) -> Result<sail_api::repair::BatchResult> {
    if !function.contains("::") || function.starts_with('-') || super::partial(&config.args) {
        return Err(Error::new(
            "SAIL3120",
            "standalone repair requires an exact crate-qualified function and full file scope",
        ));
    }
    let (request, manifest_sha256) = crate::engine::request(config, Some(function.to_owned()))?;
    let mut config = config.clone();
    config.force = true;
    let full = match run_with_engine(&config, &mut Vec::new(), err, false, Some(&request)) {
        Ok(full) => full,
        Err(fault) => {
            let mut full = run(&config, &mut Vec::new(), err, false)?;
            full.engine = Some(request.identity());
            for target in &mut full.targets {
                target.report.engine =
                    Some(request.provenance(sail_api::output::EngineEstablishment::StockFallback));
            }
            let compiler_invocations =
                full.execution.verifier_invocations + u64::from(fault.code == "SAIL3123");
            return Ok(sail_api::repair::BatchResult {
                focus: None,
                full: Some(full),
                manifest_sha256,
                compiler_invocations,
                batched: false,
                selection_error: None,
                engine_fault: Some(sail_api::output::ErrorInfo {
                    code: fault.code.into(),
                    message: fault.to_string(),
                }),
            });
        }
    };
    let path = config
        .cache
        .join("standalone")
        .join(digest(config.workspace.as_os_str().as_encoded_bytes()))
        .join(&full.context_sha256)
        .join("runs")
        .join(&full.run_id);
    let focus = standalone_focus(&config, &full, function, &path);
    let (focus, selection_error) = match focus {
        Ok(focus) => (Some(focus), None),
        Err(error) => (
            None,
            Some(sail_api::output::ErrorInfo {
                code: error.code.into(),
                message: error.to_string(),
            }),
        ),
    };
    Ok(sail_api::repair::BatchResult {
        batched: focus.is_some(),
        focus,
        compiler_invocations: full.execution.verifier_invocations,
        full: Some(full),
        manifest_sha256,
        selection_error,
        engine_fault: None,
    })
}

fn standalone_focus(
    config: &Config,
    full: &ProjectResult,
    function: &str,
    run: &Path,
) -> Result<ProjectResult> {
    let mut bytes = Vec::new();
    std::fs::File::open(run.join("focus.source.json"))
        .and_then(|file| file.take(MAX_RECORD + 1).read_to_end(&mut bytes))
        .map_err(|e| Error::new("SAIL3122", e.to_string()))?;
    if bytes.len() as u64 > MAX_RECORD {
        return Err(Error::new("SAIL3122", "focused evidence exceeds limit"));
    }
    let source: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| Error::new("SAIL3122", e.to_string()))?;
    if source["function"].as_str() != Some(function) {
        return Err(Error::new(
            "SAIL3122",
            "exact standalone repair function was not established",
        ));
    }
    let logs = attribution::read_logs(&run.join("logs/focus")).unwrap_or_default();
    let attr = attribution::attribute(
        &[source["verus"].clone()],
        Some(&logs),
        &attribution::Provenance::NoCache,
        None,
    );
    let mut report = crate::report::build(&crate::report::Inputs {
        args: &config.args,
        exit_code: Some(i32::from(
            source["verus"]["verification-results"]["errors"]
                .as_u64()
                .unwrap_or(1)
                > 0
                || source["verus"]["verification-results"]["encountered-error"] == true
                || source["verus"]["verification-results"]["encountered-vir-error"] == true,
        )),
        solver_path: Some(config.z3.display().to_string()),
        solver_sha256: Some(file_digest(&config.z3)?),
        daemon: false,
        attribution: &attr,
        diagnostics: &[],
    });
    report.diagnostics = source["diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
        .map(crate::engine_events::diagnostic)
        .collect();
    let selected = report
        .crates
        .iter()
        .flat_map(|krate| &krate.functions)
        .filter(|item| item.function == function)
        .count();
    if selected != 1 {
        return Err(Error::new(
            "SAIL3122",
            "standalone repair function is missing or ambiguous",
        ));
    }
    report.command.extend([
        "--".into(),
        "--sail-repair-function".into(),
        function.into(),
    ]);
    for krate in &mut report.crates {
        krate.functions.retain(|item| item.function == function);
        for item in &mut krate.functions {
            item.establishment = sail_api::output::EstablishmentName::Provisional;
            item.evidence_tier = None;
            item.verdict = sail_api::output::VerdictName::Provisional;
        }
    }
    report.summary = sail_api::output::Summary::of(&report.crates);
    let mut result = full.clone();
    result.exit_code = report.exit_code;
    for target in &mut result.targets {
        target.report = report.clone();
        target.complete = false;
        target.reused = false;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
    fn temp(name: &str) -> Result<PathBuf> {
        let path = std::env::temp_dir().join(format!(
            "sail-file-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| fail(e.to_string()))?
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).map_err(|e| fail(e.to_string()))?;
        Ok(path)
    }
    #[test]
    fn cached_runs_direct_session_records_to_the_records_they_read() -> TestResult {
        // #684: the shim writes session records only under
        // `SAIL_PROVENANCE_DIR`; standalone runs never set it and read the
        // trace directory, so every check of a cached run had no source.
        let run = temp("provenance")?;
        let configured = |command: &Command| {
            command
                .get_envs()
                .find(|(key, _)| *key == "SAIL_PROVENANCE_DIR")
                .map(|(_, value)| value.map(PathBuf::from))
        };
        let mut cached = Command::new("verus");
        let dir = session_records(&mut cached, &run, true)?;
        assert_eq!(configured(&cached), Some(Some(dir.clone())));
        assert!(dir.is_dir());
        let record = attribution::SessionRecord {
            version: 1,
            input_sha256: "session".into(),
            replies_served: 2,
            solver_started: false,
        };
        std::fs::write(
            dir.join("1-1.json"),
            format!(
                "{{\"version\":{},\"input_sha256\":\"{}\",\"replies_served\":{},\"solver_started\":{}}}",
                record.version, record.input_sha256, record.replies_served, record.solver_started
            ),
        )?;
        assert_eq!(attribution::read_session_records(&dir).len(), 1);
        // Without a cache an inherited value is removed, not forwarded.
        let mut uncached = Command::new("verus");
        uncached.env("SAIL_PROVENANCE_DIR", "/elsewhere");
        session_records(&mut uncached, &run, false)?;
        assert_eq!(configured(&uncached), Some(None));
        std::fs::remove_dir_all(run)?;
        Ok(())
    }
    #[test]
    fn file_selection_does_not_reinterpret_cargo_or_verus_option_values() {
        for args in [
            vec!["--manifest-path", "file.rs"],
            vec!["--", "file.rs"],
            vec!["-p", "proof.rs"],
        ] {
            assert!(source(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_none());
        }
        assert_eq!(source(&["proof.rs".into(), "--".into()]), Some("proof.rs"));
    }
    #[test]
    fn side_effects_and_unbound_search_inputs_always_execute_freshly() {
        for flag in [
            "--compile",
            "--no-verify",
            "--extern",
            "-L",
            "--sysroot",
            "--log",
            "--verify-function",
            "-o",
            "--emit",
            "--cfg",
        ] {
            assert!(!reusable(&[flag.into()]));
        }
        assert!(reusable(&[
            "--rlimit".into(),
            "30".into(),
            "--crate-type=lib".into()
        ]));
    }
    #[test]
    fn namespace_changes_and_same_length_content_edits_change_revision() -> TestResult {
        let root = temp("revision")?;
        let cache = root.join("cache");
        std::fs::create_dir_all(&cache)?;
        let file = root.join("proof.rs");
        std::fs::write(&file, "proof one")?;
        let first = observe(&root, &cache, std::iter::once(file.clone()))?;
        std::fs::write(&file, "proof two")?;
        assert!(!unchanged(
            &first,
            &observe(&root, &cache, std::iter::once(file.clone()))?
        ));
        assert_ne!(
            first,
            observe(&root, &cache, std::iter::once(file.clone()))?
        );
        std::fs::write(&file, "proof one")?;
        assert_eq!(
            first,
            observe(&root, &cache, std::iter::once(file.clone()))?
        );
        std::fs::write(root.join("helper.rs"), "new module")?;
        assert_ne!(first, observe(&root, &cache, std::iter::once(file))?);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
    #[test]
    fn external_inline_module_ancestors_cover_alternative_module_files() -> TestResult {
        let root = temp("external-inline")?;
        let workspace = root.join("work");
        let leaf = root.join("external/inner/leaf");
        std::fs::create_dir_all(&workspace)?;
        std::fs::create_dir_all(&leaf)?;
        let source = workspace.join("proof.rs");
        let module = workspace.join("../external/inner/leaf/mod.rs");
        std::fs::write(&source, "inline modules")?;
        std::fs::write(&module, "original module")?;
        let files = [source, module];
        let cache = root.join("cache");
        let before = observe(&workspace, &cache, files.iter().cloned())?;
        std::fs::write(root.join("external/inner/leaf.rs"), "ambiguous alternative")?;
        let after = observe(&workspace, &cache, files.into_iter())?;
        assert!(!unchanged(&before, &after));
        assert_ne!(before, after);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unread_symlinked_module_candidates_bind_existence() -> TestResult {
        let root = temp("module-candidates")?;
        let workspace = root.join("work");
        let external = root.join("external");
        std::fs::create_dir_all(&workspace)?;
        std::fs::create_dir_all(&external)?;
        let source = workspace.join("leaf.rs");
        std::fs::write(&source, "valid module")?;
        std::os::unix::fs::symlink(&external, workspace.join("leaf"))?;
        let cache = root.join("cache");
        let before = observe(&workspace, &cache, std::iter::once(source.clone()))?;
        std::fs::write(external.join("mod.rs"), "ambiguous alternative")?;
        let after = observe(&workspace, &cache, std::iter::once(source))?;
        assert_ne!(before, after);
        let missing = root.join("missing.rs");
        std::os::unix::fs::symlink(&missing, workspace.join("other.rs"))?;
        let before = observe(&workspace, &cache, std::iter::empty())?;
        std::fs::write(missing, "newly resolved source")?;
        assert_ne!(before, observe(&workspace, &cache, std::iter::empty())?);
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn malformed_and_corrupted_records_never_authorize_reuse() -> TestResult {
        let root = temp("record")?;
        let path = root.join("record");
        for bytes in [
            b"".as_slice(),
            b"{}",
            b"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff\n{}",
        ] {
            std::fs::write(&path, bytes)?;
            assert!(record(&path).is_none());
        }
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
