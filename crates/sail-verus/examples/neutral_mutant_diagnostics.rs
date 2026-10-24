//! Bounded exact-source fork/stock diagnostic, retaining literal solver transcripts.
//! This runner observes failure and unknown; it never manufactures a mutation kill.
use sail_api::mutation::digest;
use sail_verus::contracts::lower;
use std::path::{Path, PathBuf};
use std::process::Command;
type TestResult = Result<(), Box<dyn std::error::Error>>;
fn main() -> TestResult {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: neutral_mutant_diagnostics FRESH_DIRECTORY")?;
    std::fs::create_dir(&output)?;
    let output = std::fs::canonicalize(output)?;
    let pins = sail_verus::toolchain::Pins::embedded()?;
    let fork = match std::env::var_os("SAIL_VERUS_BUILD")
        .or_else(|| std::env::var_os("SAIL_ENGINE_BUILD"))
    {
        Some(path) => sail_verus::engine::Installed::at(Path::new(&path), &pins)?,
        None => sail_verus::engine::Installed::locate_verified(&sail_verus::cache_dir()?, &pins)?,
    };
    let stock = sail_verus::toolchain::Installed::locate_release(pins)?;
    let cases = cases();
    let mut summary = Vec::new();
    for (label, original, expected) in cases {
        let directory = output.join(label);
        std::fs::create_dir(&directory)?;
        let lowered = lower(&original, "neutral")?;
        let source = directory.join("shadow.rs");
        std::fs::write(&source, &lowered.source)?;
        std::fs::write(directory.join("original.rs"), &original)?;
        std::fs::write(
            directory.join("manifest.json"),
            serde_json::to_vec_pretty(&lowered.manifest)?,
        )?;
        for (name, executable) in [("fork", &fork.verus), ("stock", &stock.verus)] {
            let logs = directory.join(name);
            std::fs::create_dir(&logs)?;
            let mut command = Command::new(executable);
            command
                .env("VERUS_Z3_PATH", &stock.z3)
                .arg(&source)
                .args([
                    "--crate-type",
                    "lib",
                    "--crate-name",
                    "neutral",
                    "--output-json",
                    "--time",
                    "--no-report-long-running",
                    "--num-threads",
                    "1",
                    "--log",
                    "smt",
                    "--log",
                    "smt-transcript",
                    "--log",
                    "air",
                    "--log-dir",
                ])
                .arg(&logs);
            let result = command.output()?;
            std::fs::write(logs.join("stdout"), &result.stdout)?;
            std::fs::write(logs.join("stderr"), &result.stderr)?;
            let doc = serde_json::Deserializer::from_slice(&result.stdout)
                .into_iter::<serde_json::Value>()
                .next()
                .ok_or("missing actual verifier JSON")??;
            let totals = doc
                .get("verification-results")
                .ok_or("missing verification totals")?;
            if totals
                .get("encountered-vir-error")
                .and_then(serde_json::Value::as_bool)
                != Some(false)
                || result.status.success() != expected
            {
                return Err(format!(
                    "unexpected actual diagnostic verdict or frontend failure: {label}/{name}"
                )
                .into());
            }
            summary.push(serde_json::json!({"case":label,"backend":name,"expected_success":expected,"exit_code":result.status.code(),"verifier_sha256":digest(&std::fs::read(executable)?),"solver_sha256":digest(&std::fs::read(&stock.z3)?),"source_sha256":lowered.manifest.original_sha256,"shadow_sha256":lowered.manifest.lowered_sha256,"totals":totals}));
        }
    }
    std::fs::write(
        output.join("summary.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"scope":"bounded direct pinned fork/stock observations; no source-level rejection or mutation kill inferred","cases":summary}),
        )?,
    )?;
    Ok(())
}

type Case = (&'static str, String, bool);
fn cases() -> [Case; 6] {
    let bool_source = "#[sail_contracts::satisfies(\"FLAG\")] #[sail_contracts::ensures(|ret| ret == value)] pub fn candidate(value:bool)->bool { if value {true} else {false} }";
    let scalar_source = "#[sail_contracts::satisfies(\"ORD-SCALAR-001\")] #[sail_contracts::ensures(|ret| (!(amount < 255)) || ret == amount + 1)] pub fn increment_amount(amount:u8)->u8 { if amount < 255 { amount + 1 } else { amount } }";
    [
        ("bool-baseline", bool_source.to_owned(), true),
        (
            "bool-false",
            bool_source.replace("if value {true} else {false}", "false"),
            false,
        ),
        (
            "bool-true",
            bool_source.replace("if value {true} else {false}", "true"),
            false,
        ),
        ("scalar-baseline", scalar_source.to_owned(), true),
        (
            "scalar-zero",
            scalar_source.replace("if amount < 255 { amount + 1 } else { amount }", "0"),
            false,
        ),
        (
            "scalar-one",
            scalar_source.replace("if amount < 255 { amount + 1 } else { amount }", "1"),
            false,
        ),
    ]
}
