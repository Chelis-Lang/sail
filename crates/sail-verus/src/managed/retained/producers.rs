//! Actual successful producer certificates for this one workspace's copied graph.
//!
//! Older successful generations may retain their original run/PID certificate;
//! no endpoint source read manufactures a new producer. Newly executed native
//! units always compile and need their own actual compiler-read document.

use super::super::{
    dependency_snapshot::{Content, Snapshot, read_content},
    files, receipts,
};
use super::{Plan, archive, compiler, driver, native};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u32,
    context: String,
    compiler: String,
    policy: String,
    establishing_run: String,
    key: String,
    native: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shipped: Option<super::super::shipped::RetainedProof>,
    official: bool,
    crate_name: String,
    pid: u32,
    document: PathBuf,
    document_content: Content,
    output: PathBuf,
    body: Content,
}

pub(super) fn environment() -> Option<BTreeMap<String, String>> {
    std::env::vars_os()
        .map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
        .collect()
}

fn record_path(plan: &Plan, output: &Path) -> PathBuf {
    record_at(&plan.root, output)
}

fn record_at(root: &Path, output: &Path) -> PathBuf {
    root.join("producers")
        .join(files::digest(output.as_os_str().as_encoded_bytes()))
}

#[cfg(test)]
pub(super) fn has_shipped_producer(plan: &Plan, graph: &Snapshot) -> bool {
    graph.original_inputs().keys().any(|path| {
        receipts::read::<Record>(&record_path(plan, path))
            .is_some_and(|record| record.shipped.is_some() && record.crate_name == "vstd")
    })
}

/// Every original body must match a retained successful producer certificate.
/// A full new snapshot is taken after waiting, because pipelined metadata can
/// precede its producer's final rlib.
pub(super) fn established(plan: &Plan, context: &str, graph: &Snapshot) -> bool {
    // Every input is checked in full, several at once.
    let inputs: Vec<_> = graph.original_inputs().iter().collect();
    crate::parallel::in_order(&inputs, |&(path, content)| {
        let Some(record): Option<Record> = receipts::read(&record_path(plan, path)) else {
            return false;
        };
        record.version == 1
            && record.context == context
            && record.compiler == plan.compiler_identity()
            && record.policy == plan.closure.policy_digest()
            && record.official
            && certifies(&record, path, content)
            && !record.establishing_run.is_empty()
            && read_content(&record.document, 8 * 1024 * 1024).as_ref()
                == Some(&record.document_content)
            && if let Some(proof) = &record.shipped {
                !record.native
                    && record.pid == 0
                    && proof.compiler_identity() == record.compiler
                    && proof.intact(path, content)
            } else if record.native {
                native::intact(
                    &record.document,
                    &record.document_content.sha256,
                    record.pid,
                    &record.establishing_run,
                    &record.key,
                )
            } else {
                std::fs::read(&record.document).is_ok_and(|raw| {
                    compiler::stored_header(&raw, &record.key, &record.establishing_run)
                })
            }
    })
    .into_iter()
    .all(|established| established)
}

/// Whether `record` certifies the body a graph copied from `path`: the same
/// output, the same bytes, and the original still holding exactly them. The
/// copy may have been taken before its producer finished (ADR 0061); this
/// complete read after the consumer's compiler is what binds the two.
fn certifies(record: &Record, path: &Path, content: &Content) -> bool {
    record.output == path
        && record.body == *content
        && read_content(path, 256 * 1024 * 1024).as_ref() == Some(content)
}

/// Import a genuinely served release proof, preserving its distinct authority.
/// This runs after the ordinary driver finish; it is not an actual compiler
/// certificate and cannot make a requested-target retained candidate by itself.
pub(crate) fn publish_shipped(
    runtime: &receipts::Runtime,
    proof: &super::super::shipped::RetainedProof,
) -> Option<()> {
    let plan = runtime.retained.as_ref()?;
    if !proof.for_run(&runtime.run_id, plan.compiler_identity(), &plan.target)
        || !plan.closure.check_sources()
        || proof.outputs().is_empty()
        || !proof
            .outputs()
            .iter()
            .all(|(path, body)| proof.intact(path, body))
    {
        return None;
    }
    let document = proof.document()?;
    let content = read_content(&document, 8 * 1024 * 1024)?;
    let template = Record {
        version: 1,
        context: runtime.context_sha256.clone(),
        compiler: plan.compiler_identity().to_owned(),
        policy: plan.closure.policy_digest(),
        establishing_run: runtime.run_id.clone(),
        key: content.sha256.clone(),
        native: false,
        shipped: Some(proof.clone()),
        official: true,
        crate_name: "vstd".to_owned(),
        pid: 0,
        document,
        document_content: content.clone(),
        output: PathBuf::new(),
        body: content,
    };
    publish(
        plan,
        &template,
        proof.outputs().iter().map(|(p, b)| (p.clone(), b.clone())),
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "The shared completed child binds its actual PID, raw read document, source closure and immutable graph independently for retained use."
)]
pub(in crate::managed) fn publish_shipped_native(
    runtime: &receipts::Runtime,
    args: &[OsString],
    cwd: &Path,
    graph: &Snapshot,
    outputs: &[PathBuf],
    document: &Path,
    pid: u32,
    key: &str,
) -> Option<()> {
    let plan = runtime.retained.as_ref()?;
    let environment = environment()?;
    let manifest = Path::new(environment.get("CARGO_MANIFEST_DIR")?).join("Cargo.toml");
    if !plan.compiler_matches(&runtime.original_wrapper)
        || !plan.closure.unit(&environment, args)
        || !plan.native_edges(args, &manifest)
        || !graph.intact()
        || !ready(plan, &runtime.run_id, graph)
        || !established(plan, &runtime.context_sha256, graph)
        || !named_externs(plan, args)
    {
        return None;
    }
    let reads = native::Observation::completed(document, pid, &runtime.run_id, key)?;
    let sha = reads.finish(
        true,
        cwd,
        &plan.closure,
        &graph.copied_inputs()?,
        &environment,
    )?;
    let content = read_content(document, 8 * 1024 * 1024)?;
    if content.sha256 != sha {
        return None;
    }
    let template = Record {
        version: 1,
        context: runtime.context_sha256.clone(),
        compiler: plan.compiler_identity().to_owned(),
        policy: plan.closure.policy_digest(),
        establishing_run: runtime.run_id.clone(),
        key: key.to_owned(),
        native: true,
        shipped: None,
        official: true,
        crate_name: driver::value(args, "--crate-name")?.to_owned(),
        pid,
        document: document.to_owned(),
        document_content: content.clone(),
        output: PathBuf::new(),
        body: content,
    };
    let outputs = outputs
        .iter()
        .map(|p| Some((p.clone(), read_content(p, 256 * 1024 * 1024)?)))
        .collect::<Option<Vec<_>>>()?;
    publish(plan, &template, outputs.into_iter())
}

pub(super) fn named_externs(plan: &Plan, args: &[OsString]) -> bool {
    args.iter().enumerate().all(|(index, arg)| {
        let Some(text) = arg.to_str() else {
            return false;
        };
        let Some(external) = text
            .strip_prefix("--extern=")
            .or_else(|| (index > 0 && args[index - 1] == "--extern").then_some(text))
        else {
            return true;
        };
        if external == "proc_macro" {
            return true;
        }
        let Some((name, path)) = external.split_once('=') else {
            return false;
        };
        receipts::read::<Record>(&record_path(plan, Path::new(path)))
            .is_some_and(|record| record.official && record.crate_name == name)
    })
}

pub(super) fn ready(plan: &Plan, run: &str, graph: &Snapshot) -> bool {
    ready_for(
        plan,
        run,
        &graph.original_inputs().keys().cloned().collect(),
    )
}

/// Waits while any artifact lacks a certificate and its producer is still
/// running in this run, then requires every certificate. A producer that
/// ended without one, or ran in no driver of this run, is not waited for. A
/// driver that died while running is waited for at most [`RUNNING_LIMIT`].
pub(super) fn ready_for(plan: &Plan, run: &str, artifacts: &BTreeSet<PathBuf>) -> bool {
    ready_in(&plan.root, run, artifacts, RUNNING_LIMIT)
}

fn ready_in(root: &Path, run: &str, artifacts: &BTreeSet<PathBuf>, limit: Duration) -> bool {
    let certified = |path: &PathBuf| record_at(root, path).is_file();
    await_ready(
        || {
            artifacts
                .iter()
                .all(|path| certified(path) || !running(root, run, path))
        },
        limit,
    ) && artifacts.iter().all(certified)
}

/// The longest wait for a producer whose driver never ends.
const RUNNING_LIMIT: Duration = Duration::from_secs(120);

fn running_at(root: &Path, output: &Path) -> PathBuf {
    root.join("running")
        .join(files::digest(output.as_os_str().as_encoded_bytes()))
}

/// Whether a driver of this run is still producing `output`. Marks are bound
/// to their run: one left by an earlier run's driver means nothing now.
fn running(root: &Path, run: &str, output: &Path) -> bool {
    std::fs::read(running_at(root, output)).is_ok_and(|mark| mark == run.as_bytes())
}

/// Every output a compiler invocation can write into its output directory
/// that a dependent may read: its library, metadata, macro and VIR files.
fn unit_outputs(args: &[OsString]) -> Option<Vec<PathBuf>> {
    let out = PathBuf::from(driver::value(args, "--out-dir")?);
    let name = driver::value(args, "--crate-name")?;
    let extra = args.iter().enumerate().find_map(|(i, arg)| {
        arg.to_str()?.strip_prefix("-Cextra-filename=").or_else(|| {
            (arg == "-C")
                .then(|| args.get(i + 1)?.to_str()?.strip_prefix("extra-filename="))
                .flatten()
        })
    })?;
    Some(
        ["rmeta", "rlib", "vir", std::env::consts::DLL_EXTENSION]
            .iter()
            .map(|extension| out.join(format!("lib{name}{extra}.{extension}")))
            .collect(),
    )
}

/// Marks this driver's outputs as being produced in this run, from before
/// its compiler starts until the driver ends, however it ends, and after any
/// certificate it writes. Dependents that Cargo starts on this unit's
/// metadata wait on the mark instead of a fixed timeout.
pub(crate) struct Running {
    marks: Vec<PathBuf>,
}

impl Running {
    pub(crate) fn begin(runtime: &receipts::Runtime, args: &[OsString]) -> Option<Self> {
        let plan = runtime.retained.as_ref()?;
        let marks: Vec<_> = unit_outputs(args)?
            .iter()
            .map(|output| running_at(&plan.root, output))
            .collect();
        let directory = plan.root.join("running");
        std::fs::create_dir_all(&directory).ok()?;
        let mut written = Vec::new();
        for mark in marks {
            // Without a mark, a dependent simply does not wait: it requires
            // the certificate at once and otherwise misses.
            if std::fs::write(&mark, runtime.run_id.as_bytes()).is_ok() {
                written.push(mark);
            }
        }
        Some(Self { marks: written })
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        for mark in &self.marks {
            let _ = std::fs::remove_file(mark);
        }
    }
}

fn await_ready(mut ready: impl FnMut() -> bool, maximum: Duration) -> bool {
    let start = Instant::now();
    loop {
        if ready() {
            return true;
        }
        let Some(remaining) = maximum.checked_sub(start.elapsed()) else {
            return false;
        };
        std::thread::sleep(remaining.min(Duration::from_millis(10)));
    }
}

fn publish(
    plan: &Plan,
    template: &Record,
    outputs: impl Iterator<Item = (PathBuf, Content)>,
) -> Option<()> {
    let certificates = plan.root.join("certificates");
    let retained_document = certificates.join(&template.document_content.sha256);
    // Materialize the bounded record payloads before the lock. Reserve their
    // full additional size, including replacements, plus the checked document.
    let records = outputs
        .take(33)
        .map(|(path, body)| {
            (
                record_path(plan, &path),
                Record {
                    output: path,
                    body,
                    document: retained_document.clone(),
                    ..template.clone()
                },
            )
        })
        .collect::<Vec<_>>();
    if records.len() > 32 || template.document_content.size > 8 * 1024 * 1024 {
        return None;
    }
    let bytes = records
        .iter()
        .try_fold(8 * 1024 * 1024 + 1u64, |total, (_, record)| {
            total.checked_add(u64::try_from(serde_json::to_vec(record).ok()?.len()).ok()?)
        })?;
    let _admission = super::budget::admit(
        &plan.root,
        bytes,
        u64::try_from(records.len()).ok()?.checked_add(4)?,
    )?;
    std::fs::create_dir_all(plan.root.join("producers")).ok()?;
    std::fs::create_dir_all(&certificates).ok()?;
    if read_content(&retained_document, 8 * 1024 * 1024).as_ref()
        != Some(&template.document_content)
    {
        let source = super::super::dependency_snapshot::open(&template.document)?;
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&retained_document)
            .ok()?;
        let mut limited = source.take(8 * 1024 * 1024 + 1);
        std::io::copy(&mut limited, &mut output).ok()?;
        output.flush().ok()?;
        if read_content(&retained_document, 8 * 1024 * 1024).as_ref()
            != Some(&template.document_content)
        {
            return None;
        }
    }
    for (path, record) in records {
        files::json(&path, &record).ok()?;
    }
    Some(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "One certificate binds the already-admitted actual invocation, raw document and exact emitted bodies."
)]
pub(super) fn publish_verified(
    plan: &Plan,
    context: &str,
    run: &str,
    key: &str,
    document: &Path,
    document_body: &Content,
    outputs: &[archive::Output],
    variables: &BTreeMap<String, String>,
    arguments: &[OsString],
) -> Option<()> {
    // stored_header binds both successful phases; full source/graph admission
    // happened in Attempt::finish before invoking this publication function.
    let raw = std::fs::read(document).ok()?;
    if !compiler::stored_header(&raw, key, run) {
        return None;
    }
    let parsed: serde_json::Value = serde_json::from_slice(&raw).ok()?;
    let pid = u32::try_from(parsed["pid"].as_u64()?).ok()?;
    let template = Record {
        version: 1,
        context: context.to_owned(),
        compiler: plan.compiler_identity().to_owned(),
        policy: plan.closure.policy_digest(),
        establishing_run: run.to_owned(),
        key: key.to_owned(),
        native: false,
        shipped: None,
        official: plan.closure.unit(variables, arguments),
        crate_name: driver::value(arguments, "--crate-name")?.to_owned(),
        pid,
        document: document.to_owned(),
        document_content: document_body.clone(),
        output: PathBuf::new(),
        body: document_body.clone(),
    };
    publish(
        plan,
        &template,
        outputs
            .iter()
            .filter(|output| output.path.extension().is_some_and(|ext| ext != "d"))
            .map(|output| (output.path.clone(), output.content.clone())),
    )
}

/// Optional observation; refusing it leaves the original compiler invocation.
pub(crate) struct Native {
    plan: Plan,
    context: String,
    run: String,
    key: String,
    environment: BTreeMap<String, String>,
    cwd: PathBuf,
    graph: Snapshot,
    reads: native::Observation,
    outputs: Vec<PathBuf>,
    arguments: Vec<OsString>,
    wrapper: PathBuf,
}

impl Native {
    #[allow(
        clippy::too_many_lines,
        reason = "Native admission binds one source/feature unit, actual image and all successful dependency producers before spawn."
    )]
    pub(crate) fn begin(
        runtime: &receipts::Runtime,
        args: &[OsString],
        wrapper: &Path,
    ) -> Option<Self> {
        let plan = runtime.retained.as_ref()?;
        let environment = environment()?;
        if environment
            .get("__VERUS_DRIVER_VIA_CARGO__")
            .map(String::as_str)
            != Some("1")
            || !plan.compiler_matches(wrapper)
            || !plan.closure.unit(&environment, args)
        {
            return None;
        }
        let source = driver::ordinary_source(args, &plan.target)?;
        let manifest = Path::new(environment.get("CARGO_MANIFEST_DIR")?).join("Cargo.toml");
        if !plan.native_edges(args, &manifest) {
            return None;
        }
        let metadata: toml::Value =
            toml::from_str(&std::fs::read_to_string(&manifest).ok()?).ok()?;
        let name = metadata
            .get("lib")
            .and_then(|lib| lib.get("name"))
            .and_then(toml::Value::as_str)
            .map_or_else(
                || {
                    environment
                        .get("CARGO_PKG_NAME")
                        .map(|name| name.replace('-', "_"))
                },
                |name| Some(name.to_owned()),
            )?;
        let path = metadata
            .get("lib")
            .and_then(|lib| lib.get("path"))
            .and_then(toml::Value::as_str)
            .unwrap_or("src/lib.rs");
        let cwd = std::env::current_dir().ok()?;
        if cwd.join(source).canonicalize().ok()?
            != manifest.parent()?.join(path).canonicalize().ok()?
            || driver::value(args, "--crate-name")? != name
        {
            return None;
        }
        let macro_crate = metadata
            .get("lib")
            .and_then(|lib| lib.get("proc-macro"))
            .and_then(toml::Value::as_bool)
            == Some(true);
        let kind = driver::value(args, "--crate-type")?;
        if if macro_crate {
            kind != "proc-macro"
        } else {
            kind != "lib" && kind != "rlib"
        } {
            return None;
        }
        let outputs = outputs(args, &plan.target, macro_crate)?;
        if outputs.iter().any(|path| path.try_exists().unwrap_or(true)) {
            return None;
        }
        // Withdraw a previous generation's certificate before Cargo can
        // pipeline this generation's metadata to a dependent.
        for output in &outputs {
            match std::fs::remove_file(record_path(plan, output)) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return None,
            }
        }
        let key = files::digest(
            &serde_json::to_vec(&(args, &environment, &runtime.context_sha256)).ok()?,
        );
        // Temporary compiler observations belong to the managed run, which
        // already owns their lifecycle. Only checked certificates and graphs
        // enter the separately capped durable retained arena.
        let directory = runtime
            .sidecars
            .join(format!("retained-native-{key}-{}", std::process::id()));
        std::fs::create_dir_all(&directory).ok()?;
        // Cargo starts this unit once its dependencies' metadata exists, as
        // it would without Sail. The compiler reads only private copies of
        // what is present now; whether every copied body is a certified
        // producer output is decided after it finishes (ADR 0061).
        let graph = driver::publish_graph(
            &plan.root,
            Snapshot::capture_pipelined(&plan.target, &directory.join("graph"), args)?,
            args,
        )?;
        // publish_graph persists the shared graph marker before spawn.
        let reads = native::Observation::begin(&directory.join("read"), &runtime.run_id, &key)?;
        Some(Self {
            plan: plan.clone(),
            context: runtime.context_sha256.clone(),
            run: runtime.run_id.clone(),
            key,
            environment,
            cwd,
            graph,
            reads,
            outputs,
            arguments: args.to_vec(),
            wrapper: wrapper.to_owned(),
        })
    }

    pub(crate) fn arguments(&self) -> &[OsString] {
        self.graph.arguments()
    }
    pub(crate) fn configure(&self, command: &mut std::process::Command) {
        self.reads.configure(command);
    }
    pub(crate) fn started(&mut self, pid: u32) {
        self.reads.started(pid);
    }

    pub(crate) fn finish(&self, success: bool) -> Option<()> {
        // The producers of the copied inputs may still be running: wait for
        // each to settle, then require all of their certificates.
        if !success
            || !ready(&self.plan, &self.run, &self.graph)
            || !named_externs(&self.plan, &self.arguments)
            || !self.graph.intact()
            || !established(&self.plan, &self.context, &self.graph)
            || !self.plan.compiler_matches(&self.wrapper)
            || !self.plan.closure.unit(&self.environment, &self.arguments)
        {
            return None;
        }
        let sha = self.reads.finish(
            true,
            &self.cwd,
            &self.plan.closure,
            &self.graph.copied_inputs()?,
            &self.environment,
        )?;
        let content = read_content(self.reads.path(), 8 * 1024 * 1024)?;
        if content.sha256 != sha {
            return None;
        }
        let template = Record {
            version: 1,
            context: self.context.clone(),
            compiler: self.plan.compiler_identity().to_owned(),
            policy: self.plan.closure.policy_digest(),
            establishing_run: self.run.clone(),
            key: self.key.clone(),
            native: true,
            shipped: None,
            official: true,
            crate_name: driver::value(&self.arguments, "--crate-name")?.to_owned(),
            pid: self.reads.pid()?,
            document: self.reads.path().to_owned(),
            document_content: content.clone(),
            output: PathBuf::new(),
            body: content,
        };
        let outputs = self
            .outputs
            .iter()
            .map(|path| Some((path.clone(), read_content(path, 256 * 1024 * 1024)?)))
            .collect::<Option<Vec<_>>>()?;
        publish(&self.plan, &template, outputs.into_iter())
    }
}

fn outputs(args: &[OsString], target: &Path, macro_crate: bool) -> Option<Vec<PathBuf>> {
    let out = PathBuf::from(driver::value(args, "--out-dir")?);
    if out.canonicalize().ok()? != out || !out.starts_with(target) {
        return None;
    }
    let name = driver::value(args, "--crate-name")?;
    let extra = args.iter().enumerate().find_map(|(i, arg)| {
        arg.to_str()?.strip_prefix("-Cextra-filename=").or_else(|| {
            (arg == "-C")
                .then(|| args.get(i + 1)?.to_str()?.strip_prefix("extra-filename="))
                .flatten()
        })
    })?;
    if name.is_empty()
        || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        || extra.is_empty()
        || !extra
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
    {
        return None;
    }
    let stem = format!("{name}{extra}");
    let mut outputs = Vec::new();
    for emit in driver::value(args, "--emit")?.split(',') {
        match emit {
            "dep-info" => {}
            "metadata" => outputs.push(out.join(format!("lib{stem}.rmeta"))),
            "link" => outputs.push(out.join(format!(
                "lib{stem}.{}",
                if macro_crate {
                    std::env::consts::DLL_EXTENSION
                } else {
                    "rlib"
                }
            ))),
            _ => return None,
        }
    }
    (!outputs.is_empty()).then_some(outputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scratch(name: &str) -> std::io::Result<PathBuf> {
        let root =
            std::env::temp_dir().join(format!("sail-retained-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("producers"))?;
        std::fs::create_dir_all(root.join("running"))?;
        Ok(root)
    }

    #[test]
    fn dependents_wait_only_for_producers_running_in_this_run()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = scratch("running")?;
        let output = root.join("deps/libp-1.rmeta");
        let artifacts: BTreeSet<PathBuf> = [output.clone()].into();
        let limit = Duration::from_secs(10);
        // Nothing runs and nothing is certified: refuse at once.
        let start = Instant::now();
        assert!(!ready_in(&root, "run", &artifacts, limit));
        assert!(start.elapsed() < Duration::from_secs(2));
        // A mark left by another run is not a running producer.
        std::fs::write(running_at(&root, &output), b"earlier")?;
        assert!(!ready_in(&root, "run", &artifacts, limit));
        assert!(start.elapsed() < Duration::from_secs(2));
        // A producer of this run that certifies, then ends: accepted.
        std::fs::write(running_at(&root, &output), b"run")?;
        let (record, mark) = (record_at(&root, &output), running_at(&root, &output));
        let producer = std::thread::spawn(move || -> std::io::Result<()> {
            std::thread::sleep(Duration::from_millis(50));
            std::fs::write(&record, b"{}")?;
            std::fs::remove_file(&mark)
        });
        assert!(ready_in(&root, "run", &artifacts, limit));
        assert!(producer.join().is_ok_and(|result| result.is_ok()));
        // A producer of this run that ends without a certificate: refused as
        // soon as it ends, not at the limit.
        std::fs::remove_file(record_at(&root, &output))?;
        std::fs::write(running_at(&root, &output), b"run")?;
        let mark = running_at(&root, &output);
        let producer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            std::fs::remove_file(mark)
        });
        let start = Instant::now();
        assert!(!ready_in(&root, "run", &artifacts, limit));
        assert!(start.elapsed() < Duration::from_secs(2));
        assert!(producer.join().is_ok_and(|result| result.is_ok()));
        // A producer whose driver died keeps its mark: bounded, then refused.
        std::fs::write(running_at(&root, &output), b"run")?;
        assert!(!ready_in(
            &root,
            "run",
            &artifacts,
            Duration::from_millis(30)
        ));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn certification_after_the_compile_rejects_bytes_the_early_check_rejected()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = scratch("certifies")?;
        let original = root.join("libp-1.rmeta");
        std::fs::write(&original, b"metadata the consumer copied")?;
        // What a pipelined capture records for the copy it compiles against.
        let copied = read_content(&original, 1024).ok_or("copied content")?;
        let record = |body: &Content| Record {
            version: 1,
            context: "context".to_owned(),
            compiler: "compiler".to_owned(),
            policy: "policy".to_owned(),
            establishing_run: "run".to_owned(),
            key: "key".to_owned(),
            native: true,
            shipped: None,
            official: true,
            crate_name: "p".to_owned(),
            pid: 1,
            document: root.join("document"),
            document_content: body.clone(),
            output: original.clone(),
            body: body.clone(),
        };
        assert!(certifies(&record(&copied), &original, &copied));
        // The producer finished and certified other bytes than were copied.
        let other = Content {
            sha256: files::digest(b"metadata the producer finally wrote"),
            size: copied.size,
        };
        assert!(!certifies(&record(&other), &original, &copied));
        // The original changed after the copy, even back to the same length.
        std::fs::write(&original, b"metadata the consumer copiee")?;
        assert!(!certifies(&record(&copied), &original, &copied));
        // A certificate for a different output never certifies this one.
        let mut elsewhere = record(&copied);
        elsewhere.output = root.join("libq-1.rmeta");
        std::fs::write(&original, b"metadata the consumer copied")?;
        assert!(!certifies(&elsewhere, &original, &copied));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn running_marks_cover_every_dependent_visible_output_and_end_with_the_driver()
    -> Result<(), Box<dyn std::error::Error>> {
        let args: Vec<OsString> = [
            "--crate-name",
            "p",
            "--out-dir",
            "/t/deps",
            "-C",
            "extra-filename=-1",
        ]
        .iter()
        .map(OsString::from)
        .collect();
        let outputs = unit_outputs(&args).ok_or("outputs")?;
        for extension in ["rmeta", "rlib", "vir", std::env::consts::DLL_EXTENSION] {
            assert!(outputs.contains(&PathBuf::from(format!("/t/deps/libp-1.{extension}"))));
        }
        // Without an extra filename the unit is not one Cargo names: no marks.
        assert!(unit_outputs(&args[..4]).is_none());
        // Dropping the driver's guard removes its marks, so a dependent stops
        // waiting as soon as the producer's driver ends, certified or not.
        let root = scratch("marks")?;
        let output = root.join("deps/libp-1.rmeta");
        std::fs::write(running_at(&root, &output), b"run")?;
        assert!(running(&root, "run", &output));
        assert!(!running(&root, "other", &output));
        drop(Running {
            marks: vec![running_at(&root, &output)],
        });
        assert!(!running(&root, "run", &output));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn a_dependent_of_a_failed_producer_is_never_certified()
    -> Result<(), Box<dyn std::error::Error>> {
        // A dependent copied P's metadata while P ran. P then fails (no
        // certificate) or certifies other bytes: the dependent's finish waits
        // only until P's driver ends and then refuses; nothing is reused.
        let root = scratch("failed")?;
        let output = root.join("deps/libp-1.rmeta");
        std::fs::create_dir_all(root.join("deps"))?;
        std::fs::write(&output, b"metadata copied mid-build")?;
        let copied = read_content(&output, 1024).ok_or("copied")?;
        let artifacts: BTreeSet<PathBuf> = [output.clone()].into();
        std::fs::write(running_at(&root, &output), b"run")?;
        let mark = running_at(&root, &output);
        let (rewritten, record) = (output.clone(), record_at(&root, &output));
        let producer = std::thread::spawn(move || -> std::io::Result<()> {
            std::thread::sleep(Duration::from_millis(50));
            // P rewrites its metadata, certifies those bytes and ends.
            std::fs::write(&rewritten, b"metadata P finally wrote!")?;
            std::fs::write(&record, b"{}")?;
            std::fs::remove_file(mark)
        });
        let start = Instant::now();
        // The producer settled with a certificate, so the wait succeeds...
        assert!(ready_in(&root, "run", &artifacts, Duration::from_secs(10)));
        assert!(start.elapsed() < Duration::from_secs(2));
        assert!(producer.join().is_ok_and(|result| result.is_ok()));
        // ...but the certified bytes are not the ones the dependent copied.
        let final_body = read_content(&output, 1024).ok_or("final")?;
        let record = Record {
            version: 1,
            context: "context".to_owned(),
            compiler: "compiler".to_owned(),
            policy: "policy".to_owned(),
            establishing_run: "run".to_owned(),
            key: "key".to_owned(),
            native: true,
            shipped: None,
            official: true,
            crate_name: "p".to_owned(),
            pid: 1,
            document: root.join("document"),
            document_content: final_body.clone(),
            output: output.clone(),
            body: final_body,
        };
        assert!(!certifies(&record, &output, &copied));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    fn final_native_graph_waits_for_later_rlib_record() -> Result<(), Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!(
            "sail-retained-producer-ready-{}",
            std::process::id()
        ));
        std::fs::create_dir(&root)?;
        let metadata = root.join("rmeta.json");
        let library = root.join("rlib.json");
        std::fs::write(&metadata, b"metadata producer completed")?;
        let delayed = library.clone();
        let producer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            std::fs::write(delayed, b"later library certificate")
        });
        assert!(await_ready(
            || metadata.is_file() && library.is_file(),
            Duration::from_secs(1)
        ));
        assert!(producer.join().is_ok_and(|result| result.is_ok()));
        std::fs::remove_file(&library)?;
        assert!(!await_ready(|| library.is_file(), Duration::ZERO));
        std::fs::remove_dir_all(root)?;
        Ok(())
    }
}
