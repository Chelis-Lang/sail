//! Sail-owned neutral source worker; never an RPC-selected executable or path.
use std::io::{Read as _, Write};
use std::process::ExitCode;

use sail_api::contracts::{Evidence, Submission, VERSION, WorkerResult};

pub(crate) fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> ExitCode {
    if std::env::var("SAIL_JOB_GUARD_PARENT").is_ok_and(|v| v == "1") {
        crate::portfolio::guard_parent();
    }
    match execute(args, out, err) {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            if message.starts_with("error[") {
                let _ = writeln!(err, "{message}");
            } else {
                let _ = writeln!(err, "error[SAIL3142]: neutral worker: {message}");
            }
            ExitCode::from(2)
        }
    }
}

fn execute(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> Result<u8, String> {
    let [path] = args else {
        return Err("expected one Sail-owned request file".into());
    };
    let metadata = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.file_type().is_file() {
        return Err("request is not an owned regular file".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take((sail_api::contracts::MAX_SOURCE_BYTES * 8 + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > sail_api::contracts::MAX_SOURCE_BYTES * 8 {
        return Err("request exceeds bound".into());
    }
    let submission: Submission = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    submission.validate()?;
    let mut config = crate::managed_cli::config(Vec::new(), true)?;
    let pins = sail_verus::toolchain::Pins::embedded().map_err(|e| e.to_string())?;
    config.cargo_toolchain = sail_verus::managed::CargoToolchain::Pinned(pins.verus.rust_toolchain);
    config.force = true;
    let run = sail_verus::contracts::verify_profile(
        &submission.source,
        &submission.crate_name,
        submission.profile,
        &config,
        err,
    )
    .map_err(|e| e.to_string())?;
    let code = u8::try_from(run.backend.exit_code.unwrap_or(2)).unwrap_or(2);
    let result = WorkerResult {
        version: VERSION,
        backend: run.backend,
        contracts: Evidence {
            version: VERSION,
            manifest: run.lowering.manifest,
            proof: run.proof,
            translation_trust: run.translation_trust,
        },
    };
    serde_json::to_writer(out, &result).map_err(|e| e.to_string())?;
    Ok(code)
}
