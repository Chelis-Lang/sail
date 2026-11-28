//! The fault arm (ADR 0183): faults at the CBMC boundary and in resources
//! must never yield a Sail success or a cached result.
//!
//! A stand-in `cbmc` (after S6's) wraps the real binary and injects one
//! fault: results cut short then exit 6 (Kani #4905), complete results then
//! `SIGKILL`, `SIGKILL` before any result, or a hang. The direct arm runs a
//! copy of the stage with the stand-in installed, which shows what direct
//! Kani prints. On the Sail side the stand-in goes into the stage of a
//! private copy of Sail's original tools, which Sail staged itself in the
//! control row (no fault, real tools, Sail must match direct). Real resource
//! faults run Sail's real tools: a cgroup memory cap that only the harness's
//! CBMC run exceeds, and Sail's own deadline.
//!
//! Each direct fault capture is also given to the product decoder alone
//! (`sail_kani::decode`), to show which faults it would admit without
//! tool authentication. That column is informational: it is what integrity
//! supervision at the CBMC boundary has to close.
use std::collections::BTreeSet;
use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde::Serialize;

use super::Error;
use super::record::{Claim, KaniRunRecord, NoClaim};
use super::runner::{Cli, Direct, Harness, SailSide, Slot, copy_tree, sail_claim, spawn};

/// The stand-in `cbmc`. The fault comes from `cbmc.fault` beside it, since
/// the driver runs with a cleared environment.
pub const STAND_IN: &str = r#"#!/bin/bash
# Stand-in for the bundle's cbmc (Sail Kani fault arm, after spike S6).
here=$(dirname "$0")
real="$here/cbmc.real"
mode=$(cat "$here/cbmc.fault" 2>/dev/null)
goto=
for a in "$@"; do case "$a" in *.out) goto=$a ;; esac; done
[ -z "$goto" ] && exec "$real" "$@"
case "$mode" in
  truncate-exit6) "$real" "$@" | python3 -I "$here/cbmc-truncate.py"; exit 6 ;;
  results-sigkill) "$real" "$@"; kill -KILL $$ ;;
  sigkill-before-results) kill -KILL $$ ;;
  hang) exec sleep 100000 ;;
  *) exec "$real" "$@" ;;
esac
"#;

/// Drops CBMC's last result (the harness's own assertion comes last), then
/// appends CBMC's own out-of-memory message, in CBMC's JSON-UI layout (S6's
/// injector for Kani #4905).
pub const TRUNCATE: &str = r#"import json, sys
doc = json.load(sys.stdin)
out = []
for el in doc:
    if isinstance(el, dict) and "result" in el:
        res = el["result"]
        out.append({"result": res[:-1]})
        out.append({"messageText": "Out of memory", "messageType": "ERROR"})
        break
    out.append(el)
def block(el):
    return "\n".join("  " + l for l in json.dumps(el, indent=2).split("\n"))
sys.stdout.write("[\n" + ",\n".join(block(el) for el in out) + "\n]\n")
"#;

/// One fault case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fault {
    /// No fault, real tools: the control row. Sail's claim must equal direct's.
    None,
    /// The last result cut, then exit 6 (S6 way 2).
    TruncateExit6,
    /// Complete results, then `SIGKILL` (S6 way 3).
    ResultsSigkill,
    /// `SIGKILL` before any result (what the OOM killer does).
    SigkillBeforeResults,
    /// `cbmc` never finishes; a deadline must fire.
    Hang,
    /// Real tools under a cgroup memory cap that the harness exceeds.
    MemoryCap,
    /// Real tools; Sail's own `--timeout-ms` fires first.
    SailDeadline,
}

impl Fault {
    fn mode(self) -> Option<&'static str> {
        match self {
            Self::None => Some("none"),
            Self::TruncateExit6 => Some("truncate-exit6"),
            Self::ResultsSigkill => Some("results-sigkill"),
            Self::SigkillBeforeResults => Some("sigkill-before-results"),
            Self::Hang => Some("hang"),
            Self::MemoryCap | Self::SailDeadline => None,
        }
    }
}

/// Every case, in report order.
pub const ALL: &[Fault] = &[
    Fault::None,
    Fault::TruncateExit6,
    Fault::ResultsSigkill,
    Fault::SigkillBeforeResults,
    Fault::Hang,
    Fault::MemoryCap,
    Fault::SailDeadline,
];

/// One row of the fault report.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    /// The fault.
    pub fault: Fault,
    /// Harness.
    pub harness: String,
    /// What direct Kani claimed under the fault (None when not run).
    pub direct: Option<Claim>,
    /// Direct driver exit and verdict lines.
    pub direct_detail: Option<String>,
    /// What the product decoder alone says of the direct capture.
    pub decoder_alone: Option<String>,
    /// What Sail claimed.
    pub sail: Claim,
    /// Sail's message.
    pub sail_detail: String,
    /// Entries Sail left in its Kani state directory other than attempt
    /// captures (a cache entry would appear here).
    pub new_state_entries: Vec<String>,
    /// The rule: no state entry, and Sail made no claim under a fault (in the
    /// control row, Sail's claim equals direct's).
    pub passed: bool,
}

/// Fault-arm inputs.
pub struct Setup<'a> {
    /// The real direct oracle.
    pub direct: &'a Direct,
    /// The real Sail CLI.
    pub sail: &'a Cli,
    /// Sail's real cache directory (`SAIL_CACHE_DIR`), holding `kani/`.
    pub sail_cache: &'a Path,
    /// A slot.
    pub slot: &'a Slot,
    /// Scratch for copies and captures.
    pub scratch: &'a Path,
    /// `MemoryMax` for the memory-cap case, for example `96M`.
    pub memory_max: &'a str,
}

fn private(dir: &Path) -> Result<(), Error> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|e| Error::prepare(format!("{}: {e}", dir.display())))
}

fn install_stand_in(stage: &Path, mode: &str) -> Result<(), Error> {
    let bin = stage.join("bin");
    if !bin.join("cbmc.real").exists() {
        std::fs::rename(bin.join("cbmc"), bin.join("cbmc.real"))
            .map_err(|e| Error::prepare(e.to_string()))?;
    }
    let write = |name: &str, text: &str, exec: bool| -> Result<(), Error> {
        let path = bin.join(name);
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, text).map_err(|e| Error::prepare(e.to_string()))?;
        std::fs::set_permissions(
            &path,
            std::fs::Permissions::from_mode(if exec { 0o755 } else { 0o644 }),
        )
        .map_err(|e| Error::prepare(e.to_string()))
    };
    write("cbmc", STAND_IN, true)?;
    write("cbmc-truncate.py", TRUNCATE, false)?;
    write("cbmc.fault", mode, false)
}

fn entries(dir: &Path) -> BTreeSet<String> {
    std::fs::read_dir(dir)
        .map(|r| {
            r.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default()
}

fn decoder_alone(record: &KaniRunRecord) -> String {
    let Some(exit) = record.exit else {
        return "no driver exit status".into();
    };
    let export = record
        .export
        .as_ref()
        .and_then(|e| serde_json::to_vec(e).ok())
        .unwrap_or_default();
    match sail_kani::decode(&record.harness, exit, record.stdout.as_bytes(), &export) {
        Ok(observation) => format!("admits {:?}", observation.outcome()),
        Err(error) => format!("refuses: {error}"),
    }
}

fn detail(record: &KaniRunRecord) -> String {
    let verdicts: Vec<&str> = record
        .stdout
        .lines()
        .filter(|l| {
            l.starts_with("VERIFICATION:- ")
                || l.contains("CBMC failed")
                || l.starts_with("Complete - ")
        })
        .collect();
    format!(
        "exit {:?} signal {:?}; {}",
        record.exit,
        record.signal,
        verdicts.join("; ")
    )
}

/// Whether `sudo systemd-run --scope` works here (not in a container).
fn scopes_available() -> bool {
    Command::new("sudo")
        .args(["-n", "systemd-run", "--quiet", "--scope", "true"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Runs every fault case and returns the rows: `small` for the CBMC-boundary
/// faults, `hog` (whose CBMC needs far more memory and time than its build)
/// for the memory cap and Sail's deadline.
///
/// # Errors
/// `SAIL6321`/`SAIL6322` when a copy or a run cannot be made.
pub fn run(setup: &Setup<'_>, small: &Harness, hog: &Harness) -> Result<Vec<Row>, Error> {
    private(setup.scratch)?;
    let scratch = setup
        .scratch
        .canonicalize()
        .map_err(|e| Error::prepare(e.to_string()))?;
    // Direct: a copy of the stage with the stand-in.
    let stage_copy = scratch.join("direct-stage");
    copy_tree(&setup.direct.stage, &stage_copy)?;
    let faulted = Direct {
        stage: stage_copy.clone(),
        deadline: Duration::from_mins(2),
    };
    // Sail: a copy of its authenticated original tools in a private cache.
    // Sail stages them itself in the control row; later rows change that stage.
    let sail_cache = scratch.join("sail-cache");
    private(&sail_cache.join("kani"))?;
    copy_tree(
        &setup.sail_cache.join("kani/original-0.68.0-linux-x86_64"),
        &sail_cache.join("kani/original-0.68.0-linux-x86_64"),
    )?;
    let sail_stage = sail_cache.join("kani/stage-0.68.0-linux-x86_64");
    let mut copied = setup.sail.clone();
    copied.kani_root = sail_cache.join("kani");
    copied.environment.insert(
        "SAIL_CACHE_DIR".into(),
        sail_cache.to_string_lossy().into_owned(),
    );
    let mut rows = Vec::new();
    let scopes = scopes_available();
    for (index, fault) in ALL.iter().copied().enumerate() {
        if fault == Fault::MemoryCap && !scopes {
            eprintln!(
                "kani fault arm: memory-cap case not run: no `sudo systemd-run --scope` here"
            );
            continue;
        }
        let capture = scratch.join(format!("case-{index}"));
        let harness = if matches!(fault, Fault::MemoryCap | Fault::SailDeadline) {
            hog
        } else {
            small
        };
        let (direct, sail, state_dir) = match fault {
            Fault::None => {
                // Control: real tools on both sides; Sail's copied cache must
                // reproduce the direct claim.
                let before = entries(&copied.kani_root);
                let d = setup.direct.run(harness, setup.slot, &capture)?;
                let s = copied.run(harness, setup.slot)?;
                (Some(d), s, (copied.kani_root.clone(), before))
            }
            Fault::MemoryCap => {
                let before = entries(&setup.sail.kani_root);
                let d = capped(setup, harness, &capture, true)?;
                let s = capped(setup, harness, &capture.with_extension("sail"), false)?;
                (Some(d), s, (setup.sail.kani_root.clone(), before))
            }
            Fault::SailDeadline => {
                let before = entries(&setup.sail.kani_root);
                let mut quick = setup.sail.clone();
                quick.timeout_ms = 1000;
                (
                    None,
                    quick.run(harness, setup.slot)?,
                    (setup.sail.kani_root.clone(), before),
                )
            }
            _ => {
                let mode = fault.mode().unwrap_or("none");
                install_stand_in(&stage_copy, mode)?;
                install_stand_in(&sail_stage, mode)?;
                let before = entries(&copied.kani_root);
                let d = faulted.run(harness, setup.slot, &capture)?;
                let s = copied.run(harness, setup.slot)?;
                (Some(d), s, (copied.kani_root.clone(), before))
            }
        };
        let new_state_entries: Vec<String> = entries(&state_dir.0)
            .difference(&state_dir.1)
            .filter(|e| !e.starts_with("attempt-") && *e != "stage-0.68.0-linux-x86_64")
            .cloned()
            .collect();
        let passed = new_state_entries.is_empty()
            && if fault == Fault::None {
                direct.as_ref().is_some_and(|d| d.claim == sail.claim)
            } else {
                !sail.claim.is_claim()
            };
        rows.push(Row {
            fault,
            harness: harness.name.clone(),
            direct: direct.as_ref().map(|d| d.claim),
            direct_detail: direct.as_ref().map(detail),
            decoder_alone: direct.as_ref().map(decoder_alone),
            sail: sail.claim,
            sail_detail: sail
                .refusal
                .clone()
                .unwrap_or_else(|| format!("exit {:?}", sail.sail_exit)),
            new_state_entries,
            passed,
        });
    }
    Ok(rows)
}

/// `sudo systemd-run --scope -p MemoryMax=…` with swap off and the kernel's
/// OOM killer left to act on the real tools, then `env -i`.
fn wrapper(setup: &Setup<'_>) -> Command {
    let id = |flag: &str| -> String {
        Command::new("id")
            .arg(flag)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .unwrap_or_default()
    };
    let mut wrapper = Command::new("sudo");
    wrapper.args([
        "systemd-run",
        "--quiet",
        "--scope",
        "-p",
        &format!("MemoryMax={}", setup.memory_max),
        "-p",
        "MemorySwapMax=0",
        "-p",
        "OOMPolicy=continue",
        &format!("--uid={}", id("-u")),
        &format!("--gid={}", id("-g")),
        "--",
        "env",
        "-i",
    ]);
    wrapper
}

fn capped(
    setup: &Setup<'_>,
    harness: &Harness,
    capture: &Path,
    direct: bool,
) -> Result<KaniRunRecord, Error> {
    if direct {
        capped_direct(setup, harness, capture)
    } else {
        capped_sail(setup, harness)
    }
}

fn capped_direct(
    setup: &Setup<'_>,
    harness: &Harness,
    capture: &Path,
) -> Result<KaniRunRecord, Error> {
    for dir in ["", "home", "tmp", "target"] {
        private(&capture.join(dir))?;
    }
    let argv = Direct::argv(harness, capture);
    let environment = setup.direct.environment(capture, &setup.slot.cargo_home);
    let mut command = wrapper(setup);
    command
        .args(environment.iter().map(|(k, v)| format!("{k}={v}")))
        .arg(setup.direct.stage.join("bin/kani-driver"))
        .args(&argv)
        .current_dir(&harness.package);
    let started = Instant::now();
    let (exit, signal, stdout, stderr) = spawn(&mut command, Duration::from_mins(5))?;
    let mut record = KaniRunRecord {
        arm: super::Arm::Direct,
        project: harness.project.clone(),
        harness: harness.name.clone(),
        unwind: harness.unwind,
        argv,
        environment,
        capture: capture.to_path_buf(),
        exit,
        signal,
        stdout,
        stderr,
        export: std::fs::read(capture.join("export.json"))
            .ok()
            .and_then(|b| super::strict_json::parse(&b).ok()),
        claim: Claim::NoClaim(NoClaim::Refused),
        refusal: None,
        sail_exit: None,
        wall_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    };
    record.claim = Claim::of_direct(&record);
    Ok(record)
}

fn capped_sail(setup: &Setup<'_>, harness: &Harness) -> Result<KaniRunRecord, Error> {
    let sail = setup.sail;
    let mut command = wrapper(setup);
    command
        .args(sail.environment.iter().map(|(k, v)| format!("{k}={v}")))
        .arg(&sail.sail)
        .args(sail.arguments(harness, setup.slot));
    let (exit, _, stdout, stderr) = spawn(&mut command, Duration::from_mins(5))?;
    Ok(KaniRunRecord {
        arm: super::Arm::Sail,
        project: harness.project.clone(),
        harness: harness.name.clone(),
        unwind: harness.unwind,
        argv: Vec::new(),
        environment: std::collections::BTreeMap::new(),
        capture: PathBuf::new(),
        exit: None,
        signal: None,
        stdout: String::new(),
        stderr: String::new(),
        export: None,
        claim: sail_claim(exit, &stdout, &stderr, ""),
        refusal: Some(stderr.trim().to_owned()),
        sail_exit: exit,
        wall_ms: 0,
    })
}
