//! Admission, process faults and independent pinned-solver evidence.

use crate::{Cancellation, Config, Runtime, admission, process};
use sail_api::runtime::{Answer, Input, Limits, Outcome, Output, Query, Request, VERSION};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct Directory(PathBuf);
impl Directory {
    fn new() -> std::io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "sail-runtime-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn query(script: &str) -> Query {
    Query {
        obligation_id: "obligation-1".into(),
        requirement_ids: vec!["REQ-1".into(), "REQ-2".into()],
        backend: "z3".into(),
        script: script.into(),
        limits: Limits::default(),
    }
}

fn request(queries: Vec<Query>) -> Request {
    Request {
        version: VERSION,
        input: Input::Smt { queries },
    }
}

#[test]
fn full_batch_is_admitted_before_any_cache_or_backend_access() -> TestResult {
    let directory = Directory::new()?;
    let cache = directory.0.join("unopened-cache");
    let runtime = Runtime::new(Config {
        z3: Some(directory.0.join("does-not-exist")),
        cache_directory: Some(cache.clone()),
        ..Config::default()
    })?;
    let mut bad = query("(echo \"unsat\") (check-sat)");
    assert_eq!(
        runtime
            .execute(
                &request(vec![query("(check-sat)"), bad.clone()]),
                &Cancellation::default()
            )
            .err()
            .map(|e| e.kind),
        Some("unsupported_command".into())
    );
    assert!(!cache.exists());
    bad = query("(check-sat)");
    bad.requirement_ids.reverse();
    assert!(
        runtime
            .execute(&request(vec![bad]), &Cancellation::default())
            .is_err()
    );
    let mut bad = query("(check-sat)");
    bad.limits.output_bytes = 0;
    assert!(
        runtime
            .execute(&request(vec![bad]), &Cancellation::default())
            .is_err()
    );
    assert!(!cache.exists());
    // Reserve all retained stream/product bytes before accepting a large batch.
    assert!(
        runtime
            .execute(
                &request(vec![query("(check-sat)"); 3]),
                &Cancellation::default()
            )
            .is_err()
    );
    Ok(())
}

#[test]
fn forged_answers_and_products_cannot_supply_a_check() -> TestResult {
    let plan = admission::plan(&query("(check-sat)"))?;
    for forged in [
        b"(error \"bad\")\nunsat\n".as_slice(),
        b"\"unsat\"\n",
        b"success\nunsat\n",
        b"sat\nunsat\n",
        b"unsupported\n",
        b"\xff\n",
    ] {
        assert!(admission::responses(&plan, forged).is_err());
    }
    let values = admission::plan(&query("(check-sat) (get-value (x y))"))?;
    assert!(admission::responses(&values, b"sat\n((x 1))\n").is_err());
    assert!(admission::responses(&values, b"unsat\n((x 1) (y 2))\n").is_err());
    assert_eq!(
        admission::responses(&values, b"sat\n((x 1) (y 2))\n")?[0]
            .products
            .len(),
        1
    );
    Ok(())
}

#[test]
fn closed_grammar_retains_matching_limits_and_refuses_output_and_seed_overrides() -> TestResult {
    let good = query(
        "; exact comment\n(set-option :print-success false)\n(set-option :rlimit 2000000)\n(set-option :smt.random_seed 0)\n(set-option :sat.random_seed 0)\n(check-sat)\n",
    );
    assert_eq!(admission::plan(&good)?.len(), 1);
    for script in [
        "(set-option :rlimit 1) (check-sat)",
        "(set-option :timeout 1) (check-sat)",
        "(set-option :print-success true) (check-sat)",
        "(set-option :regular-output-channel \"/tmp/output\") (check-sat)",
        "(set-option :smt.random_seed 3) (check-sat)",
        "(pop 1) (check-sat)",
        "(push 999999999) (check-sat)",
        "(exit) (check-sat)",
        "(check-sat (echo \"unsat\"))",
        "(get-info :version)",
        "(check-sat) (assert false) (get-model)",
    ] {
        assert!(
            admission::plan(&query(script)).is_err(),
            "admitted {script}"
        );
    }
    Ok(())
}

#[test]
fn unavailable_backend_and_cancelled_work_are_not_solver_answers() -> TestResult {
    let runtime = Runtime::new(Config::default())?;
    assert!(!runtime.capabilities().backends[0].installed);
    let Output::Smt { results } = runtime
        .execute(
            &request(vec![query("(check-sat)")]),
            &Cancellation::default(),
        )?
        .output;
    assert_eq!(results[0].outcome, Outcome::Unavailable);
    assert_eq!(results[0].checks, []);
    let token = Cancellation::default();
    token.cancel();
    let Output::Smt { results } = runtime
        .execute(&request(vec![query("(check-sat)")]), &token)?
        .output;
    assert_eq!(results[0].outcome, Outcome::Cancelled);
    assert!(results[0].solver.is_none());
    Ok(())
}

#[cfg(unix)]
fn script(directory: &Directory, body: &str) -> std::io::Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt as _;
    let path = directory.0.join("backend");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n"))?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
    Ok(path)
}

#[test]
#[cfg(unix)]
fn output_limits_and_deadlines_kill_and_join_actual_processes() -> TestResult {
    let directory = Directory::new()?;
    let executable = script(
        &directory,
        "while :; do printf 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx'; done",
    )?;
    let limits = Limits {
        output_bytes: 17,
        wall_time_ms: 1000,
        ..Limits::default()
    };
    let captured = process::run(&executable, &[], b"", &limits, &Cancellation::default());
    assert_eq!(captured.outcome, Outcome::OutputLimit);
    assert_eq!(captured.stdout.len(), 17);
    let executable = script(&directory, "sleep 30 & wait")?;
    let limits = Limits {
        wall_time_ms: 50,
        ..Limits::default()
    };
    let started = Instant::now();
    let captured = process::run(&executable, &[], b"", &limits, &Cancellation::default());
    assert_eq!(captured.outcome, Outcome::Timeout);
    assert!(started.elapsed() < Duration::from_secs(3));
    Ok(())
}

#[test]
#[cfg(unix)]
fn cancellation_reaps_a_running_process_and_unblocks_descendant_pipes() -> TestResult {
    let directory = Directory::new()?;
    let executable = script(&directory, "sleep 30 & wait")?;
    let token = Cancellation::default();
    let cancel = token.clone();
    let started = Instant::now();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        cancel.cancel();
    });
    let captured = process::run(&executable, &[], b"", &Limits::default(), &token);
    canceller.join().map_err(|_| "canceller panicked")?;
    assert_eq!(captured.outcome, Outcome::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(3));
    Ok(())
}

#[test]
#[cfg(unix)]
fn changed_executable_never_becomes_ready() -> TestResult {
    let directory = Directory::new()?;
    let executable = script(&directory, "echo 'Z3 version 4.16.0'")?;
    let runtime = Runtime::new(Config {
        z3: Some(executable),
        ..Config::default()
    })?;
    assert!(!runtime.capabilities().backends[0].installed);
    let Output::Smt { results } = runtime
        .execute(
            &request(vec![query("(check-sat)")]),
            &Cancellation::default(),
        )?
        .output;
    assert_eq!(results[0].outcome, Outcome::Unavailable);
    assert_eq!(results[0].checks, []);
    Ok(())
}

fn pinned() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(std::env::var("SAIL_TEST_Z3").map_err(
        |_| "SAIL_TEST_Z3 must name the pinned bundled solver",
    )?))
}

fn direct(path: &Path, query: &Query) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use std::{
        io::Write as _,
        process::{Command, Stdio},
    };
    let mut child = Command::new(path)
        .args([
            "-smt2",
            "-in",
            "smt.random_seed=0",
            "sat.random_seed=0",
            &format!("rlimit={}", query.limits.resource_limit),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or("stdin missing")?
        .write_all(query.script.as_bytes())?;
    let output = child.wait_with_output()?;
    assert!(output.status.success());
    assert_eq!(output.stderr, [] as [u8; 0]);
    Ok(output.stdout)
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn actual_pinned_solver_matches_direct_on_models_cores_enums_and_incremental_checks() -> TestResult
{
    let solver = pinned()?;
    let runtime = Runtime::new(Config {
        z3: Some(solver.clone()),
        workers: 3,
        ..Config::default()
    })?;
    assert!(runtime.capabilities().backends[0].installed);
    let cases = [
        "(set-option :print-success false) (set-option :produce-models true) (set-option :rlimit 2000000) (declare-const x Int) (assert (> x 99999999999999999999999999999999)) (check-sat) (get-model) (get-value (x (+ x 1)))",
        "(set-option :produce-unsat-cores true) (declare-const x Int) (assert (! (> x 0) :named positive)) (assert (! (< x 0) :named negative)) (check-sat) (get-unsat-core)",
        "(set-option :produce-models true) (declare-datatypes () ((State idle busy))) (declare-const state State) (assert (= state busy)) (check-sat) (get-value (state))",
        "(declare-const x Int) (assert (> x 0)) (check-sat) (push 1) (assert (< x 0)) (check-sat) (pop 1) (check-sat)",
    ];
    let queries = cases
        .iter()
        .map(|script| {
            let mut item = query(script);
            item.limits.output_bytes = 1024 * 1024;
            item
        })
        .collect::<Vec<_>>();
    // Establish direct results before enabling any acceleration.
    let expected = queries
        .iter()
        .map(|query| direct(&solver, query))
        .collect::<Result<Vec<_>, _>>()?;
    let Output::Smt { results } = runtime
        .execute(&request(queries), &Cancellation::default())?
        .output;
    for (result, expected) in results.iter().zip(expected) {
        assert_eq!(result.outcome, Outcome::Completed, "{result:?}");
        assert_eq!(result.stdout, expected);
        assert!(!result.cache_hit);
    }
    assert_eq!(results[1].checks[0].answer, Answer::Unsat);
    assert_eq!(
        results[3]
            .checks
            .iter()
            .map(|check| check.answer)
            .collect::<Vec<_>>(),
        [Answer::Sat, Answer::Unsat, Answer::Sat]
    );
    Ok(())
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn actual_cache_binds_exact_content_limits_and_identity_and_survives_corruption() -> TestResult {
    let directory = Directory::new()?;
    let cache = directory.0.join("cache");
    let runtime = Runtime::new(Config {
        z3: Some(pinned()?),
        cache_directory: Some(cache.clone()),
        ..Config::default()
    })?;
    let original = query("(declare-const x Int) (assert (> x 2)) (check-sat)");
    let execute = |query| -> Result<_, Box<dyn std::error::Error>> {
        let Output::Smt { results } = runtime
            .execute(&request(vec![query]), &Cancellation::default())?
            .output;
        results
            .into_iter()
            .next()
            .ok_or_else(|| "missing result".into())
    };
    let first = execute(original.clone())?;
    assert_eq!(first.outcome, Outcome::Completed);
    assert!(!first.cache_hit);
    let mut renamed = original.clone();
    renamed.obligation_id = "other-obligation".into();
    renamed.requirement_ids = vec!["OTHER-REQ".into()];
    let reused = execute(renamed)?;
    assert!(reused.cache_hit);
    assert_eq!(reused.stdout, first.stdout);
    assert_eq!(reused.requirement_ids, ["OTHER-REQ"]);
    let mut changed = original.clone();
    changed.script.push_str("\n; different exact bytes\n");
    assert!(!execute(changed)?.cache_hit);
    let mut changed = original.clone();
    changed.limits.resource_limit += 1;
    assert!(!execute(changed)?.cache_hit);
    corrupt(&cache)?;
    let restored = execute(original)?;
    assert_eq!(restored.outcome, Outcome::Completed);
    assert_eq!(restored.stdout, first.stdout);
    assert!(!restored.cache_hit);
    Ok(())
}

#[test]
#[ignore = "needs SAIL_TEST_Z3"]
fn actual_unknown_is_explicit_and_never_cached() -> TestResult {
    let directory = Directory::new()?;
    let runtime = Runtime::new(Config {
        z3: Some(pinned()?),
        cache_directory: Some(directory.0.join("cache")),
        ..Config::default()
    })?;
    let mut item = query(
        "(declare-const x Int) (assert (> (* x x) 10)) (check-sat) (get-info :reason-unknown)",
    );
    item.limits.resource_limit = 1;
    for _ in 0..2 {
        let Output::Smt { results } = runtime
            .execute(&request(vec![item.clone()]), &Cancellation::default())?
            .output;
        assert_eq!(results[0].outcome, Outcome::Completed, "{:?}", results[0]);
        assert_eq!(results[0].checks[0].answer, Answer::Unknown);
        assert!(!results[0].cache_hit);
    }
    Ok(())
}

fn corrupt(path: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(path)? {
        let path = entry?.path();
        if path.is_dir() {
            corrupt(&path)?;
        } else {
            std::fs::write(path, b"corrupt")?;
        }
    }
    Ok(())
}

#[test]
#[cfg(unix)]
fn ambient_environment_cannot_change_authenticated_execution() -> TestResult {
    const MARKER: &str = "SAIL_TEST_AMBIENT_RUNTIME";
    if std::env::var_os(MARKER).is_some() {
        let captured = process::run(
            Path::new("/bin/sh"),
            &["-c".into(), format!("printf '%s' \"${{{MARKER}-unset}}\"")],
            b"",
            &Limits::default(),
            &Cancellation::default(),
        );
        assert_eq!(captured.outcome, Outcome::Completed);
        assert_eq!(captured.stdout, b"unset");
        return Ok(());
    }
    let child = std::process::Command::new(std::env::current_exe()?)
        .args([
            "--exact",
            "tests::ambient_environment_cannot_change_authenticated_execution",
            "--nocapture",
        ])
        .env(MARKER, "must-not-reach-backend")
        .output()?;
    assert!(
        child.status.success(),
        "{}",
        String::from_utf8_lossy(&child.stderr)
    );
    Ok(())
}
