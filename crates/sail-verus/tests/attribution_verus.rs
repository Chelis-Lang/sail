//! Attribution against the pinned Verus (ADR 0014): the log flags sail adds
//! leave Z3's input unchanged, and obligation identities survive unrelated
//! edits.
//!
//! Needs the pinned Verus (`cargo xtask fetch-verus`, or `SAIL_VERUS_HOME`),
//! so it is ignored by default and runs in `ci/run.sh solver`:
//! `SAIL_TEST_Z3=<pinned z3> cargo nextest run -p sail-verus --run-ignored all`.
#![cfg(unix)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use sail_verus::attribution::{
    Attribution, Provenance, Status, attribute, json_documents, read_logs,
};
use sail_verus::toolchain::{Installed, Pins};

const BASE: &str = r"use vstd::prelude::*;
verus! {

mod arith {
    use vstd::prelude::*;

    pub proof fn mul_pos(a: int, b: int)
        requires a > 0, b > 0,
        ensures a * b > 0,
    {
        assert(a * b > 0) by (nonlinear_arith)
            requires a > 0, b > 0;
    }

    pub proof fn bv_and(x: u32)
        ensures x & 0 == 0,
    {
        assert(x & 0 == 0) by (bit_vector);
    }
}

pub struct Counter { pub n: u64 }

impl Counter {
    pub fn incr(&mut self)
        requires old(self).n < 100,
        ensures final(self).n == old(self).n + 1,
    {
        self.n = self.n + 1;
    }
}

fn max(a: u32, b: u32) -> (r: u32)
    ensures r >= a, r >= b, r == a || r == b,
{ if a > b { a } else { b } }

fn sum_to(n: u32) -> (s: u64)
    requires n < 1000,
    ensures s == (n as u64) * ((n as u64) + 1) / 2,
{
    let mut s: u64 = 0;
    let mut i: u32 = 0;
    while i < n
        invariant i <= n, n < 1000, s == (i as u64) * ((i as u64) + 1) / 2,
        decreases n - i,
    {
        i += 1;
        s += i as u64;
    }
    s
}

fn bad(x: u32) -> (r: u32)
    ensures r > x,
{ x }

fn main() {}
}
";

fn installed() -> Installed {
    let pins = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/toolchain.toml");
    match Pins::load(&pins).and_then(Installed::locate) {
        Ok(i) => i,
        Err(e) => unreachable!("the pinned Verus is needed: {e}"),
    }
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sail-attr-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    assert!(std::fs::create_dir_all(&d).is_ok());
    d
}

/// A solver wrapper that records each process's input, then runs `z3`.
fn recording_solver(dir: &Path, z3: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let inputs = dir.join("inputs");
    assert!(std::fs::create_dir_all(&inputs).is_ok());
    let path = dir.join("z3");
    let script = format!(
        "#!/bin/sh\nf=$(mktemp '{}/in.XXXXXX')\ntee \"$f\" | '{}' \"$@\"\n",
        inputs.display(),
        z3.display()
    );
    assert!(std::fs::write(&path, script).is_ok());
    assert!(std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).is_ok());
    path
}

/// Sorted SHA-256s of every process's input.
fn input_digests(dir: &Path) -> Vec<String> {
    use sha2::Digest as _;
    let mut out: Vec<String> = std::fs::read_dir(dir.join("inputs"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| std::fs::read(e.path()).ok())
        .map(|b| format!("{:x}", sha2::Sha256::digest(b)))
        .collect();
    out.sort();
    out
}

struct Run {
    exit: Option<i32>,
    stderr: String,
    attribution: Attribution,
    inputs: Vec<String>,
}

/// Runs Verus on `source` through the recording solver, with or without the
/// log flags sail adds.
fn run(tag: &str, source: &str, with_logs: bool) -> Run {
    let v = installed();
    let dir = scratch(tag);
    let file = dir.join("input.rs");
    assert!(std::fs::write(&file, source).is_ok());
    let solver = recording_solver(&dir, &v.z3);
    let logs = dir.join("logs");
    let mut cmd = Command::new(&v.verus);
    cmd.arg("input.rs")
        .args(["--output-json", "--time"])
        .current_dir(&dir)
        .env("VERUS_Z3_PATH", &solver);
    if with_logs {
        cmd.args(["--log", "smt-transcript", "--log", "air", "--log-dir"])
            .arg(&logs);
    }
    let out = match cmd.output() {
        Ok(o) => o,
        Err(e) => unreachable!("cannot run verus: {e}"),
    };
    let (docs, _) = json_documents(&out.stdout);
    assert_eq!(docs.len(), 1, "{}", String::from_utf8_lossy(&out.stderr));
    let files = with_logs.then(|| read_logs(&logs).unwrap_or_default());
    let attribution = attribute(
        &docs,
        files.as_deref(),
        &Provenance::NoCache,
        Some("no logs"),
    );
    let run = Run {
        exit: out.status.code(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        attribution,
        inputs: input_digests(&dir),
    };
    let _ = std::fs::remove_dir_all(&dir);
    run
}

/// Obligation id → span, for every function.
fn ids(a: &Attribution) -> BTreeMap<String, Option<String>> {
    a.crates
        .iter()
        .flat_map(|c| &c.functions)
        .flat_map(|f| f.obligations.iter().flatten())
        .map(|o| (o.id.to_string(), o.span.clone()))
        .collect()
}

fn functions_of(ids: &BTreeMap<String, Option<String>>, skip: &[&str]) -> BTreeSet<String> {
    ids.keys()
        .filter(|id| !skip.iter().any(|s| id.starts_with(&format!("{s}#"))))
        .cloned()
        .collect()
}

#[test]
#[ignore = "needs the pinned Verus"]
fn log_flags_leave_the_pipe_unchanged() {
    let plain = run("plain", BASE, false);
    let logged = run("logged", BASE, true);
    assert_ne!(plain.inputs, Vec::<String>::new());
    assert_eq!(
        plain.inputs, logged.inputs,
        "the log flags changed Z3's input"
    );
    assert_eq!(plain.stderr, logged.stderr);
    assert_eq!(plain.exit, logged.exit);
    assert_eq!(
        logged.attribution.status,
        Status::Complete,
        "{:?}",
        logged.attribution.notes
    );
    // Every function Verus reports has obligations, and verdicts agree.
    for f in logged.attribution.crates.iter().flat_map(|c| &c.functions) {
        assert!(
            f.obligations.as_ref().is_some_and(|o| !o.is_empty()),
            "{}",
            f.function
        );
        assert_eq!(
            Some(f.verdict() == sail_ledger::Verdict::Verified),
            f.verus_success,
            "{}",
            f.function
        );
    }
}

#[test]
#[ignore = "needs the pinned Verus"]
fn obligation_ids_survive_unrelated_edits() {
    let base = run("base", BASE, true);
    // Unrelated edits: a new function and comments above everything (every
    // span moves), `bad` and `max` swapped, and `max`'s body changed.
    let edited = BASE
        .replace(
            "verus! {\n",
            "verus! {\n// a comment\n// another\n\nfn added(x: u8) -> (r: u8) ensures r == x { x }\n\n",
        )
        .replace("{ if a > b { a } else { b } }", "{ if a >= b { a } else { b } }");
    let (Some(bad_start), Some(bad_end)) = (edited.find("fn bad("), edited.find("fn main()"))
    else {
        unreachable!("fixture changed")
    };
    let bad_fn = edited
        .get(bad_start..bad_end)
        .unwrap_or_default()
        .to_owned();
    let edited =
        edited
            .replacen(&bad_fn, "", 1)
            .replacen("fn max(", &format!("{bad_fn}fn max("), 1);
    let after = run("edited", &edited, true);
    let (before, now) = (ids(&base.attribution), ids(&after.attribution));
    let untouched = ["input::max", "input::added"];
    assert_eq!(
        functions_of(&before, &untouched),
        functions_of(&now, &untouched)
    );
    // The identities held although every span moved.
    let moved = before
        .iter()
        .filter(|(id, span)| now.get(*id).is_some_and(|s| s != *span))
        .count();
    assert!(moved > 5, "spans should have moved: {moved}");
    // The edited function keeps its own identity too; the new one is new.
    assert!(now.contains_key("input::max#Function-Def/0"));
    assert!(now.contains_key("input::added#Function-Def/0"));
    // The loop and the function body are distinct obligations of `sum_to`.
    assert!(now.contains_key("input::sum_to#Function-Def/0"));
    assert!(now.contains_key("input::sum_to#Function-Def/1"));
    assert!(now.contains_key("input::arith::mul_pos#Function-Def/spinoff-1"));
}

#[test]
#[ignore = "needs the pinned Verus"]
fn renaming_or_moving_a_function_changes_its_identity() {
    // Documented in docs/output-schema.md: identity is path-based.
    let renamed = BASE.replace("fn bad(", "fn worse(");
    let after = run("renamed", &renamed, true);
    let now = ids(&after.attribution);
    assert!(!now.contains_key("input::bad#Function-Def/0"));
    assert!(now.contains_key("input::worse#Function-Def/0"));
}

#[test]
#[ignore = "needs the pinned Verus"]
fn trait_implementation_collisions_preserve_plain_verdicts_without_guessing() {
    let source = include_str!("../../../corpus/repros/verus-ambiguous-friendly-name/input.rs");
    let plain = run("trait-collision-plain", source, false);
    let logged = run("trait-collision-logged", source, true);
    assert_eq!(plain.exit, Some(0));
    assert_eq!(logged.exit, plain.exit);
    assert_eq!(logged.stderr, plain.stderr);
    assert_eq!(logged.inputs, plain.inputs);
    assert_eq!(logged.attribution.status, Status::Partial);
    let records: Vec<_> = logged
        .attribution
        .crates
        .iter()
        .flat_map(|c| &c.functions)
        .filter(|f| f.function == "input::S::check")
        .collect();
    assert_eq!(records.len(), 2);
    assert!(records.iter().all(|f| f.obligations.is_none()
        && f.verus_success == Some(true)
        && f.verdict() == sail_ledger::Verdict::Verified));
    assert!(
        logged
            .attribution
            .notes
            .iter()
            .any(|n| n.contains("does not distinguish implementations"))
    );
}
