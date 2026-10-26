//! Bind boundary inputs to actual successful native compiler invocations.
//!
//! Observation never replaces, buffers or changes the native compiler's output.
//! Missing provenance only disables the experimental vstd lookup.

use super::super::dependency_snapshot::Snapshot;
use super::{Call, Plan, native_reads, sha256_file};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[path = "native_shape.rs"]
mod native_shape;

#[cfg(test)]
#[path = "admission_actual_tests.rs"]
mod admission_actual_tests;

#[derive(Serialize, Deserialize)]
struct Record {
    runtime: String,
    policy: String,
    run_id: String,
    package: String,
    crate_name: String,
    compiler: String,
    inputs: String,
    output: PathBuf,
    sha256: String,
    compiler_pid: u32,
    source_observation: PathBuf,
    source_observation_sha256: String,
    native_graph_sha256: String,
}

/// Optional observation around one actual native compiler subprocess.
pub(crate) struct Observation<'a> {
    call: Call<'a>,
    generated: Option<BTreeMap<String, String>>,
    outputs: Vec<PathBuf>,
    graph: Snapshot,
    reads: native_reads::Observation,
    cwd: PathBuf,
    arguments: Vec<OsString>,
    compiler_options: Vec<String>,
    _producer: Producer,
}

/// Marks only an admitted observation that can publish these outputs. Marks
/// control waiting, never whether a producer certificate is accepted.
struct Producer {
    marks: Vec<PathBuf>,
}

impl Producer {
    fn begin(plan: &Plan, outputs: &[PathBuf]) -> Self {
        let mut marks = Vec::new();
        if std::fs::create_dir_all(plan.native_records.join("running")).is_ok() {
            for output in outputs {
                let (_, mark) = producer_paths(&plan.native_records, output);
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&mark)
                {
                    use std::io::Write as _;
                    marks.push(mark);
                    let _ = file.write_all(plan.run_id.as_bytes());
                }
            }
        }
        Self { marks }
    }
}

impl Drop for Producer {
    fn drop(&mut self) {
        for mark in &self.marks {
            let _ = std::fs::remove_file(mark);
        }
    }
}

impl<'a> Observation<'a> {
    pub(crate) fn begin(plan: &'a Plan, args: &'a [OsString]) -> Option<Self> {
        let observed = Self::try_begin(plan, args);
        if let Err(reason) = &observed {
            trace(
                args,
                &serde_json::json!({"event":"native-observation-refused","reason":reason}),
            );
        }
        observed.ok()
    }

    fn try_begin(plan: &'a Plan, args: &'a [OsString]) -> Result<Self, &'static str> {
        if !Snapshot::pipelined_inputs_admitted(&plan.target, args, |output| {
            let (record, running) = producer_paths(&plan.native_records, output);
            record.is_file()
                || std::fs::read(running).is_ok_and(|bytes| bytes == plan.run_id.as_bytes())
        }) {
            return Err("native lookup inputs lack current producer records or marks");
        }
        let mut call =
            Call::new(plan, args, false, std::env::vars_os()).ok_or("unkeyed native unit")?;
        if !call.out_dir.starts_with(&plan.target) {
            return Err("foreign native output directory");
        }
        let closure = plan.closure.as_ref().ok_or("no admitted source closure")?;
        if !closure.native_unit(&call.vars, args) {
            return Err("foreign native source, version or compiler feature");
        }
        // The compiler check reads the launcher and compiler in full. It runs
        // beside the read-only checks that follow; its refusal still comes
        // first, as when the checks ran one after another.
        let vars = call.vars.clone();
        let (compiler, checked) = std::thread::scope(|scope| {
            let compiler = std::thread::Builder::new()
                .name("sail-native-compiler".to_owned())
                .spawn_scoped(scope, || compiler_matches_in(&vars, plan))
                .ok();
            let checked = (|| {
                // Cargo pipelines dependents after metadata emission, before the
                // producer exits. Native units always compile; require successful
                // dependency provenance when completing their observation instead.
                let generated = generated(&call)?;
                let outputs =
                    emitted_outputs(&call).ok_or("unsupported native Cargo library invocation")?;
                if !matches!(plan.mode, super::Mode::MachineLocal { .. })
                    && !native_shape::admitted(&call)
                {
                    return Err("unsupported portable native compiler tuple");
                }
                if outputs.iter().any(|path| path.try_exists().unwrap_or(true)) {
                    return Err("native output already exists before this compiler invocation");
                }
                let compiler_options = bind_compiler_options(&mut call)
                    .ok_or("native compiler options could not be bound")?;
                Ok((generated, outputs, compiler_options))
            })();
            let compiler = match compiler {
                Some(thread) => thread.join().unwrap_or(false),
                None => compiler_matches_in(&vars, plan),
            };
            (compiler, checked)
        });
        if !compiler {
            return Err("native compiler is not the admitted sysroot compiler");
        }
        let (generated, outputs, compiler_options) = checked?;
        let base = plan
            .target
            .join(".sail-boundary-native-reads")
            .join(&plan.run_id)
            .join(format!("{}-{}", call.key, std::process::id()));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            builder.mode(0o700);
        }
        builder
            .recursive(true)
            .create(&base)
            .map_err(|_| "native observation directory unavailable")?;
        // Let Cargo keep metadata pipelining (ADR 0061). The child sees only
        // private copies of inputs already present. Missing producer
        // certificates still refuse publication after the child finishes.
        let graph = Snapshot::capture_pipelined(&plan.target, &base.join("graph"), args)
            .ok_or_else(|| {
                trace_inventory(&call, "pipelined");
                "native graph capture refused"
            })?;
        let reads = native_reads::Observation::begin(&base.join("read"), &plan.run_id, &call.key)
            .ok_or("native read request unavailable")?;
        let mut arguments = graph.arguments().to_vec();
        arguments.extend(compiler_options.iter().map(OsString::from));
        let producer = Producer::begin(plan, &outputs);
        Ok(Self {
            call,
            generated,
            outputs,
            graph,
            reads,
            cwd: std::env::current_dir().map_err(|_| "native cwd unavailable")?,
            arguments,
            compiler_options,
            _producer: producer,
        })
    }

    pub(crate) fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    pub(crate) fn configure(&self, command: &mut std::process::Command) {
        self.reads.configure(command);
    }

    pub(crate) fn started(&mut self, pid: u32) {
        self.reads.started(pid);
    }

    /// Called only after the real compiler returns successfully.
    pub(crate) fn complete(self, runtime: Option<&super::super::receipts::Runtime>) {
        if self.try_complete().is_some()
            && let Some(runtime) = runtime
            && let Some(pid) = self.reads.pid()
        {
            let _ = super::super::retained::publish_shipped_native(
                runtime,
                self.call.arguments,
                &self.cwd,
                &self.graph,
                &self.outputs,
                self.reads.path(),
                pid,
                &self.call.key,
            );
        }
    }

    fn try_complete(&self) -> Option<()> {
        let plan = self.call.plan;
        if !await_graph_records(&self.call, &self.graph)
            || !self.graph.intact()
            || !graph_established(&self.call, &self.graph)
        {
            return None;
        }
        let source_observation_sha256 = self.reads.finish(
            true,
            &self.cwd,
            plan.closure.as_ref()?,
            &self.graph.copied_inputs()?,
            &self.call.vars,
        )?;
        if !plan
            .closure
            .as_ref()?
            .native_unit(&self.call.vars, self.call.arguments)
        {
            return None;
        }
        // As at the start: the full compiler read runs beside the remaining
        // read-only checks, and all of them must hold.
        let (compiler, rebound) = std::thread::scope(|scope| {
            let compiler = std::thread::Builder::new()
                .name("sail-native-compiler".to_owned())
                .spawn_scoped(scope, || compiler_matches_in(&self.call.vars, plan))
                .ok();
            let rebound = (|| {
                if generated(&self.call).ok()? != self.generated || !externs_are_fresh(&self.call) {
                    return None;
                }
                let mut current = Call::new(
                    plan,
                    self.call.arguments,
                    false,
                    self.call
                        .vars
                        .iter()
                        .map(|(key, value)| (key.into(), value.into())),
                )?;
                (bind_compiler_options(&mut current)? == self.compiler_options
                    && current.key == self.call.key)
                    .then_some(())
            })();
            let compiler = match compiler {
                Some(thread) => thread.join().unwrap_or(false),
                None => compiler_matches_in(&self.call.vars, plan),
            };
            (compiler, rebound)
        });
        if !compiler {
            return None;
        }
        rebound?;
        std::fs::create_dir_all(&plan.native_records).ok()?;
        for output in &self.outputs {
            if !output.is_file() {
                continue;
            }
            let record = Record {
                runtime: plan.identity.clone(),
                policy: plan.closure.as_ref()?.policy_sha256().to_owned(),
                run_id: plan.run_id.clone(),
                package: self.call.package.clone(),
                crate_name: self.call.crate_name.clone(),
                compiler: plan.native_compiler_sha256.clone(),
                inputs: self.call.key.clone(),
                sha256: sha256_file(output)?.0,
                output: output.clone(),
                compiler_pid: self.reads.pid()?,
                source_observation: self.reads.path().to_owned(),
                source_observation_sha256: source_observation_sha256.clone(),
                native_graph_sha256: self.graph.key()?,
            };
            let path = record_path(plan, output);
            let staging = path.with_extension(format!("{}.tmp", std::process::id()));
            std::fs::write(&staging, serde_json::to_vec(&record).ok()?).ok()?;
            std::fs::rename(staging, path).ok()?;
            trace(
                self.call.arguments,
                &serde_json::json!({
                    "event": "native-output-established",
                    "record": record,
                    "environment": traced_environment(&self.call.vars),
                }),
            );
        }
        Some(())
    }
}

/// macOS otherwise records the absolute output path as `LC_ID_DYLIB`. Give
/// admitted proc macros their exact Cargo output basename as the install name
/// at compilation, before hashing any artifact. Portable recording and serving
/// also name freshly compiled registry sources by the existing virtual base:
/// rustc otherwise exports definition spans from builtin metadata with physical
/// registry filenames. No stored byte is rewritten.
fn bind_compiler_options(call: &mut Call<'_>) -> Option<Vec<String>> {
    let mut options = compiler_options(
        std::env::consts::OS,
        &super::argument_value(&call.keyed_args, "--crate-type")?,
        &call.stem,
    )?;
    options.extend(registry_source_options(
        &call.plan.mode,
        &call.plan.registry,
        &call.plan.virtual_base,
    )?);
    options.extend(portable_debug_map_options(
        std::env::consts::OS,
        &super::argument_value(&call.keyed_args, "--crate-type")?,
        &call.plan.mode,
    ));
    // Bind every added option, with the same physical-root substitution used
    // by the rest of this portable invocation key. The actual child receives
    // the literal options above. Machine-local options and keys are unchanged.
    let mut keyed_options: Vec<_> = options
        .iter()
        .map(|option| call.normalizer.normalize(option))
        .collect();
    if !matches!(call.plan.mode, super::Mode::MachineLocal { .. }) {
        let source = call.plan.compiler_source.as_ref()?;
        source.current(call.plan)?;
        let (actual, keyed) = compiler_source_options(source)?;
        options.extend(actual);
        // This substitution applies only to the option constructed above,
        // never to caller arguments, environment values or observed sources.
        keyed_options.extend(keyed);
    }
    call.key = compiler_option_key(&call.key, &keyed_options)?;
    Some(options)
}

fn compiler_source_options(source: &super::CompilerSource) -> Option<([String; 2], [String; 2])> {
    Some((
        [
            "--remap-path-prefix".to_owned(),
            format!("{}={}", source.root.to_str()?, source.virtual_root),
        ],
        [
            "--remap-path-prefix".to_owned(),
            format!("@@sail-compiler-source@@={}", source.virtual_root),
        ],
    ))
}

fn registry_source_options(
    mode: &super::Mode,
    registry: &Path,
    virtual_base: &str,
) -> Option<Vec<String>> {
    if matches!(mode, super::Mode::MachineLocal { .. }) {
        return Some(Vec::new());
    }
    let mut options = Vec::new();
    for directory in super::registry_forms(registry) {
        options.extend([
            "--remap-path-prefix".to_owned(),
            format!("{}={virtual_base}", directory.to_str()?),
        ]);
    }
    Some(options)
}

fn compiler_options(platform: &str, kind: &str, stem: &str) -> Option<Vec<String>> {
    if platform == "macos" && kind == "proc-macro" {
        if stem.is_empty()
            || !stem
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return None;
        }
        Some(vec![
            "-C".to_owned(),
            format!("link-arg=-Wl,-install_name,@rpath/lib{stem}.dylib"),
        ])
    } else {
        Some(Vec::new())
    }
}

// Native producer compilation only: ld otherwise embeds physical Rustup and
// temporary output paths as N_OSO debug-map entries. This compile-time option
// changes no consumer argument and is part of the existing literal option key.
fn portable_debug_map_options(platform: &str, kind: &str, mode: &super::Mode) -> Vec<String> {
    if platform == "macos"
        && kind == "proc-macro"
        && !matches!(mode, super::Mode::MachineLocal { .. })
    {
        vec!["-C".to_owned(), "link-arg=-Wl,-S".to_owned()]
    } else {
        Vec::new()
    }
}

fn compiler_option_key(base: &str, options: &[String]) -> Option<String> {
    if options.is_empty() {
        Some(base.to_owned())
    } else {
        Some(super::digest(&serde_json::to_vec(&(base, options)).ok()?))
    }
}

// Optional bounded evidence for refused lookups, never an admission check.
// In particular, recording a transient entry cannot authorize ignoring it.
fn trace_inventory(call: &Call<'_>, phase: &str) {
    if std::env::var_os("SAIL_SHIPPED_TRACE").is_none() {
        return;
    }
    let entries: Vec<_> = std::fs::read_dir(&call.out_dir)
        .into_iter()
        .flatten()
        .take(256)
        .map(|entry| match entry {
            Ok(entry) => serde_json::json!({
                "name": entry.file_name(),
                "regular": entry.file_type().ok().map(|kind|kind.is_file()),
                "directory": entry.file_type().ok().map(|kind|kind.is_dir()),
                "symlink": entry.file_type().ok().map(|kind|kind.is_symlink()),
                "size": entry.metadata().ok().map(|metadata|metadata.len()),
            }),
            Err(error) => serde_json::json!({"error":error.to_string()}),
        })
        .collect();
    trace(
        call.arguments,
        &serde_json::json!({
            "event":"native-snapshot-refusal-inventory", "phase":phase,
            "directory":call.out_dir,
            "canonical":call.out_dir.canonicalize().ok(),
            "entries":entries,"entry_limit":256,
        }),
    );
}

fn await_graph_records(call: &Call<'_>, graph: &Snapshot) -> bool {
    let paths = graph
        .original_inputs()
        .keys()
        .map(|path| producer_paths(&call.plan.native_records, path))
        .collect::<Vec<_>>();
    await_producers(
        &paths,
        &call.plan.run_id,
        std::time::Duration::from_secs(120),
    )
}

fn graph_established(call: &Call<'_>, graph: &Snapshot) -> bool {
    let Some(closure) = &call.plan.closure else {
        return false;
    };
    graph.original_inputs().iter().all(|(path, copied)| {
        std::fs::read(record_path(call.plan, path))
            .ok()
            .and_then(|raw| serde_json::from_slice::<Record>(&raw).ok())
            .is_some_and(|record| {
                established(
                    &record,
                    call.plan,
                    path,
                    &copied.sha256,
                    closure.policy_sha256(),
                )
            })
    })
}

fn emitted_outputs(call: &Call<'_>) -> Option<Vec<PathBuf>> {
    let text: Vec<_> = call
        .arguments
        .iter()
        .map(|arg| arg.to_str())
        .collect::<Option<_>>()?;
    let source = native_source(&text, &call.plan.target)?;
    let root = Path::new(call.vars.get("CARGO_MANIFEST_DIR")?);
    let manifest: toml::Value =
        toml::from_str(&std::fs::read_to_string(root.join("Cargo.toml")).ok()?).ok()?;
    let library = manifest.get("lib");
    let name = library
        .and_then(|lib| lib.get("name"))
        .and_then(toml::Value::as_str)
        .map_or_else(
            || {
                call.vars
                    .get("CARGO_PKG_NAME")
                    .map(|name| name.replace('-', "_"))
            },
            |name| Some(name.to_owned()),
        )?;
    let path = library
        .and_then(|lib| lib.get("path"))
        .and_then(toml::Value::as_str)
        .unwrap_or("src/lib.rs");
    let macro_crate = library
        .and_then(|lib| lib.get("proc-macro"))
        .and_then(toml::Value::as_bool)
        == Some(true);
    let kind = super::argument_value(&call.keyed_args, "--crate-type")?;
    if name != call.crate_name
        || Path::new(source) != root.join(path)
        || if macro_crate {
            kind != "proc-macro"
        } else {
            kind != "lib" && kind != "rlib"
        }
    {
        return None;
    }
    let emit = super::argument_value(&call.keyed_args, "--emit")?;
    let mut outputs = Vec::new();
    for emission in emit.split(',') {
        match emission {
            "dep-info" => {}
            "metadata" => outputs.push(call.out_dir.join(format!("lib{}.rmeta", call.stem))),
            "link" => outputs.push(call.out_dir.join(format!(
                "lib{}.{}",
                call.stem,
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

// Lint levels change diagnostics, not the compiler's source/lookup boundary.
// The full original flag/value spelling remains in Call's existing literal key.
fn lint_arguments(args: &[&str]) -> Option<usize> {
    const LONG: &[&str] = &["--allow", "--warn", "--deny", "--forbid", "--force-warn"];
    const SHORT: &[&str] = &["-A", "-W", "-D", "-F"];
    let argument = *args.first()?;
    let (value, count) = if LONG.contains(&argument) || SHORT.contains(&argument) {
        (*args.get(1)?, 2)
    } else if let Some((flag, value)) = argument.split_once('=')
        && LONG.contains(&flag)
    {
        (value, 1)
    } else if SHORT.iter().any(|flag| argument.starts_with(flag)) {
        (argument.get(2..)?, 1)
    } else {
        return None;
    };
    value
        .split(',')
        .all(|name| {
            name.split("::").all(|segment| {
                let mut bytes = segment.bytes();
                bytes
                    .next()
                    .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
                    && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            })
        })
        .then_some(count)
}

/// Only the ordinary native Cargo library flags are observed. Unsupported
/// backend/linker/sysroot/source overrides still compile, but establish no reuse.
fn native_source<'a>(args: &'a [&str], target: &Path) -> Option<&'a str> {
    let mut source = None;
    let mut index = 1;
    while let Some(argument) = args.get(index) {
        if let Some(consumed) = lint_arguments(&args[index..]) {
            index += consumed;
            continue;
        }
        match *argument {
            "--crate-name" | "--crate-type" | "--edition" | "--emit" | "--out-dir"
            | "--error-format" | "--json" | "--cap-lints" | "--cfg" | "--check-cfg"
            | "--extern" => {
                args.get(index + 1)?;
                index += 2;
            }
            "-C" => {
                if !safe_codegen(args.get(index + 1)?, target) {
                    return None;
                }
                index += 2;
            }
            "-Z" => {
                if *args.get(index + 1)? != "checksum-hash-algorithm=blake3" {
                    return None;
                }
                index += 2;
            }
            "-L" => {
                let directory = args.get(index + 1)?.strip_prefix("dependency=")?;
                if !within_target(Path::new(directory), target) {
                    return None;
                }
                index += 2;
            }
            flag if flag.starts_with("-C") => {
                if !safe_codegen(flag.strip_prefix("-C")?, target) {
                    return None;
                }
                index += 1;
            }
            flag if flag.starts_with("--") => {
                let (name, _) = flag.split_once('=')?;
                if ![
                    "--crate-name",
                    "--crate-type",
                    "--edition",
                    "--emit",
                    "--out-dir",
                    "--error-format",
                    "--json",
                    "--cap-lints",
                    "--cfg",
                    "--check-cfg",
                    "--extern",
                ]
                .contains(&name)
                {
                    return None;
                }
                index += 1;
            }
            path if !path.starts_with('-') => {
                if source.replace(path).is_some() {
                    return None;
                }
                index += 1;
            }
            _ => return None,
        }
    }
    source
}

fn within_target(path: &Path, target: &Path) -> bool {
    path.strip_prefix(target).is_ok_and(|relative| {
        relative
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
    })
}

fn safe_codegen(value: &str, target: &Path) -> bool {
    if value == "prefer-dynamic" {
        return true;
    }
    value.split_once('=').is_some_and(|(name, argument)| {
        if name == "incremental" {
            return within_target(Path::new(argument), target);
        }
        // Cargo's ordinary macOS debug profile supplies this for metadata
        // units. It selects a compiler enum, never an external input path;
        // the literal option remains bound in the native invocation key.
        if name == "split-debuginfo" {
            return matches!(argument, "off" | "packed" | "unpacked");
        }
        [
            "metadata",
            "extra-filename",
            "embed-bitcode",
            "debuginfo",
            "opt-level",
            "debug-assertions",
            "overflow-checks",
            "panic",
            "prefer-dynamic",
            "strip",
            "lto",
            "codegen-units",
            "incremental",
        ]
        .contains(&name)
    })
}

fn generated(call: &Call<'_>) -> Result<Option<BTreeMap<String, String>>, &'static str> {
    let Some(directory) = call.vars.get("OUT_DIR") else {
        return Ok(None);
    };
    let directory = Path::new(directory);
    if !directory.starts_with(&call.plan.target) {
        return Err("foreign generated-input directory");
    }
    super::package_tree(directory)
        .map(Some)
        .ok_or("unreadable or redirected generated inputs")
}

pub(super) fn compiler_matches(call: &Call<'_>) -> bool {
    compiler_matches_in(&call.vars, call.plan)
}

/// [`compiler_matches`] for an invocation's environment `vars` under `plan`.
fn compiler_matches_in(vars: &BTreeMap<String, String>, plan: &Plan) -> bool {
    if vars.get("__VERUS_DRIVER_VIA_CARGO__").map(String::as_str) != Some("1") {
        return false;
    }
    vars.get("SAIL_MANAGED_WRAPPER")
        .is_some_and(|wrapper| launcher_matches(plan, Path::new(wrapper)))
}

pub(super) fn launcher_matches(plan: &Plan, wrapper: &Path) -> bool {
    // Verus consumes Cargo's first rustc-path argument as a compatibility hint;
    // native units run rustc_driver in rust_verify, not that executable.
    wrapper.canonicalize().ok().as_ref() == Some(&plan.native_launcher)
        && sha256_file(wrapper).is_some_and(|(sha256, _)| sha256 == plan.native_launcher_sha256)
        && sha256_file(&plan.native_launcher.with_file_name("rust_verify"))
            .is_some_and(|(sha256, _)| sha256 == plan.native_compiler_sha256)
}

fn record_path(plan: &Plan, output: &Path) -> PathBuf {
    producer_paths(&plan.native_records, output).0
}

fn producer_paths(records: &Path, output: &Path) -> (PathBuf, PathBuf) {
    let key = super::digest(output.as_os_str().as_encoded_bytes());
    (
        records.join(format!("{key}.json")),
        records.join("running").join(key),
    )
}

/// Every explicit library must have been emitted by a successful owned native
/// unit in this run. Files with merely matching names/hashes do not establish it.
pub(super) fn externs_are_fresh(call: &Call<'_>) -> bool {
    if call.externs.iter().any(|(_, _, vir)| vir.is_some()) {
        return false;
    }
    let Some(records) = call
        .externs
        .iter()
        .map(|(argument, _, _)| {
            let (_, path) = external(call, argument)?;
            Some(producer_paths(&call.plan.native_records, &path))
        })
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let ready = await_producers(
        &records,
        &call.plan.run_id,
        std::time::Duration::from_secs(120),
    );
    // Hash actual libraries only once after the records arrive. A producer's
    // metadata-ready notification must not cause a busy loop hashing libraries.
    ready && externs_ready(call, true)
}

// Native successful observations can trail Cargo's metadata-ready message.
// Waiting is optional and bounded; missing/invalid provenance always misses.
fn await_producers(
    paths: &[(PathBuf, PathBuf)],
    run_id: &str,
    maximum: std::time::Duration,
) -> bool {
    let started = std::time::Instant::now();
    loop {
        let mut complete = true;
        for (record, mark) in paths {
            if record.is_file() {
                continue;
            }
            complete = false;
            if !std::fs::read(mark).is_ok_and(|bytes| bytes == run_id.as_bytes()) {
                return false;
            }
        }
        if complete {
            return true;
        }
        let Some(remaining) = maximum.checked_sub(started.elapsed()) else {
            return false;
        };
        std::thread::sleep(remaining.min(std::time::Duration::from_millis(10)));
    }
}

fn externs_ready(call: &Call<'_>, report: bool) -> bool {
    let Some(policy) = call
        .plan
        .closure
        .as_ref()
        .map(super::admission::Closure::policy_sha256)
    else {
        return false;
    };
    call.externs.iter().all(|(argument, sha256, vir)| {
        if vir.is_some() {
            return false;
        }
        let Some((name, path)) = external(call, argument) else {
            return false;
        };
        let path = path.as_path();
        let record = std::fs::read(record_path(call.plan, path))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Record>(&bytes).ok());
        let valid = record.as_ref().is_some_and(|record| {
            record.crate_name == name && established(record, call.plan, path, sha256, policy)
        });
        if !valid && report {
            trace(
                call.arguments,
                &serde_json::json!({
                    "event": "native-extern-refused",
                    "name": name,
                    "output": path,
                    "record_path": record_path(call.plan, path),
                    "expected_sha256": sha256,
                    "actual_sha256": sha256_file(path).map(|(sha, _)| sha),
                    "expected_run": call.plan.run_id,
                    "expected_runtime": call.plan.identity,
                    "expected_compiler": call.plan.native_compiler_sha256,
                    "expected_policy": policy,
                    "record": record,
                }),
            );
        }
        valid
    })
}

pub(super) fn external(call: &Call<'_>, argument: &str) -> Option<(String, PathBuf)> {
    let argument = call.normalizer.localize(argument);
    let argument = argument.strip_prefix("--extern=").unwrap_or(&argument);
    let (name, path) = argument.split_once('=')?;
    Some((name.to_owned(), PathBuf::from(path)))
}

fn established(record: &Record, plan: &Plan, path: &Path, sha256: &str, policy: &str) -> bool {
    record.runtime == plan.identity
        && record.run_id == plan.run_id
        && record.compiler == plan.native_compiler_sha256
        && record.policy == policy
        && record.output == path
        && record.native_graph_sha256.len() == 64
        && record.source_observation.starts_with(
            plan.target
                .join(".sail-boundary-native-reads")
                .join(&plan.run_id),
        )
        && native_reads::intact(
            &record.source_observation,
            &record.source_observation_sha256,
            record.compiler_pid,
            &plan.run_id,
            &record.inputs,
        )
        && record.sha256 == sha256
        && sha256_file(path).is_some_and(|(actual, _)| actual == sha256)
}

/// The invocation's tool-read variables, which the external admission
/// witness (`scripts/admission-witness.py`) replays: the proof-context
/// families, the driver's per-package roles and Sail's managed plumbing.
/// Recorded only in the opt-in trace; credential-like values are withheld.
fn traced_environment(vars: &BTreeMap<String, String>) -> BTreeMap<&str, &str> {
    vars.iter()
        .filter(|(name, _)| {
            super::super::environment::input_environment_key(std::ffi::OsStr::new(name))
                || name.starts_with(" __VERUS_")
                || name.starts_with("SAIL_MANAGED_")
                || name.as_str() == "CARGO_TARGET_DIR"
        })
        .map(|(name, value)| {
            let upper = name.to_ascii_uppercase();
            let secret = ["TOKEN", "SECRET", "PASSWORD", "CREDENTIAL"]
                .iter()
                .any(|word| upper.contains(word));
            (
                name.as_str(),
                if secret { "<withheld>" } else { value.as_str() },
            )
        })
        .collect()
}

fn trace(args: &[OsString], record: &serde_json::Value) {
    let Some(directory) = std::env::var_os(super::TRACE_ENV) else {
        return;
    };
    let directory = PathBuf::from(directory);
    if std::fs::create_dir_all(&directory).is_err() {
        return;
    }
    let identity = super::digest(format!("{args:?}").as_bytes());
    let record = serde_json::json!({"observation":record,"actual_args":args.iter().map(|arg|arg.to_string_lossy()).collect::<Vec<_>>()});
    if let Ok(bytes) = serde_json::to_vec_pretty(&record) {
        let event = super::digest(&bytes);
        let _ = std::fs::write(
            directory.join(format!(
                "native-{}-{identity}-{}.json",
                std::process::id(),
                &event[..12]
            )),
            bytes,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traced_environment_keeps_tool_reads_and_withholds_credentials() {
        let vars: BTreeMap<String, String> = [
            ("CARGO_PKG_NAME", "verus_builtin"),
            (" __VERUS_DRIVER_IS_BUILTIN_verus_builtin-1", "1"),
            ("SAIL_MANAGED_PROXY", "/proxy"),
            ("RUSTC_BOOTSTRAP", "1"),
            ("CARGO_REGISTRY_TOKEN", "secret-value"),
            ("HOME", "/home/someone"),
            ("AWS_SECRET_ACCESS_KEY", "secret-value"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
        let traced = traced_environment(&vars);
        assert_eq!(traced.get("CARGO_PKG_NAME"), Some(&"verus_builtin"));
        assert_eq!(
            traced.get(" __VERUS_DRIVER_IS_BUILTIN_verus_builtin-1"),
            Some(&"1")
        );
        assert_eq!(traced.get("SAIL_MANAGED_PROXY"), Some(&"/proxy"));
        assert_eq!(traced.get("RUSTC_BOOTSTRAP"), Some(&"1"));
        assert_eq!(traced.get("CARGO_REGISTRY_TOKEN"), Some(&"<withheld>"));
        assert!(!traced.contains_key("HOME"));
        assert!(!traced.contains_key("AWS_SECRET_ACCESS_KEY"));
    }

    #[test]
    fn compiler_source_options_keep_physical_paths_out_of_the_typed_key()
    -> Result<(), Box<dyn std::error::Error>> {
        let first = super::super::CompilerSource {
            root: "/owned-a/lib/rustlib/src/rust".into(),
            virtual_root: format!("/rustc/{}", "1".repeat(40)),
            tree_sha256: "same-source-bytes".into(),
        };
        let mut second = first.clone();
        second.root = "/owned-b/lib/rustlib/src/rust".into();
        let (actual_a, keyed_a) = compiler_source_options(&first).ok_or("A source options")?;
        let (actual_b, keyed_b) = compiler_source_options(&second).ok_or("B source options")?;
        assert_ne!(actual_a, actual_b);
        assert_eq!(keyed_a, keyed_b);
        assert!(actual_b[1].starts_with("/owned-b/lib/rustlib/src/rust=/rustc/"));
        assert_eq!(
            compiler_option_key("same-inputs", &keyed_a),
            compiler_option_key("same-inputs", &keyed_b)
        );
        second.virtual_root = format!("/rustc/{}", "2".repeat(40));
        assert_ne!(
            compiler_option_key("same-inputs", &keyed_a),
            compiler_option_key(
                "same-inputs",
                &compiler_source_options(&second)
                    .ok_or("other source options")?
                    .1
            )
        );
        Ok(())
    }

    #[test]
    fn portable_native_remap_is_mode_scoped_and_part_of_the_key()
    -> Result<(), Box<dyn std::error::Error>> {
        let first = Path::new("/sail-first-registry");
        let second = Path::new("/sail-second-registry");
        let options = |mode, root| {
            registry_source_options(mode, root, "/rustc-dev/pinned")
                .ok_or("UTF-8 portable source options")
        };
        let record = super::super::Mode::Record {
            out: "record".into(),
        };
        let serve = super::super::Mode::Serve {
            bundle: "serve".into(),
        };
        let local = super::super::Mode::MachineLocal {
            out: "local".into(),
        };
        let first_options = options(&record, first)?;
        assert_eq!(first_options, options(&serve, first)?);
        assert_eq!(
            first_options,
            [
                "--remap-path-prefix",
                "/sail-first-registry=/rustc-dev/pinned"
            ]
        );
        assert_eq!(options(&local, first)?, Vec::<String>::new());
        let keyed = |root: &Path, values: &[String]| {
            let normalizer = super::super::Normalizer {
                pairs: vec![(root.to_string_lossy().into_owned(), super::super::REGISTRY)],
            };
            values
                .iter()
                .map(|value| normalizer.normalize(value))
                .collect::<Vec<_>>()
        };
        let normalized = keyed(first, &first_options);
        assert_eq!(normalized, keyed(second, &options(&serve, second)?));
        let key = compiler_option_key("inputs", &normalized).ok_or("option key")?;
        assert_ne!(key, "inputs");
        assert_ne!(
            Some(key),
            compiler_option_key(
                "inputs",
                &keyed(
                    first,
                    &registry_source_options(&record, first, "/rustc-dev/other",)
                        .ok_or("other virtual base")?
                )
            )
        );
        Ok(())
    }

    #[test]
    fn native_debug_map_option_is_portable_mac_producer_only_and_keyed() {
        let record = super::super::Mode::Record {
            out: "record".into(),
        };
        let serve = super::super::Mode::Serve {
            bundle: "serve".into(),
        };
        let local = super::super::Mode::MachineLocal {
            out: "local".into(),
        };
        let options = portable_debug_map_options("macos", "proc-macro", &record);
        assert_eq!(options, ["-C", "link-arg=-Wl,-S"]);
        assert_eq!(
            options,
            portable_debug_map_options("macos", "proc-macro", &serve)
        );
        assert_ne!(
            compiler_option_key("original", &options).as_deref(),
            Some("original")
        );
        for (platform, kind, mode) in [
            ("linux", "proc-macro", &record),
            ("macos", "lib", &record),
            ("macos", "rlib", &serve),
            ("macos", "proc-macro", &local),
        ] {
            let unchanged = portable_debug_map_options(platform, kind, mode);
            assert_eq!(unchanged, Vec::<String>::new());
            assert_eq!(
                compiler_option_key("original", &unchanged).as_deref(),
                Some("original")
            );
        }
    }

    #[test]
    fn native_install_names_are_scoped_and_bound_to_literal_compiler_options()
    -> Result<(), Box<dyn std::error::Error>> {
        let options = compiler_options("macos", "proc-macro", "example-abcd")
            .ok_or("ordinary macro basename")?;
        assert_eq!(
            options,
            [
                "-C",
                "link-arg=-Wl,-install_name,@rpath/libexample-abcd.dylib"
            ]
        );
        for (platform, kind) in [("linux", "proc-macro"), ("macos", "lib"), ("macos", "rlib")] {
            let unchanged = compiler_options(platform, kind, "example-abcd")
                .ok_or("ordinary non-dylib invocation")?;
            assert_eq!(unchanged, Vec::<String>::new());
            assert_eq!(
                compiler_option_key("original", &unchanged).as_deref(),
                Some("original")
            );
        }
        for stem in [
            "",
            "../other",
            "example,other",
            "example.dylib",
            "example/other",
        ] {
            assert!(compiler_options("macos", "proc-macro", stem).is_none());
        }
        let key = compiler_option_key("original", &options).ok_or("bound options")?;
        assert_ne!(key, "original");
        assert_ne!(
            Some(key.clone()),
            compiler_option_key("other compiler inputs", &options)
        );
        let changed = compiler_options("macos", "proc-macro", "different-abcd")
            .ok_or("different macro basename")?;
        assert_ne!(Some(key), compiler_option_key("original", &changed));
        Ok(())
    }

    #[test]
    fn metadata_ready_can_precede_successful_producer_record() -> std::io::Result<()> {
        let root =
            std::env::temp_dir().join(format!("sail-native-pipeline-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let record = root.join("producer.json");
        let mark = root.join("running");
        let paths = vec![(record.clone(), mark.clone())];
        std::fs::write(&mark, b"this-run")?;
        let producer_record = record.clone();
        let producer = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(30));
            std::fs::write(producer_record, b"compiler exited successfully")
        });
        assert!(await_producers(
            &paths,
            "this-run",
            std::time::Duration::from_secs(2)
        ));
        assert!(producer.join().is_ok_and(|result| result.is_ok()));
        assert_eq!(std::fs::read(&record)?, b"compiler exited successfully");
        std::fs::remove_file(&record)?;
        std::fs::remove_file(&mark)?;
        let started = std::time::Instant::now();
        assert!(!await_producers(
            &paths,
            "this-run",
            std::time::Duration::from_secs(2)
        ));
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
        std::fs::write(&mark, b"older-run")?;
        assert!(!await_producers(
            &paths,
            "this-run",
            std::time::Duration::from_secs(2)
        ));
        std::fs::write(&mark, b"this-run")?;
        let producer_mark = mark.clone();
        let failed_producer = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(30));
            std::fs::remove_file(producer_mark)
        });
        assert!(!await_producers(
            &paths,
            "this-run",
            std::time::Duration::from_secs(2)
        ));
        assert!(failed_producer.join().is_ok_and(|result| result.is_ok()));
        std::fs::remove_dir_all(root)
    }

    #[test]
    fn native_shape_refuses_foreign_source_and_compiler_backends() {
        let ordinary = [
            "rustc",
            "--crate-name",
            "macros",
            "/registry/macros/src/lib.rs",
            "--crate-type",
            "proc-macro",
            "--emit=dep-info,link",
            "-C",
            "metadata=abc",
            "-C",
            "prefer-dynamic",
        ];
        assert_eq!(
            native_source(&ordinary, Path::new("/owned-target")),
            Some("/registry/macros/src/lib.rs")
        );
        for mode in ["off", "packed", "unpacked"] {
            let option = format!("split-debuginfo={mode}");
            let mut arguments = ordinary.to_vec();
            arguments.extend(["-C", &option]);
            assert_eq!(
                native_source(&arguments, Path::new("/owned-target")),
                Some("/registry/macros/src/lib.rs")
            );
        }
        for override_args in [
            vec!["--sysroot", "/foreign"],
            vec!["--sysroot", "/toolchain"],
            vec!["--sysroot=/toolchain"],
            vec!["--remap-path-prefix", "/registry=/foreign"],
            vec!["--remap-path-prefix=/registry=/foreign"],
            vec![
                "--remap-path-prefix",
                "/toolchain/lib/rustlib/src/rust=/rustc/58cf8f9c35b355199727627d07ae34a4e2846d67",
            ],
            vec![
                "--remap-path-prefix=/toolchain/lib/rustlib/src/rust=/rustc/58cf8f9c35b355199727627d07ae34a4e2846d67",
            ],
            vec!["--remap-path-scope", "object"],
            vec!["--remap-path-scope=all"],
            vec!["-Z", "codegen-backend=/foreign/backend.so"],
            vec!["-C", "linker=/foreign/cc"],
            vec!["-C", "link-arg=-Wl,-S"],
            vec!["-Clink-arg=-Wl,-S"],
            vec!["--emit=link=/foreign/output"],
            vec!["/foreign/second.rs"],
            vec!["-L", "dependency=/foreign"],
            vec!["-C", "incremental=/foreign"],
            vec!["-C", "split-debuginfo=/foreign"],
            vec!["-L", "dependency=/owned-target/../foreign"],
        ] {
            let mut arguments = ordinary.to_vec();
            arguments.extend(override_args);
            // --emit path syntax is rejected when deriving actual output files.
            if arguments.last() != Some(&"--emit=link=/foreign/output") {
                assert!(native_source(&arguments, Path::new("/owned-target")).is_none());
            }
        }
    }

    #[test]
    fn cargo_lint_options_preserve_native_source_and_refuse_path_overrides() {
        let source = "/registry/macros/src/lib.rs";
        let ordinary = [
            "rustc",
            source,
            "--crate-type",
            "lib",
            "--emit=metadata,link",
        ];
        // Joined spellings occur in the actual pinned Cargo traces for
        // verus_syn, hashbrown, indexmap, builtin and both macro libraries.
        for lint in [
            vec!["--warn=unexpected_cfgs"],
            vec!["--allow=clippy::zero_ptr"],
            vec!["--deny=unsafe_code"],
            vec!["--forbid=unsafe_code"],
            vec!["--force-warn=unused_variables"],
            vec!["--allow", "clippy::wrong_self_convention"],
            vec!["--warn", "rust_2018_idioms"],
            vec!["--deny", "unsafe_code"],
            vec!["--forbid", "unsafe_code"],
            vec!["--force-warn", "unused_variables"],
            vec!["-A", "clippy::zero_ptr"],
            vec!["-W", "unused_variables"],
            vec!["-D", "unsafe_code"],
            vec!["-F", "unsafe_code"],
            vec!["-Aclippy::zero_ptr"],
            vec!["-Wunsafe-op-in-unsafe-fn"],
            vec!["-Dunsafe_code"],
            vec!["-Funsafe_code"],
            vec!["--warn=dead_code,unused_imports"],
        ] {
            let mut args = ordinary.to_vec();
            args.extend(lint);
            assert_eq!(native_source(&args, Path::new("/target")), Some(source));
            for foreign in [
                vec!["/foreign/second.rs"],
                vec!["--sysroot", "/foreign"],
                vec!["--remap-path-prefix=/registry=/foreign"],
                vec!["-C", "linker=/foreign/cc"],
                vec!["-Z", "codegen-backend=/foreign"],
                vec!["--unknown-flag=unused"],
            ] {
                let mut changed = args.clone();
                changed.extend(foreign);
                assert!(native_source(&changed, Path::new("/target")).is_none());
            }
        }
        for malformed in [
            vec!["--warn"],
            vec!["--allow="],
            vec!["-W"],
            vec!["--warn", "--sysroot=/foreign"],
            vec!["--allow=/foreign/lib.rs"],
            vec!["--deny=clippy::::zero_ptr"],
            vec!["--warn=dead_code,"],
            vec!["--warn=unused variables"],
            vec!["-A=unused"],
        ] {
            let mut args = ordinary.to_vec();
            args.extend(malformed);
            assert!(native_source(&args, Path::new("/target")).is_none());
        }
    }

    #[test]
    fn native_provenance_binds_actual_file_run_and_compiler() -> std::io::Result<()> {
        let root =
            std::env::temp_dir().join(format!("sail-native-provenance-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let output = root.join("macro.so");
        std::fs::write(&output, b"native compiler output")?;
        let sha256 = super::super::digest(b"native compiler output");
        let mut plan = Plan {
            mode: super::super::Mode::MachineLocal { out: root.clone() },
            identity: "runtime".into(),
            registry: root.clone(),
            target: root.clone(),
            virtual_base: String::new(),
            solver_sha256: "solver".into(),
            solver_adapter: None,
            run_id: "current-run".into(),
            closure: None,
            native_records: root.clone(),
            native_compiler_sha256: "compiler".into(),
            native_launcher: root.join("verus"),
            native_launcher_sha256: "launcher".into(),
            compiler_source: None,
            release: None,
        };
        let read_dir = root.join(".sail-boundary-native-reads/current-run/fixture");
        std::fs::create_dir_all(&read_dir)?;
        let source_observation = read_dir.join("inputs.json");
        let observation = serde_json::to_vec(&serde_json::json!({
            "kind":"sail-native-compiler-read-inputs","schema":1,
            "binding":super::super::digest(b"current-run\0owned-inputs"),
            "pid":42,"complete":true,"inputs":{
                "sources":{"lib.rs":{"checksum":"blake3=fixture","size":1}},
                "extra_tracked_files":[],"tracked_environment":{}
            }
        }))?;
        std::fs::write(&source_observation, &observation)?;
        let mut record = Record {
            runtime: "runtime".into(),
            policy: "policy".into(),
            run_id: "current-run".into(),
            package: "macro-1".into(),
            crate_name: "macro".into(),
            compiler: "compiler".into(),
            inputs: "owned-inputs".into(),
            output: output.clone(),
            sha256: sha256.clone(),
            compiler_pid: 42,
            source_observation,
            source_observation_sha256: super::super::digest(&observation),
            native_graph_sha256: "a".repeat(64),
        };
        assert!(established(&record, &plan, &output, &sha256, "policy"));
        let mut legacy = serde_json::to_value(&record)?;
        legacy
            .as_object_mut()
            .ok_or_else(|| std::io::Error::other("record object"))?
            .remove("source_observation_sha256");
        assert!(serde_json::from_value::<Record>(legacy).is_err());
        std::fs::write(&record.source_observation, b"different compiler record")?;
        assert!(!established(&record, &plan, &output, &sha256, "policy"));
        std::fs::write(&record.source_observation, &observation)?;
        std::fs::write(&output, b"changed native output")?;
        assert!(!established(&record, &plan, &output, &sha256, "policy"));
        std::fs::write(&output, b"native compiler output")?;
        record.run_id = "previous-run".into();
        assert!(!established(&record, &plan, &output, &sha256, "policy"));
        record.run_id = "current-run".into();
        record.compiler = "foreign-compiler".into();
        assert!(!established(&record, &plan, &output, &sha256, "policy"));
        record.compiler = "compiler".into();
        assert!(!established(
            &record,
            &plan,
            &output,
            &sha256,
            "foreign-policy"
        ));
        let foreign = root.join("foreign.so");
        std::fs::write(&foreign, b"native compiler output")?;
        assert!(!established(&record, &plan, &foreign, &sha256, "policy"));
        let launcher = root.join("verus");
        let driver = root.join("rust_verify");
        std::fs::write(&launcher, b"launcher")?;
        std::fs::write(&driver, b"embedded rustc driver")?;
        plan.native_launcher = launcher.canonicalize()?;
        plan.native_launcher_sha256 = super::super::digest(b"launcher");
        plan.native_compiler_sha256 = super::super::digest(b"embedded rustc driver");
        assert!(launcher_matches(&plan, &launcher));
        let cargo_hint = root.join("unexecuted-rustc");
        std::fs::write(&cargo_hint, b"Cargo may select another rustc hint")?;
        assert!(
            launcher_matches(&plan, &launcher),
            "Cargo's discarded rustc hint is not the executing compiler"
        );
        std::fs::write(&driver, b"changed compiler driver")?;
        assert!(!launcher_matches(&plan, &launcher));
        std::fs::write(&driver, b"embedded rustc driver")?;
        std::fs::write(&launcher, b"changed launcher")?;
        assert!(!launcher_matches(&plan, &launcher));
        std::fs::remove_dir_all(root)
    }
}
