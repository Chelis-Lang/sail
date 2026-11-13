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
        .output
    else {
        return Err("expected SMT family".into());
    };
    assert_eq!(results[0].outcome, Outcome::Unavailable);
    assert_eq!(results[0].checks, []);
    let token = Cancellation::default();
    token.cancel();
    let Output::Smt { results } = runtime
        .execute(&request(vec![query("(check-sat)")]), &token)?
        .output
    else {
        return Err("expected SMT family".into());
    };
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
    // The output limit is under test here, so the wall-time bound must not
    // race it: macOS assesses this freshly written script on its first exec,
    // which under heavy parallel load has taken over ten seconds.
    let limits = Limits {
        output_bytes: 17,
        wall_time_ms: 120_000,
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
        .output
    else {
        return Err("expected SMT family".into());
    };
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
        .output
    else {
        return Err("expected SMT family".into());
    };
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
            .output
        else {
            return Err("expected SMT family".into());
        };
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
            .output
        else {
            return Err("expected SMT family".into());
        };
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

// ---------------------------------------------------------------------------
// SPIKE (v0.3.0 track G): second backend. Everything above is unchanged.
// ---------------------------------------------------------------------------

/// The Z3 argv and cache-key bytes are exactly what they were before the
/// `Backend` abstraction: the key is recomputed here from the original
/// literal tuple and compared to the refactored `store::key`.
#[test]
fn z3_backend_keeps_argv_and_cache_key_bytes() -> TestResult {
    use crate::backend::{self, Backend as _};
    use sail_api::runtime::SolverIdentity;
    let item = query("(declare-const x Int) (assert (> x 2)) (check-sat)");
    assert_eq!(
        backend::Z3.args(&item.limits),
        [
            "-smt2",
            "-in",
            "smt.random_seed=0",
            "sat.random_seed=0",
            "rlimit=2000000"
        ]
    );
    let solver = SolverIdentity {
        backend: "z3".into(),
        version: "4.16.0".into(),
        sha256: "e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765".into(),
        platform: "linux-x86_64".into(),
    };
    let original = serde_json::to_vec(&(
        "runtime-smt-v1",
        &item.script,
        &solver,
        &item.limits,
        "smt.random_seed=0",
        "sat.random_seed=0",
        "sail-process-env-v1:clear:LC_ALL=C",
    ))?;
    let key = crate::store::key(&item, &solver).ok_or("no key")?;
    assert_eq!(key.0, sail_key::sha256(&original));
    // The same script under a cvc5 identity can never share that key.
    let other = SolverIdentity {
        backend: "cvc5".into(),
        ..solver
    };
    assert_ne!(crate::store::key(&item, &other).ok_or("no key")?.0, key.0);
    Ok(())
}

#[test]
fn cvc5_option_table_is_closed_and_refuses_z3_dialect_options() -> TestResult {
    let cvc5 = |script: &str| {
        let mut item = query(script);
        item.backend = "cvc5".into();
        admission::plan(&item)
    };
    assert_eq!(
        cvc5(
            "(set-logic ALL) (set-option :print-success false) (set-option :produce-models true) (set-option :seed 0) (set-option :sat-random-seed 0) (set-option :rlimit-per 2000000) (check-sat)"
        )?
        .len(),
        1
    );
    for script in [
        // cvc5 answers `unsupported` and continues on unknown options.
        "(set-option :smt.random_seed 0) (check-sat)",
        "(set-option :sat.random_seed 0) (check-sat)",
        // The cumulative limit and any conflicting per-check limit.
        "(set-option :rlimit 2000000) (check-sat)",
        "(set-option :rlimit-per 5) (check-sat)",
        "(set-option :tlimit 5) (check-sat)",
        "(set-option :tlimit-per 5) (check-sat)",
        "(set-option :seed 1) (check-sat)",
        "(set-option :regular-output-channel \"/tmp/output\") (check-sat)",
        "(set-option :dump-proofs true) (check-sat)",
    ] {
        assert!(cvc5(script).is_err(), "admitted {script}");
    }
    // The Z3 table is unchanged and still refuses cvc5's spellings.
    assert!(admission::plan(&query("(set-option :seed 0) (check-sat)")).is_err());
    // Reason-unknown dialect: cvc5 prints a bare symbol, Z3 a string.
    let plan = cvc5("(check-sat) (get-info :reason-unknown)")?;
    assert!(admission::responses(&plan, b"unknown\n(:reason-unknown resourceout)\n").is_ok());
    let plan = admission::plan(&query("(check-sat) (get-info :reason-unknown)"))?;
    assert!(admission::responses(&plan, b"unknown\n(:reason-unknown resourceout)\n").is_err());
    Ok(())
}

fn pinned_cvc5() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(
        std::env::var("SAIL_TEST_CVC5").map_err(|_| "SAIL_TEST_CVC5 must name the pinned cvc5")?,
    ))
}

/// The runtime transcripts in the SMT-LIB 2.6 forms cvc5 accepts, plus
/// push/pop, check-sat-assuming and get-value cases.
const CVC5_CASES: [&str; 8] = [
    "(set-option :print-success false) (set-option :produce-models true) (set-option :rlimit-per 2000000) (declare-const x Int) (assert (> x 99999999999999999999999999999999)) (check-sat) (get-model) (get-value (x (+ x 1)))",
    "(set-option :produce-unsat-cores true) (declare-const x Int) (assert (! (> x 0) :named positive)) (assert (! (< x 0) :named negative)) (check-sat) (get-unsat-core)",
    "(set-option :produce-models true) (declare-datatypes ((State 0)) (((idle) (busy)))) (declare-const state State) (assert (= state busy)) (check-sat) (get-value (state))",
    "(declare-const x Int) (assert (> x 0)) (check-sat) (push 1) (assert (< x 0)) (check-sat) (pop 1) (check-sat)",
    "(set-logic QF_UF) (set-option :produce-models true) (declare-const p Bool) (declare-const q Bool) (assert (or p q)) (check-sat-assuming ((not p))) (get-value (p q)) (check-sat-assuming ((not p) (not q))) (check-sat-assuming (p)) (get-value (p))",
    "(set-logic QF_UFLIA) (set-option :produce-models true) (declare-fun f (Int) Int) (declare-const a Int) (declare-const b Int) (assert (= (f a) (+ b 1))) (push 1) (assert (= a b)) (assert (< (f b) b)) (assert (> (f a) (+ b 1))) (check-sat) (pop 1) (push 2) (assert (> (f a) 10)) (check-sat) (get-value (a b (f a))) (pop 2) (check-sat) (get-model)",
    "(set-logic QF_BV) (set-option :produce-models true) (declare-const x (_ BitVec 32)) (declare-const y (_ BitVec 32)) (assert (= (bvadd x y) #x00000010)) (assert (bvult x y)) (check-sat) (get-value (x y)) (push 1) (assert (= x y)) (check-sat) (pop 1)",
    "(set-logic ALL) (set-option :produce-unsat-cores true) (declare-const x Int) (declare-const g1 Bool) (declare-const g2 Bool) (assert (=> g1 (> x 5))) (assert (=> g2 (< x 3))) (check-sat-assuming (g1 g2)) (get-unsat-core) (check-sat-assuming (g1)) (check-sat)",
];

#[test]
#[ignore = "needs SAIL_TEST_CVC5"]
fn actual_pinned_cvc5_is_deterministic_cached_exactly_and_bounded() -> TestResult {
    let directory = Directory::new()?;
    let solver = pinned_cvc5()?;
    let runtime = Runtime::new(Config {
        cvc5: Some(solver.clone()),
        cache_directory: Some(directory.0.join("cache")),
        workers: 3,
        ..Config::default()
    })?;
    let capabilities = runtime.capabilities();
    assert_eq!(capabilities.backends[0].backend, "z3");
    assert_eq!(capabilities.backends[1].backend, "cvc5");
    assert!(
        capabilities.backends[1].installed,
        "{:?}",
        capabilities.backends[1]
    );
    let queries = CVC5_CASES
        .iter()
        .map(|script| {
            let mut item = query(script);
            item.backend = "cvc5".into();
            item.limits.output_bytes = 1024 * 1024;
            item
        })
        .collect::<Vec<_>>();
    let run = |runtime: &Runtime| -> Result<_, Box<dyn std::error::Error>> {
        let Output::Smt { results } = runtime
            .execute(&request(queries.clone()), &Cancellation::default())?
            .output
        else {
            return Err("expected SMT family".into());
        };
        Ok(results)
    };
    // Three fresh executions (separate uncached runtimes) must agree byte for byte.
    let mut fresh = Vec::new();
    for _ in 0..3 {
        let uncached = Runtime::new(Config {
            cvc5: Some(solver.clone()),
            workers: 3,
            ..Config::default()
        })?;
        let results = run(&uncached)?;
        for result in &results {
            assert_eq!(result.outcome, Outcome::Completed, "{result:?}");
            assert!(!result.cache_hit);
            assert_eq!(
                result.solver.as_ref().map(|s| s.backend.as_str()),
                Some("cvc5")
            );
        }
        fresh.push(results.into_iter().map(|r| r.stdout).collect::<Vec<_>>());
    }
    assert_eq!(fresh[0], fresh[1]);
    assert_eq!(fresh[0], fresh[2]);
    // Exact-bytes caching: first run misses, second hits with the same bytes.
    let first = run(&runtime)?;
    let second = run(&runtime)?;
    for ((first, second), expected) in first.iter().zip(&second).zip(&fresh[0]) {
        assert!(!first.cache_hit);
        assert!(second.cache_hit);
        assert_eq!(&first.stdout, expected);
        assert_eq!(&second.stdout, expected);
        assert_eq!(first.checks, second.checks);
    }
    let answers = |index: usize| {
        first[index]
            .checks
            .iter()
            .map(|check| check.answer)
            .collect::<Vec<_>>()
    };
    assert_eq!(answers(1), [Answer::Unsat]);
    assert_eq!(answers(3), [Answer::Sat, Answer::Unsat, Answer::Sat]);
    assert_eq!(answers(4), [Answer::Sat, Answer::Unsat, Answer::Sat]);
    assert_eq!(answers(5), [Answer::Unsat, Answer::Sat, Answer::Sat]);
    assert_eq!(answers(6), [Answer::Sat, Answer::Unsat]);
    assert_eq!(answers(7), [Answer::Unsat, Answer::Sat, Answer::Sat]);
    for (index, result) in first.iter().enumerate() {
        println!(
            "cvc5 case {index}: {}",
            String::from_utf8_lossy(&result.stdout).replace('\n', " ")
        );
    }
    // A changed byte is a miss.
    let mut changed = queries[3].clone();
    changed.script.push_str("\n; different exact bytes\n");
    let Output::Smt { results } = runtime
        .execute(&request(vec![changed]), &Cancellation::default())?
        .output
    else {
        return Err("expected SMT family".into());
    };
    assert!(!results[0].cache_hit);
    Ok(())
}

/// A pigeonhole instance cvc5 cannot finish in a small resource budget.
fn pigeonhole(pigeons: usize, holes: usize) -> String {
    use std::fmt::Write as _;
    let mut script = String::from("(set-logic QF_UFLIA)\n(declare-fun p (Int Int) Bool)\n");
    for i in 0..pigeons {
        script.push_str("(assert (or");
        for h in 0..holes {
            let _ = write!(script, " (p {i} {h})");
        }
        script.push_str("))\n");
    }
    for h in 0..holes {
        for i in 0..pigeons {
            for j in i + 1..pigeons {
                let _ = writeln!(script, "(assert (or (not (p {i} {h})) (not (p {j} {h}))))");
            }
        }
    }
    script
}

#[test]
#[ignore = "needs SAIL_TEST_CVC5"]
fn actual_cvc5_resource_limit_is_deterministic_explicit_and_never_cached() -> TestResult {
    let directory = Directory::new()?;
    let runtime = Runtime::new(Config {
        cvc5: Some(pinned_cvc5()?),
        cache_directory: Some(directory.0.join("cache")),
        ..Config::default()
    })?;
    let execute = |script: String, limit: u64| -> Result<_, Box<dyn std::error::Error>> {
        let mut item = query(&script);
        item.backend = "cvc5".into();
        item.limits.resource_limit = limit;
        let Output::Smt { results } = runtime
            .execute(&request(vec![item]), &Cancellation::default())?
            .output
        else {
            return Err("expected SMT family".into());
        };
        results
            .into_iter()
            .next()
            .ok_or_else(|| "missing result".into())
    };
    let hard = format!(
        "{}(check-sat) (get-info :reason-unknown) (check-sat)\n",
        pigeonhole(9, 8)
    );
    let mut seen = Vec::new();
    for _ in 0..3 {
        let result = execute(hard.clone(), 1000)?;
        assert_eq!(result.outcome, Outcome::Completed, "{result:?}");
        assert_eq!(
            result
                .checks
                .iter()
                .map(|check| check.answer)
                .collect::<Vec<_>>(),
            [Answer::Unknown, Answer::Unknown]
        );
        assert_eq!(
            result.checks[0].products[0].raw,
            b"(:reason-unknown resourceout)"
        );
        assert!(!result.cache_hit);
        seen.push(result.stdout);
    }
    assert_eq!(seen[0], seen[1]);
    assert_eq!(seen[0], seen[2]);
    // Bisect the smallest sufficient budget three times: the boundary itself
    // must be identical for the limit to be a reproducible cache-key input.
    let solved = format!("{}(check-sat)\n", pigeonhole(7, 6));
    let mut boundaries = Vec::new();
    for round in 0..3 {
        let (mut low, mut high) = (1_u64, 50_000_000_u64);
        while low < high {
            let middle = low + (high - low) / 2;
            // A distinct trailing comment per probe defeats the cache.
            let result = execute(format!("{solved}; round {round} limit {middle}\n"), middle)?;
            assert_eq!(result.outcome, Outcome::Completed, "{result:?}");
            assert!(!result.cache_hit);
            match result.checks[0].answer {
                Answer::Unsat => high = middle,
                Answer::Unknown => low = middle + 1,
                Answer::Sat => return Err("pigeonhole cannot be sat".into()),
            }
        }
        boundaries.push(low);
    }
    println!("cvc5 rlimit-per boundary for pigeonhole(7,6): {boundaries:?}");
    assert!(boundaries[0] > 1 && boundaries[0] < 50_000_000);
    assert_eq!(boundaries[0], boundaries[1]);
    assert_eq!(boundaries[0], boundaries[2]);
    Ok(())
}

#[test]
#[ignore = "needs SAIL_TEST_CVC5"]
fn actual_cvc5_dialect_errors_are_infrastructure_never_answers() -> TestResult {
    let runtime = Runtime::new(Config {
        cvc5: Some(pinned_cvc5()?),
        ..Config::default()
    })?;
    for script in [
        // Z3's legacy datatype syntax, taken from the Z3 transcript above.
        "(set-option :produce-models true) (declare-datatypes () ((State idle busy))) (declare-const state State) (assert (= state busy)) (check-sat) (get-value (state))",
        // A model without model production: `(error ...)` with exit status 0.
        "(check-sat) (get-model)",
        // An option change after the solver is initialised.
        "(declare-const x Int) (check-sat) (set-option :produce-models true) (check-sat)",
    ] {
        let mut item = query(script);
        item.backend = "cvc5".into();
        let Output::Smt { results } = runtime
            .execute(&request(vec![item]), &Cancellation::default())?
            .output
        else {
            return Err("expected SMT family".into());
        };
        assert_eq!(results[0].outcome, Outcome::Infrastructure, "{script}");
        assert_eq!(results[0].checks, [], "{script}");
    }
    Ok(())
}

#[test]
#[cfg(unix)]
fn changed_cvc5_executable_never_becomes_ready() -> TestResult {
    let directory = Directory::new()?;
    let executable = script(&directory, "echo 'cvc5 1.4.2 [git 282d5e0 on branch HEAD]'")?;
    let runtime = Runtime::new(Config {
        cvc5: Some(executable),
        ..Config::default()
    })?;
    let capabilities = runtime.capabilities();
    assert_eq!(capabilities.backends.len(), 2);
    assert!(!capabilities.backends[1].installed);
    let mut item = query("(check-sat)");
    item.backend = "cvc5".into();
    let Output::Smt { results } = runtime
        .execute(&request(vec![item]), &Cancellation::default())?
        .output
    else {
        return Err("expected SMT family".into());
    };
    assert_eq!(results[0].outcome, Outcome::Unavailable);
    assert_eq!(results[0].checks, []);
    // With no cvc5 configured the capability record lists Z3 only, as before.
    assert_eq!(
        Runtime::new(Config::default())?
            .capabilities()
            .backends
            .len(),
        1
    );
    Ok(())
}
