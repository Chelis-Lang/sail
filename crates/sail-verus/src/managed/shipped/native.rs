//! Bind boundary inputs to actual successful native compiler invocations.
//!
//! Observation never replaces, buffers or changes the native compiler's output.
//! Missing provenance only disables the experimental vstd lookup.

use super::{Call, Plan, sha256_file};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

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
}

/// Optional observation around one actual native compiler subprocess.
pub(crate) struct Observation<'a> {
    call: Call<'a>,
    generated: Option<BTreeMap<String, String>>,
    outputs: Vec<PathBuf>,
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
        let call =
            Call::new(plan, args, false, std::env::vars_os()).ok_or("unkeyed native unit")?;
        if !call.out_dir.starts_with(&plan.target) {
            return Err("foreign native output directory");
        }
        let closure = plan.closure.as_ref().ok_or("no admitted source closure")?;
        if !closure.native_unit(&call.vars, args) {
            return Err("foreign native source, version or compiler feature");
        }
        if !compiler_matches(&call) {
            return Err("native compiler is not the admitted sysroot compiler");
        }
        // Cargo pipelines dependents after metadata emission, before the
        // producer exits. Native units always compile; require successful
        // dependency provenance when completing their observation instead.
        let generated = generated(&call)?;
        let outputs =
            emitted_outputs(&call).ok_or("unsupported native Cargo library invocation")?;
        if outputs.iter().any(|path| path.try_exists().unwrap_or(true)) {
            return Err("native output already exists before this compiler invocation");
        }
        Ok(Self {
            call,
            generated,
            outputs,
        })
    }

    /// Called only after the real compiler returns successfully.
    pub(crate) fn complete(self) {
        let _ = self.try_complete();
    }

    fn try_complete(&self) -> Option<()> {
        let plan = self.call.plan;
        if !plan
            .closure
            .as_ref()?
            .native_unit(&self.call.vars, self.call.arguments)
            || !compiler_matches(&self.call)
            || generated(&self.call).ok()? != self.generated
            || !externs_are_fresh(&self.call)
        {
            return None;
        }
        let current = Call::new(
            plan,
            self.call.arguments,
            false,
            self.call
                .vars
                .iter()
                .map(|(key, value)| (key.into(), value.into())),
        )?;
        if current.key != self.call.key {
            return None;
        }
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
            };
            let path = record_path(plan, output);
            let staging = path.with_extension(format!("{}.tmp", std::process::id()));
            std::fs::write(&staging, serde_json::to_vec(&record).ok()?).ok()?;
            std::fs::rename(staging, path).ok()?;
            trace(
                self.call.arguments,
                &serde_json::json!({"event":"native-output-established","record":record}),
            );
        }
        Some(())
    }
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

/// Only the ordinary native Cargo library flags are observed. Unsupported
/// backend/linker/sysroot/source overrides still compile, but establish no reuse.
fn native_source<'a>(args: &'a [&str], target: &Path) -> Option<&'a str> {
    let mut source = None;
    let mut index = 1;
    while let Some(argument) = args.get(index) {
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
    value.split_once('=').is_some_and(|(name, argument)| {
        if name == "incremental" {
            return within_target(Path::new(argument), target);
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
    if call
        .vars
        .get("__VERUS_DRIVER_VIA_CARGO__")
        .map(String::as_str)
        != Some("1")
    {
        return false;
    }
    call.vars
        .get("SAIL_MANAGED_WRAPPER")
        .is_some_and(|wrapper| launcher_matches(call.plan, Path::new(wrapper)))
}

fn launcher_matches(plan: &Plan, wrapper: &Path) -> bool {
    // Verus consumes Cargo's first rustc-path argument as a compatibility hint;
    // native units run rustc_driver in rust_verify, not that executable.
    wrapper.canonicalize().ok().as_ref() == Some(&plan.native_launcher)
        && sha256_file(wrapper).is_some_and(|(sha256, _)| sha256 == plan.native_launcher_sha256)
        && sha256_file(&plan.native_launcher.with_file_name("rust_verify"))
            .is_some_and(|(sha256, _)| sha256 == plan.native_compiler_sha256)
}

fn record_path(plan: &Plan, output: &Path) -> PathBuf {
    plan.native_records.join(format!(
        "{}.json",
        super::digest(output.as_os_str().as_encoded_bytes())
    ))
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
            Some(record_path(call.plan, &path))
        })
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let ready = await_records(
        || records.iter().all(|path| path.is_file()),
        std::time::Duration::from_secs(2),
    );
    // Hash actual libraries only once after the records arrive. A producer's
    // metadata-ready notification must not cause a busy loop hashing libraries.
    ready && externs_ready(call, true)
}

// Native successful observations can trail Cargo's metadata-ready message.
// Waiting is optional and bounded; missing/invalid provenance always misses.
fn await_records(mut ready: impl FnMut() -> bool, maximum: std::time::Duration) -> bool {
    let started = std::time::Instant::now();
    loop {
        if ready() {
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

fn external(call: &Call<'_>, argument: &str) -> Option<(String, PathBuf)> {
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
        && record.sha256 == sha256
        && sha256_file(path).is_some_and(|(actual, _)| actual == sha256)
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
    fn metadata_ready_can_precede_successful_producer_record() -> std::io::Result<()> {
        let root =
            std::env::temp_dir().join(format!("sail-native-pipeline-{}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let record = root.join("producer.json");
        let producer_record = record.clone();
        let producer = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(30));
            std::fs::write(producer_record, b"compiler exited successfully")
        });
        assert!(await_records(
            || std::fs::read(&record).is_ok_and(|bytes| bytes == b"compiler exited successfully"),
            std::time::Duration::from_secs(2),
        ));
        assert!(producer.join().is_ok_and(|result| result.is_ok()));
        std::fs::remove_file(&record)?;
        assert!(!await_records(
            || record.exists(),
            std::time::Duration::ZERO,
        ));
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
        ];
        assert_eq!(
            native_source(&ordinary, Path::new("/owned-target")),
            Some("/registry/macros/src/lib.rs")
        );
        for override_args in [
            vec!["--sysroot", "/foreign"],
            vec!["-Z", "codegen-backend=/foreign/backend.so"],
            vec!["-C", "linker=/foreign/cc"],
            vec!["--emit=link=/foreign/output"],
            vec!["/foreign/second.rs"],
            vec!["-L", "dependency=/foreign"],
            vec!["-C", "incremental=/foreign"],
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
            run_id: "current-run".into(),
            closure: None,
            native_records: root.clone(),
            native_compiler_sha256: "compiler".into(),
            native_launcher: root.join("verus"),
            native_launcher_sha256: "launcher".into(),
        };
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
        };
        assert!(established(&record, &plan, &output, &sha256, "policy"));
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
