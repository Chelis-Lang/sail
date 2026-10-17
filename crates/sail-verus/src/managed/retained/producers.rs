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
use std::collections::BTreeMap;
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

fn policy() -> String {
    files::digest(include_bytes!(
        "../../../../../engine/verus/retained-vest-inputs.json"
    ))
}

fn record_path(plan: &Plan, output: &Path) -> PathBuf {
    plan.root
        .join("producers")
        .join(files::digest(output.as_os_str().as_encoded_bytes()))
}

/// Every original body must match a retained successful producer certificate.
/// A full new snapshot is taken after waiting, because pipelined metadata can
/// precede its producer's final rlib.
pub(super) fn established(plan: &Plan, context: &str, graph: &Snapshot) -> bool {
    graph.original_inputs().iter().all(|(path, content)| {
        let Some(record): Option<Record> = receipts::read(&record_path(plan, path)) else {
            return false;
        };
        record.version == 1
            && record.context == context
            && record.compiler == plan.compiler_identity()
            && record.policy == policy()
            && record.official
            && record.output == *path
            && record.body == *content
            && read_content(path, 256 * 1024 * 1024).as_ref() == Some(content)
            && !record.establishing_run.is_empty()
            && read_content(&record.document, 8 * 1024 * 1024).as_ref()
                == Some(&record.document_content)
            && if record.native {
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

pub(super) fn ready(plan: &Plan, graph: &Snapshot) -> bool {
    await_ready(
        || {
            graph
                .original_inputs()
                .keys()
                .all(|path| record_path(plan, path).is_file())
        },
        Duration::from_secs(2),
    )
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
        policy: policy(),
        establishing_run: run.to_owned(),
        key: key.to_owned(),
        native: false,
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
        let first = Snapshot::capture_ready(&plan.target, &directory.join("first"), args)?;
        if !ready(plan, &first) {
            return None;
        }
        drop(first);
        let graph = driver::publish_graph(
            &plan.root,
            Snapshot::capture_ready(&plan.target, &directory.join("graph"), args)?,
            args,
        )?;
        if !ready(plan, &graph) || !established(plan, &runtime.context_sha256, &graph) {
            return None;
        }
        if !named_externs(plan, args) {
            return None;
        }
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
        if !success
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
            policy: policy(),
            establishing_run: self.run.clone(),
            key: self.key.clone(),
            native: true,
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
