//! Explicit native acceptance of the actual managed neutral helper.
//! Build Sail and sail-z3 in target/release first. Invoke this binary directly,
//! outside Cargo's injected loader environment, with a fresh evidence directory.

use std::path::PathBuf;

use sail_api::project::Currentness;
use sail_verus::contracts::{ContractRun, verify};
use sail_verus::managed::{CargoToolchain, Config};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn main() -> TestResult {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: neutral_contracts_acceptance FRESH_EVIDENCE_DIRECTORY")?;
    std::fs::create_dir(&output)?;
    let pins = sail_verus::toolchain::Pins::embedded()?;
    let installed =
        sail_verus::engine::Installed::locate_verified(&sail_verus::cache_dir()?, &pins)?;
    let stock = sail_verus::toolchain::Installed::locate_release(pins.clone())?;
    let workspace = std::env::current_dir()?;
    let config = Config {
        workspace: output.clone(),
        cache: output.join("cache"),
        sail: workspace.join("target/release/sail"),
        cargo_verus: installed.cargo_verus,
        cargo: cargo()?,
        cargo_toolchain: CargoToolchain::Pinned(pins.verus.rust_toolchain),
        shim: workspace.join("target/release/sail-z3"),
        z3: stock.z3,
        socket: None,
        args: Vec::new(),
        force: true,
        attribution_logs: true,
    };
    let original = include_str!("../tests/fixtures/neutral-contracts/add_one.rs");
    let changed = original.replace("    input + 1\n", "    input\n");
    if changed == original {
        return Err("body edit did not change the fixture".into());
    }
    for (label, source, expected) in [
        ("positive", original, true),
        ("body-edit", changed.as_str(), false),
    ] {
        let source_path = output.join(format!("{label}.original.rs"));
        std::fs::write(&source_path, source)?;
        let mut diagnostics = Vec::new();
        let result = verify(source, "neutral", &config, &mut diagnostics);
        std::fs::write(output.join(format!("{label}.stderr")), diagnostics)?;
        if std::fs::read(&source_path)? != source.as_bytes() {
            return Err("caller source changed during managed neutral verification".into());
        }
        let run = result?;
        std::fs::write(
            output.join(format!("{label}.run.json")),
            serde_json::to_vec_pretty(&run)?,
        )?;
        validate(&run, expected)?;
        let lock = std::fs::read(run.shadow.join("Cargo.lock"))?;
        if sha(&lock) != run.translation_trust.cargo_lock_sha256 {
            return Err("post-run lockfile does not match translation trust evidence".into());
        }
        let shadow = std::fs::read(run.shadow.join("src/lib.rs"))?;
        if sha(&shadow) != run.lowering.manifest.lowered_sha256
            || sha(source.as_bytes()) != run.lowering.manifest.original_sha256
        {
            return Err("post-run original or shadow digest is wrong".into());
        }
        println!(
            "{label}: actual expected={expected}, original unchanged, shadow/lock hashes current"
        );
    }
    Ok(())
}

fn validate(run: &ContractRun, expected: bool) -> TestResult {
    if run.backend.error.is_some()
        || run.backend.currentness != Currentness::Current
        || run.backend.no_reuse.is_some()
    {
        return Err("actual helper did not establish current managed backend evidence".into());
    }
    let targets = run
        .backend
        .targets
        .iter()
        .filter(|target| target.requested)
        .collect::<Vec<_>>();
    if targets.is_empty() {
        return Err("no actual requested target was verified".into());
    }
    let mut completed = 0;
    for target in targets {
        if target.complete != expected
            || target.report.error.is_some()
            || target.report.crates.is_empty()
        {
            return Err("requested target completion disagrees with expected verification".into());
        }
        for krate in &target.report.crates {
            if krate.verus.encountered_vir_error != Some(false)
                || krate.verus.success != Some(expected)
            {
                return Err(
                    "compiler failure or unexpected crate verdict is not a contract result".into(),
                );
            }
            completed += if expected {
                krate.verus.verified
            } else {
                krate.verus.errors
            }
            .unwrap_or_default();
        }
    }
    if completed == 0 {
        return Err("empty verification is not contract evidence".into());
    }
    for digest in [
        &run.translation_trust.source_policy_sha256,
        &run.translation_trust.metadata_sha256,
        &run.translation_trust.cargo_lock_sha256,
    ] {
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("translation trust evidence has no exact digest".into());
        }
    }
    Ok(())
}

fn sha(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

fn cargo() -> TestResult<PathBuf> {
    let path = std::env::var_os("PATH").ok_or("native acceptance has no PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join("cargo"))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| "native acceptance cannot locate the actual Cargo executable".into())
}
