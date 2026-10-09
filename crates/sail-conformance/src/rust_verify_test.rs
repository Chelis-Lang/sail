//! `sail-conformance rust-verify-test`: plain Verus against sail on Verus's
//! own `rust_verify_test` suite, from the source build made by `cargo xtask
//! build-verus-source` (ADR 0019).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use sail_conformance::Verdict;
use sail_conformance::rvt::classify_tests;
use sail_verus::run::{Solver, copy_tree, machine_info};
use sail_verus::rvt::{Outcome, Suite, TestCase, TestRecord, kept_input_dir};
use sail_verus::source_build;
use sail_verus::toolchain::{Installed, Pins};
use serde::Serialize;

use crate::{now_unix, slug};

pub const USAGE: &str = "usage:
  sail-conformance rust-verify-test [--filter SUBSTR]... [--every N] [--jobs N]
                                    [--confirm-runs N] [--out DIR] [--shim PATH]
                                    [--list]

  Runs every test of Verus's rust_verify_test suite (from `cargo xtask
  build-verus-source`) plainly and through sail-z3, and compares outcomes,
  exit codes and diagnostics. The sail side uses SAIL_DAEMON_SOCKET if set.

  --filter SUBSTR   only tests whose id (<file>::<test>) contains SUBSTR
                    (repeatable: any of them)
  --every N         then only every N-th test (a spread-out sample)
  --jobs N          tests run in parallel (default 4)
  --confirm-runs N  extra plain runs when sail differs (default 10; ADR 0009)
  --verus-threads N Verus's --num-threads for every test, both sides
                    (default 1: deterministic diagnostic order, VI-0007;
                    0 keeps Verus's default of all cores)
  --list            print the selected test ids and exit";

struct Options {
    filters: Vec<String>,
    every: usize,
    jobs: usize,
    confirm_runs: usize,
    verus_threads: u32,
    out: Option<PathBuf>,
    shim: Option<PathBuf>,
    list: bool,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options {
        filters: Vec::new(),
        every: 1,
        jobs: 4,
        confirm_runs: 10,
        verus_threads: 1,
        out: None,
        shim: None,
        list: false,
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = || {
            it.next()
                .cloned()
                .ok_or(format!("{arg} needs a value\n{USAGE}"))
        };
        let number = |v: String| {
            v.parse::<usize>()
                .map_err(|_| format!("{arg} needs a number"))
        };
        match arg.as_str() {
            "--filter" => o.filters.push(value()?),
            "--every" => o.every = number(value()?)?.max(1),
            "--jobs" => o.jobs = number(value()?)?.max(1),
            "--confirm-runs" => o.confirm_runs = number(value()?)?,
            "--verus-threads" => {
                o.verus_threads = u32::try_from(number(value()?)?)
                    .map_err(|_| "--verus-threads is too large".to_owned())?;
            }
            "--out" => o.out = Some(value()?.into()),
            "--shim" => o.shim = Some(value()?.into()),
            "--list" => o.list = true,
            _ => return Err(format!("unknown argument `{arg}`\n{USAGE}")),
        }
    }
    Ok(o)
}

#[derive(Serialize)]
struct TestReport {
    id: String,
    verdict: Verdict,
    plain: Option<Outcome>,
    sail: Option<Outcome>,
    plain_runs: usize,
    plain_varies: bool,
    /// Whether sail matched a complete plain run after approved normalization.
    matches_a_plain_run: bool,
    differences: Vec<String>,
    infra_error: Option<String>,
    plain_ms: f64,
    sail_ms: f64,
    /// Where the outputs (and, for a difference, the reproduction) are.
    artifacts: Option<PathBuf>,
}

#[derive(Serialize, Default)]
struct Summary {
    tests: usize,
    ignored: usize,
    same: usize,
    unstable: usize,
    different: usize,
    infra_error: usize,
    plain_ok: usize,
    plain_failed: usize,
    plain_unknown: usize,
    plain_ms: f64,
    sail_ms: f64,
    wall_s: f64,
}

#[derive(Serialize)]
struct Report {
    schema_version: u32,
    suite: &'static str,
    verus_release: String,
    verus_commit: String,
    z3_version: String,
    features: Vec<String>,
    sail_daemon: bool,
    /// Verus's `--num-threads` for every test (`None`: Verus's default).
    verus_threads: Option<u32>,
    machine: std::collections::BTreeMap<String, String>,
    started_unix: u64,
    selection: Vec<String>,
    summary: Summary,
    tests: Vec<TestReport>,
}

struct Context<'a> {
    suite: &'a Suite,
    sail: Solver,
    out: &'a Path,
    confirm_runs: usize,
}

fn infra(test: &TestCase, message: String) -> TestReport {
    TestReport {
        id: test.id(),
        verdict: Verdict::InfraError,
        plain: None,
        sail: None,
        plain_runs: 0,
        plain_varies: false,
        matches_a_plain_run: false,
        differences: Vec::new(),
        infra_error: Some(message),
        plain_ms: 0.0,
        sail_ms: 0.0,
        artifacts: None,
    }
}

fn save(dir: &Path, name: &str, r: &TestRecord) {
    let _ = std::fs::write(dir.join(format!("{name}.stdout")), &r.stdout);
    let _ = std::fs::write(dir.join(format!("{name}.stderr")), &r.stderr);
}

/// Exports the test's input (`VERUS_KEEP_TEST_DIR`) and how to rerun it.
fn reproduce(cx: &Context, test: &TestCase, dir: &Path) -> Result<(), String> {
    let kept = cx
        .suite
        .run(test, &Solver::Plain, true)
        .map_err(|e| e.to_string())?;
    // Tests that drive `cargo verus` on `tests/cargo-tests` materialize no
    // input; their sources are in the checkout.
    let input = kept_input_dir(&kept.stderr);
    if let Some(input) = &input {
        copy_tree(input, &dir.join("input")).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir_all(input);
    }
    let input = input.map_or_else(
        || {
            "none: this test materializes no input (see its source in rust_verify_test/tests)"
                .to_owned()
        },
        |p| {
            format!(
                "input/ (exported with VERUS_KEEP_TEST_DIR=1 from {})",
                p.display()
            )
        },
    );
    let rerun: Vec<&str> = kept
        .stderr
        .lines()
        .skip_while(|l| !l.contains("rerun this test with"))
        .take(2)
        .collect();
    let sail_z3 = match &cx.sail {
        Solver::Shim { shim, .. } => shim.display().to_string(),
        Solver::Plain => String::new(),
    };
    let note = format!(
        "test: {id}\n\
         input: {input}\n\n\
         Verus's own rerun line (from the source build's `source` directory):\n{rerun}\n\n\
         Rerun the test itself, in {pkg}:\n\
         plain: VERUS_IN_VARGO=1 RUSTC_BOOTSTRAP=1 RUST_MIN_STACK=20971520 {extra}VERUS_Z3_PATH={z3} {bin} --exact {name} --nocapture --test-threads 1\n\
         sail:  VERUS_IN_VARGO=1 RUSTC_BOOTSTRAP=1 RUST_MIN_STACK=20971520 {extra}VERUS_Z3_PATH={sail_z3} SAIL_Z3_PATH={z3} {bin} --exact {name} --nocapture --test-threads 1\n\
         (with Verus's toolchain library directory on the dynamic library path)\n",
        id = test.id(),
        rerun = rerun.join("\n"),
        pkg = cx.suite.package_dir().display(),
        z3 = cx.suite.z3.display(),
        bin = test.path.display(),
        name = test.name,
        extra = cx.suite.verus_threads.map_or_else(String::new, |n| format!(
            "VERUS_EXTRA_ARGS=\"--num-threads {n}\" "
        )),
    );
    std::fs::write(dir.join("reproduce.txt"), note).map_err(|e| e.to_string())
}

fn run_one(cx: &Context, test: &TestCase) -> TestReport {
    if test.ignored {
        return TestReport {
            id: test.id(),
            verdict: Verdict::Same,
            plain: Some(Outcome::Ignored),
            sail: Some(Outcome::Ignored),
            plain_runs: 0,
            plain_varies: false,
            matches_a_plain_run: false,
            differences: Vec::new(),
            infra_error: None,
            plain_ms: 0.0,
            sail_ms: 0.0,
            artifacts: None,
        };
    }
    let mut plains = Vec::new();
    match cx.suite.run(test, &Solver::Plain, false) {
        Ok(p) => plains.push(p),
        Err(e) => return infra(test, e.to_string()),
    }
    let sail = match cx.suite.run(test, &cx.sail, false) {
        Ok(s) => s,
        Err(e) => return infra(test, e.to_string()),
    };
    let mut c = classify_tests(&plains, &sail);
    for _ in 0..cx.confirm_runs {
        if c.verdict != Verdict::Different {
            break;
        }
        match cx.suite.run(test, &Solver::Plain, false) {
            Ok(p) => plains.push(p),
            Err(e) => return infra(test, e.to_string()),
        }
        c = classify_tests(&plains, &sail);
    }
    let mut artifacts = None;
    if c.verdict != Verdict::Same {
        let dir = cx.out.join("tests").join(slug(&test.id()));
        if std::fs::create_dir_all(&dir).is_ok() {
            for (i, p) in plains.iter().enumerate() {
                save(&dir, &format!("plain-{}", i + 1), p);
            }
            save(&dir, "sail", &sail);
            if c.verdict == Verdict::Different
                && let Err(e) = reproduce(cx, test, &dir)
            {
                let _ = std::fs::write(dir.join("reproduce-error.txt"), e);
            }
            artifacts = Some(dir);
        }
    }
    let first = plains.first();
    TestReport {
        id: test.id(),
        verdict: c.verdict,
        plain: first.map(|p| p.outcome),
        sail: Some(sail.outcome),
        plain_runs: c.plain_runs,
        plain_varies: c.plain_varies,
        matches_a_plain_run: c.matches_a_plain_run,
        differences: c.differences,
        infra_error: None,
        plain_ms: first.map_or(0.0, |p| p.wall_ms),
        sail_ms: sail.wall_ms,
        artifacts,
    }
}

fn run_all(tests: Vec<TestCase>, jobs: usize, cx: &Context) -> Vec<TestReport> {
    let total = tests.len();
    let queue = Mutex::new(tests.into_iter().collect::<VecDeque<_>>());
    let results = Mutex::new(Vec::new());
    let done = AtomicUsize::new(0);
    let not_same = AtomicUsize::new(0);
    let started = Instant::now();
    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| {
                loop {
                    let next = queue.lock().ok().and_then(|mut q| q.pop_front());
                    let Some(test) = next else { break };
                    let report = run_one(cx, &test);
                    let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if report.verdict != Verdict::Same {
                        not_same.fetch_add(1, Ordering::Relaxed);
                        eprintln!(
                            "{:<11} {}{}",
                            format!("{:?}", report.verdict).to_lowercase(),
                            report.id,
                            report
                                .differences
                                .first()
                                .or(report.infra_error.as_ref())
                                .map(|d| format!("\n            {d}"))
                                .unwrap_or_default()
                        );
                    }
                    if n.is_multiple_of(100) || n == total {
                        eprintln!(
                            "[{n}/{total}] {:.0}s, {} not same",
                            started.elapsed().as_secs_f64(),
                            not_same.load(Ordering::Relaxed)
                        );
                    }
                    if let Ok(mut r) = results.lock() {
                        r.push(report);
                    }
                }
            });
        }
    });
    results.into_inner().unwrap_or_default()
}

fn summarize(tests: &[TestReport], wall_s: f64) -> Summary {
    let mut s = Summary {
        tests: tests.len(),
        wall_s,
        ..Summary::default()
    };
    for t in tests {
        match t.verdict {
            Verdict::Same => s.same += 1,
            Verdict::Unstable => s.unstable += 1,
            Verdict::Different => s.different += 1,
            Verdict::InfraError => s.infra_error += 1,
        }
        match t.plain {
            Some(Outcome::Ignored) => s.ignored += 1,
            Some(Outcome::Ok) => s.plain_ok += 1,
            Some(Outcome::Failed) => s.plain_failed += 1,
            Some(Outcome::Unknown) => s.plain_unknown += 1,
            None => {}
        }
        s.plain_ms += t.plain_ms;
        s.sail_ms += t.sail_ms;
    }
    s
}

/// `--shim`, or `sail-z3` next to this binary.
fn find_shim(given: Option<&Path>) -> Result<PathBuf, String> {
    let shim = match given {
        Some(p) => p.to_path_buf(),
        None => std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("sail-z3"),
    };
    if shim.is_file() {
        Ok(shim)
    } else {
        Err(format!(
            "error[SAIL3030]: shim not found at {} (build it with `cargo build --release -p sail-shim`, or pass --shim)",
            shim.display()
        ))
    }
}

/// The tests `o` selects, in suite order.
fn select(o: &Options, suite: &Suite) -> Result<Vec<TestCase>, String> {
    let mut tests = suite.list().map_err(|e| e.to_string())?;
    if !o.filters.is_empty() {
        tests.retain(|t| {
            let id = t.id();
            o.filters.iter().any(|f| id.contains(f.as_str()))
        });
    }
    Ok(tests.into_iter().step_by(o.every).collect())
}

/// Runs the command; exit 0 if every test is the same (or unstable), 1 on
/// any difference, 2 on infrastructure errors.
pub fn main(args: &[String]) -> Result<ExitCode, String> {
    let o = parse(args)?;
    let pins = Pins::load(Path::new("corpus/toolchain.toml")).map_err(|e| e.to_string())?;
    let installed = Installed::locate(pins).map_err(|e| e.to_string())?;
    let verus = &installed.pins.verus;
    let cache = sail_verus::cache_dir().map_err(|e| e.to_string())?;
    let build = source_build::build_dir(&cache, &verus.commit);
    let mut suite =
        Suite::open(&build, &verus.rust_toolchain, &installed.z3).map_err(|e| e.to_string())?;
    suite.verus_threads = (o.verus_threads > 0).then_some(o.verus_threads);
    let tests = select(&o, &suite)?;
    if o.list {
        for t in &tests {
            println!("{}{}", t.id(), if t.ignored { " (ignored)" } else { "" });
        }
        return Ok(ExitCode::SUCCESS);
    }
    let shim = find_shim(o.shim.as_deref())?;
    let daemon_socket = std::env::var_os("SAIL_DAEMON_SOCKET")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    let started = now_unix();
    let out = o.out.clone().unwrap_or_else(|| {
        cache
            .join("conformance")
            .join(format!("rust-verify-test-{started}"))
    });
    std::fs::create_dir_all(&out).map_err(|e| format!("cannot create {}: {e}", out.display()))?;
    eprintln!(
        "rust_verify_test @ {}: {} tests ({} ignored) with {} jobs; sail side {}; output in {}",
        verus.commit,
        tests.len(),
        tests.iter().filter(|t| t.ignored).count(),
        o.jobs,
        if daemon_socket.is_some() {
            "with the cache daemon"
        } else {
            "pass-through"
        },
        out.display()
    );
    let cx = Context {
        suite: &suite,
        sail: Solver::Shim {
            shim,
            trace_dir: None,
            daemon_socket: daemon_socket.clone(),
        },
        out: &out,
        confirm_runs: o.confirm_runs,
    };
    let clock = Instant::now();
    suite.clear_inputs();
    let mut results = run_all(tests, o.jobs, &cx);
    suite.clear_inputs();
    results.sort_by(|a, b| a.id.cmp(&b.id));
    let summary = summarize(&results, clock.elapsed().as_secs_f64());
    let mut selection: Vec<String> = o.filters.iter().map(|f| format!("filter:{f}")).collect();
    if o.every > 1 {
        selection.push(format!("every:{}", o.every));
    }
    let report = Report {
        schema_version: 1,
        suite: "rust_verify_test",
        verus_release: verus.release.clone(),
        verus_commit: verus.commit.clone(),
        z3_version: installed.pins.z3.version.clone(),
        features: suite.manifest.features.clone(),
        sail_daemon: daemon_socket.is_some(),
        verus_threads: suite.verus_threads,
        machine: machine_info(),
        started_unix: started,
        selection,
        summary,
        tests: results,
    };
    let path = out.join("report.json");
    let json = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    let s = &report.summary;
    println!(
        "rust_verify_test conformance: {} tests ({} ignored), {} same, {} unstable, {} different, {} infra errors; plain outcomes {} ok, {} FAILED, {} unknown; {:.0}s\nreport: {}",
        s.tests,
        s.ignored,
        s.same,
        s.unstable,
        s.different,
        s.infra_error,
        s.plain_ok,
        s.plain_failed,
        s.plain_unknown,
        s.wall_s,
        path.display()
    );
    Ok(if s.different > 0 {
        ExitCode::from(1)
    } else if s.infra_error > 0 {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    })
}
