//! External, explicitly invoked actual compiler admission witness.
//!
//! This module is diagnostic-only. It uses the production plan resolver,
//! source policy, argument binding, private graph and compiler-read consumer.

use super::super::{self as shipped, sha256_file};
use super::Observation;
use serde_json::json;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

type Failure = Box<dyn std::error::Error>;

fn required(name: &str) -> Result<PathBuf, Failure> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .ok_or_else(|| format!("missing external witness input {name}").into())
}

fn save_output(base: &Path, label: &str, output: &Output) -> Result<(), Failure> {
    std::fs::write(base.join(format!("{label}.stdout")), &output.stdout)?;
    std::fs::write(base.join(format!("{label}.stderr")), &output.stderr)?;
    std::fs::write(
        base.join(format!("{label}.status.json")),
        serde_json::to_vec_pretty(&json!({"code":output.status.code()}))?,
    )?;
    Ok(())
}

/// One child owns its environment and compiler group; the external bounded
/// runner captures every byte and kills/reaps that group on deadline.
#[allow(
    clippy::too_many_lines,
    reason = "Retain the exact outer/native layer split, real compiler comparisons and certificate check in one externally invoked witness."
)]
#[test]
#[ignore = "requires explicitly frozen genuine runtime, all official sources and bounded external runner"]
fn actual_native_admission_child() -> Result<(), Failure> {
    let config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(required("SAIL_ACTUAL_ADMISSION_CONFIG")?)?)?;
    let path = |name: &str| -> Result<PathBuf, Failure> {
        config[name]
            .as_str()
            .map(PathBuf::from)
            .ok_or_else(|| format!("missing config path {name}").into())
    };
    let root = path("root")?;
    let registry = path("registry")?;
    let target = path("target")?;
    let wrapper = path("wrapper")?;
    let metadata = serde_json::from_slice(&std::fs::read(path("metadata")?)?)?;
    let kind = config["case"].as_str().ok_or("missing case")?;
    let expected_source_admission = matches!(
        kind,
        "official" | "official-default" | "official-custom-cfg"
    );
    let expected_observation = kind == "official-default";
    // Production resolves the verifier/sysroot in the outer caller BEFORE
    // native-mode environment is installed on a compiler proxy. Reproduce
    // that layer boundary with one owned child; do not mutate process-global
    // environment or fabricate any Plan/Closure field.
    let plan: shipped::Plan =
        if let Some(plan_path) = std::env::var_os("SAIL_ACTUAL_ADMISSION_PLAN") {
            serde_json::from_slice(&std::fs::read(plan_path)?)?
        } else {
            if std::env::var_os("__VERUS_DRIVER_VIA_CARGO__").is_some() {
                return Err("outer plan fixture must not start in native driver mode".into());
            }
            let plan = shipped::plan(
                &wrapper.with_file_name("cargo-verus"),
                &target,
                Some(&registry),
                true,
                &path("solver")?,
                &path("shim")?,
                "actual-admission",
                &root,
                &[],
                &metadata,
            )
            .ok_or("real plan resolver refused the unchanged complete closure")?;
            let plan_path = root.join("actual-outer-plan.json");
            std::fs::write(&plan_path, serde_json::to_vec_pretty(&plan)?)?;
            let status = Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "managed::shipped::native::admission_actual_tests::actual_native_admission_child",
                "--ignored",
                "--nocapture",
            ])
            .env("SAIL_ACTUAL_ADMISSION_PLAN", plan_path)
            .env("__VERUS_DRIVER_VIA_CARGO__", "1")
            .status()?;
            return if status.success() {
                Ok(())
            } else {
                Err("owned native-mode admission child failed; retain its full output".into())
            };
        };
    let closure = plan.closure.as_ref().ok_or("real closure absent")?;
    if !closure.check_sources() {
        return Err("unchanged full official source closure refused".into());
    }
    // The source mutation happens only AFTER a genuine full plan was admitted;
    // native_unit must freshly reread and reject it before option binding.
    if kind == "modified" {
        let source = path("source")?;
        let mut bytes = std::fs::read(&source)?;
        bytes.extend_from_slice(b"\ncompile_error!(\"owned modified-source negative\");\n");
        std::fs::write(source, bytes)?;
        if closure.check_sources() {
            return Err("modified official source still admitted".into());
        }
        if shipped::admission::Closure::resolve(&metadata, &registry).is_some() {
            return Err("modified official source admitted by fresh closure resolution".into());
        }
    }
    let args: Vec<OsString> = config["args"]
        .as_array()
        .ok_or("missing args")?
        .iter()
        .map(|value| value.as_str().map(OsString::from).ok_or("non-string arg"))
        .collect::<Result<_, _>>()?;
    let vars = std::env::vars().collect();
    let unit_admitted = closure.native_unit(&vars, &args);
    if unit_admitted != expected_source_admission {
        return Err("actual native_unit admission differed from expected case".into());
    }
    let output_paths: Vec<PathBuf> = config["outputs"]
        .as_array()
        .ok_or("missing outputs")?
        .iter()
        .map(|value| value.as_str().map(PathBuf::from).ok_or("non-string output"))
        .collect::<Result<_, _>>()?;
    std::fs::copy(path("source")?, root.join("actual-compiler-source.rs"))?;
    let original = Command::new(&wrapper)
        .args(&args)
        .stdin(Stdio::null())
        .output()?;
    save_output(&root, "original", &original)?;
    if kind == "official"
        && (!original.status.success()
            || !original
                .stderr
                .windows(b"missing documentation".len())
                .any(|bytes| bytes == b"missing documentation"))
    {
        return Err("official original arm did not emit the actual missing_docs warning".into());
    }
    if matches!(kind, "foreign" | "modified" | "official-custom-cfg") && original.status.success() {
        return Err("negative source fixture did not produce actual compiler failure".into());
    }
    for (index, output) in output_paths.iter().enumerate() {
        if output.try_exists()? {
            std::fs::copy(output, root.join(format!("original-artifact-{index}")))?;
            std::fs::remove_file(output)?;
        }
    }
    let mut observation = Observation::begin(&plan, &args);
    if observation.is_some() != expected_observation {
        return Err("actual Observation admission differed from expected case".into());
    }
    let actual_args = observation
        .as_ref()
        .map_or_else(|| args.clone(), |observed| observed.arguments().to_vec());
    std::fs::write(
        root.join("admission.json"),
        serde_json::to_vec_pretty(&json!({
            "case":kind,"full_plan":true,"native_unit":unit_admitted,
            "observation":observation.is_some(),"original_args":args,
            "actual_args":actual_args,"identity":plan.identity,
            "closure":closure,"compiler_source":plan.compiler_source,
        }))?,
    )?;
    if !expected_observation && actual_args != args {
        return Err("refused invocation changed original arguments".into());
    }
    if !expected_observation
        && (plan.native_records.exists() || target.join(".sail-boundary-native-reads").exists())
    {
        return Err("refused invocation created native read/provenance state".into());
    }
    let stdout = std::fs::File::create(root.join("candidate.stdout"))?;
    let stderr = std::fs::File::create(root.join("candidate.stderr"))?;
    let mut command = Command::new(&wrapper);
    command
        .args(&actual_args)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr);
    if let Some(observed) = &observation {
        observed.configure(&mut command);
    }
    let mut child = command.spawn()?;
    if let Some(observed) = &mut observation {
        observed.started(child.id());
    }
    let status = child.wait()?;
    let established = if let Some(observed) = &observation {
        status.success() && observed.try_complete().is_some()
    } else {
        false
    };
    let stdout_equal = original.stdout == std::fs::read(root.join("candidate.stdout"))?;
    let stderr_equal = original.stderr == std::fs::read(root.join("candidate.stderr"))?;
    let code_equal = original.status.code() == status.code();
    let artifacts: Vec<_> = output_paths
        .iter()
        .map(|output| (output, sha256_file(output)))
        .collect();
    std::fs::write(
        root.join("comparison.json"),
        serde_json::to_vec_pretty(&json!({
            "case":kind,"original_code":original.status.code(),
            "candidate_code":status.code(),"code_equal":code_equal,
            "stdout_equal":stdout_equal,"stderr_equal":stderr_equal,
            "successful_source_provenance":established,"artifacts":artifacts,
            "original_args_fallback":!expected_observation && actual_args == args,
            "passed":code_equal && stdout_equal && stderr_equal &&
                (!expected_observation || (status.success() && established)),
        }))?,
    )?;
    // Keep diagnostics mismatches as actual failing tests, after saving every
    // stream, admission result and compiler-read record for independent review.
    if !code_equal || !stdout_equal || !stderr_equal {
        return Err("literal original/candidate native compiler output differs".into());
    }
    if expected_observation && (!status.success() || !established) {
        return Err("official compiler success did not establish actual provenance".into());
    }
    Ok(())
}
