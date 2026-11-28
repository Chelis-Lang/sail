//! The comparator admits only named, witnessed variation.
use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value, json};

use super::{KaniVerdict, classify, compare, witness};
use crate::kani::record::{Arm, Claim, KaniRunRecord, NoClaim};

fn export(run: &str, millis: u64, description: &str, status: &str) -> Value {
    json!({
        "metadata": {"version": "1.0", "timestamp": format!("2026-11-28T10:00:0{millis}.1Z")},
        "project": {"output_dir": format!("/r/{run}/target/kani/out")},
        "harness_metadata": [{"pretty_name": "h", "goto_file": format!("/r/{run}/target/kani/out/h.symtab.out")}],
        "cbmc": [{"harness_id": "h", "configuration": {"solver": "cadical", "object_bits": 16},
                  "cbmc_stats": {"runtime_symex_s": 0.01 * f64::from(u32::try_from(millis).unwrap_or(0)), "vccs_generated": 4}}],
        "verification_results": {"summary": {"duration_ms": millis, "status": "completed"},
            "results": [{"harness_id": "h", "status": "Success", "duration_ms": millis, "checks": [
                {"id": 1, "function": "f", "category": "assertion", "status": status,
                 "description": "assertion failed: x", "location": {"file": "src/lib.rs", "line": "3", "column": "5"}},
                {"id": 2, "function": "g", "category": "sanity_check", "status": "Success",
                 "description": description, "location": {"file": "src/lib.rs", "line": "9", "column": "1"}}]}]}
    })
}

fn stdout(run: &str, secs: &str, description: &str) -> String {
    format!(
        "Kani Rust Verifier 0.68.0 (cargo plugin)\nReading GOTO program from file /r/{run}/target/kani/out/h.out\nRuntime Symex: {secs}s\nCheck 1: f.assertion.1\n\t - Status: SUCCESS\nCheck 2: g.sanity_check.1\n\t - Description: \"{description}\"\nVERIFICATION:- SUCCESSFUL\nVerification Time: {secs}s\n"
    )
}

const VTABLE: &str = "Correct CBMC vtable size for Ty { id: 41, kind: X } (DefId { id: 7 })";

fn record(arm: Arm, run: &str, millis: u64, vtable_id: u32) -> KaniRunRecord {
    let description = VTABLE.replace("41", &vtable_id.to_string());
    let mut r = KaniRunRecord {
        arm,
        project: "p".into(),
        harness: "h".into(),
        unwind: 2,
        argv: vec![
            "kani".into(),
            "--target-dir".into(),
            format!("/r/{run}/target"),
        ],
        environment: BTreeMap::from([
            ("HOME".into(), format!("/r/{run}/home")),
            ("LC_ALL".into(), "C".into()),
        ]),
        capture: PathBuf::from(format!("/r/{run}")),
        exit: Some(0),
        signal: None,
        stdout: stdout(run, &format!("0.{millis}"), &description),
        stderr: format!(
            "   Compiling p v0.1.0 (/src/p)\n    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.{millis}s\n"
        ),
        export: Some(export(run, millis, &description, "Success")),
        claim: Claim::BoundedSuccess,
        refusal: None,
        sail_exit: None,
        wall_ms: millis,
    };
    if arm == Arm::Sail {
        r.sail_exit = Some(0);
    }
    r
}

#[test]
fn named_timing_paths_and_vtable_indices_vary_and_nothing_else() {
    let a = record(Arm::Direct, "a", 1, 41);
    let mut b = record(Arm::Direct, "b", 2, 43);
    b.stderr = format!(
        "   Compiling q v0.2.0\n{}",
        b.stderr
            .replace("    Finished", "   Compiling r v0.3.0\n    Finished")
    );
    let mut a = a;
    a.stderr = a.stderr.replace(
        "   Compiling p",
        "   Compiling r v0.3.0\n   Compiling q v0.2.0\n   Compiling p",
    );
    let c = compare(&a, &b);
    assert!(c.clean(), "{:?}", c.differences);
    for name in [
        "export:metadata.timestamp",
        "export:verification_results.results[].duration_ms",
        "export:cbmc[].cbmc_stats.runtime_symex_s",
        "export:project.output_dir",
        "export:harness_metadata[].goto_file",
        "stream:Runtime Symex",
        "stream:Verification Time",
        "stream:Cargo Finished",
        "stream:Reading GOTO program",
        "stream:Cargo Compiling order",
        "check:vtable-size description",
    ] {
        assert!(c.variances.contains_key(name), "{name}: {:?}", c.variances);
    }
    assert_eq!(c.checks, 2);
}

type Mutation = Box<dyn Fn(&mut KaniRunRecord)>;

#[test]
#[allow(clippy::too_many_lines, reason = "one table of mutations")]
fn every_other_field_is_exact() {
    let a = record(Arm::Direct, "a", 1, 41);
    let mutations: Vec<(&str, Mutation)> = vec![
        (
            "status",
            Box::new(|r| r.export = Some(export("b", 1, VTABLE, "Failure"))),
        ),
        (
            "stats",
            Box::new(|r| {
                if let Some(e) = r.export.as_mut() {
                    e["cbmc"][0]["cbmc_stats"]["vccs_generated"] = json!(5);
                }
            }),
        ),
        (
            "solver",
            Box::new(|r| {
                if let Some(e) = r.export.as_mut() {
                    e["cbmc"][0]["configuration"]["solver"] = json!("kissat");
                }
            }),
        ),
        (
            "location",
            Box::new(|r| {
                if let Some(e) = r.export.as_mut() {
                    e["verification_results"]["results"][0]["checks"][0]["location"]["line"] =
                        json!("4");
                }
            }),
        ),
        (
            "vtable text",
            Box::new(|r| {
                let other = VTABLE.replace("kind: X", "kind: Y");
                r.export = Some(export("b", 1, &other, "Success"));
            }),
        ),
        (
            "extra check",
            Box::new(|r| {
                if let Some(e) = r.export.as_mut()
                    && let Some(checks) =
                        e["verification_results"]["results"][0]["checks"].as_array_mut()
                {
                    checks.push(json!({"id": 3}));
                }
            }),
        ),
        (
            "path escape",
            Box::new(|r| {
                if let Some(e) = r.export.as_mut() {
                    e["project"]["output_dir"] = json!("/r/b/../a/target/kani/out");
                }
            }),
        ),
        (
            "negative time",
            Box::new(|r| {
                if let Some(e) = r.export.as_mut() {
                    e["cbmc"][0]["cbmc_stats"]["runtime_symex_s"] = json!(-1.0);
                }
            }),
        ),
        (
            "stdout line",
            Box::new(|r| r.stdout = r.stdout.replace("Status: SUCCESS", "Status: FAILURE")),
        ),
        (
            "line ending",
            Box::new(|r| r.stdout = r.stdout.replacen('\n', "\r\n", 1)),
        ),
        (
            "extra Compiling line",
            Box::new(|r| r.stderr.push_str("   Compiling z v1.0.0\n")),
        ),
        (
            "extra stderr line",
            Box::new(|r| {
                r.stderr
                    .push_str("    Blocking waiting for file lock on package cache\n");
            }),
        ),
        (
            "runtime label",
            Box::new(|r| {
                r.stdout = r
                    .stdout
                    .replace("Runtime Symex: 0.1s", "Runtime Other: 0.1s");
            }),
        ),
        ("exit", Box::new(|r| r.exit = Some(1))),
        ("argv", Box::new(|r| r.argv.push("--unwind".into()))),
        (
            "environment",
            Box::new(|r| {
                r.environment.insert("RUSTFLAGS".into(), "-C x".into());
            }),
        ),
    ];
    for (name, mutate) in mutations {
        let mut b = record(Arm::Direct, "b", 1, 41);
        mutate(&mut b);
        assert!(!compare(&a, &b).clean(), "{name} must be a difference");
    }
}

#[test]
fn verdicts_follow_the_direct_runs() {
    let d = [
        record(Arm::Direct, "a", 1, 41),
        record(Arm::Direct, "b", 2, 42),
    ];
    let same = classify(&d, &record(Arm::Sail, "s", 3, 44));
    assert_eq!(same.verdict, KaniVerdict::Same, "{:?}", same.sail_against);

    let mut wrong = record(Arm::Sail, "s", 3, 44);
    wrong.claim = Claim::Counterexample;
    assert_eq!(classify(&d, &wrong).verdict, KaniVerdict::Different);

    let mut refused = record(Arm::Sail, "s", 3, 44);
    refused.claim = Claim::NoClaim(NoClaim::Unsupported);
    assert_eq!(classify(&d, &refused).verdict, KaniVerdict::NoClaim);
    refused.stdout.push_str("extra\n");
    assert_eq!(
        classify(&d, &refused).verdict,
        KaniVerdict::Different,
        "raw output still compared"
    );

    let mut unstable = [
        record(Arm::Direct, "a", 1, 41),
        record(Arm::Direct, "b", 2, 42),
    ];
    unstable[0].stdout.push_str("only in a\n");
    assert_eq!(
        classify(&unstable, &record(Arm::Sail, "s", 3, 44)).verdict,
        KaniVerdict::Unstable
    );

    assert_eq!(
        classify(&d[..1], &record(Arm::Sail, "s", 3, 44)).verdict,
        KaniVerdict::InfraError
    );
}

#[test]
fn a_named_field_seen_only_against_sail_is_a_difference() {
    // The two direct runs agree on the vtable text; Sail differs on it.
    let d = [
        record(Arm::Direct, "a", 1, 41),
        record(Arm::Direct, "b", 2, 41),
    ];
    let mut results = vec![classify(&d, &record(Arm::Sail, "s", 3, 44))];
    assert_eq!(results[0].verdict, KaniVerdict::Same);
    let unwitnessed = witness(&mut results);
    assert!(unwitnessed.contains("check:vtable-size description"));
    assert_eq!(results[0].verdict, KaniVerdict::Different);

    // Witnessed by another harness's direct pair in the same project: allowed.
    let other = [
        record(Arm::Direct, "a", 1, 41),
        record(Arm::Direct, "b", 2, 42),
    ];
    let mut results = vec![
        classify(&d, &record(Arm::Sail, "s", 3, 44)),
        classify(&other, &record(Arm::Sail, "t", 3, 41)),
    ];
    assert!(witness(&mut results).is_empty());
    assert!(results.iter().all(|r| r.verdict == KaniVerdict::Same));
}
