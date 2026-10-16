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
use sail_conformance::rvt::{classify_tests, classify_tests_exact};
use sail_verus::run::{Solver, copy_tree, machine_info};
use sail_verus::rvt::{Outcome, Suite, TestCase, TestRecord, kept_input_dir};
use sail_verus::source_build;
use sail_verus::toolchain::{Installed, Pins};
use serde::Serialize;

use crate::{now_unix, slug};

pub const USAGE: &str = "usage:
  sail-conformance rust-verify-test [--filter SUBSTR]... [--every N] [--jobs N]
                                    [--confirm-runs N] [--out DIR] [--shim PATH]
                                    [--baseline fork|stock]
                                    [--list]

  Runs the pinned fork's rust_verify_test suite plainly and through sail-z3
  with exact output comparison. Build it with `cargo xtask build-verus-source
  --fork`. With --baseline stock, retain the original stock-vs-shim control
  using ADR 0042 for diagnostics. The sail side uses SAIL_DAEMON_SOCKET if set.

  --filter SUBSTR   only tests whose id (<file>::<test>) contains SUBSTR
                    (repeatable: any of them)
  --every N         then only every N-th test (a spread-out sample)
  --jobs N          tests run in parallel (default 4)
  --confirm-runs N  extra plain runs when sail differs (default 10; ADR 0009)
  --baseline NAME  fork (default): exact fork vs fork through the shim;
                    stock: original stock vs stock through the shim, ADR 0042
  --verus-threads N Verus's --num-threads for every test, both sides
                    (default 0: Verus's own default; messages from different
                    modules may come in any order, ADR 0042. 1 gives a fixed
                    order but fewer errors on three multi-module tests)
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
    stock_baseline: bool,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options {
        filters: Vec::new(),
        every: 1,
        jobs: 4,
        confirm_runs: 10,
        verus_threads: 0,
        out: None,
        shim: None,
        list: false,
        stock_baseline: false,
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
            "--baseline" => {
                o.stock_baseline = match value()?.as_str() {
                    "fork" => false,
                    "stock" => true,
                    _ => return Err("--baseline must be fork or stock".to_owned()),
                };
            }
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
    /// Present only when this arm runs the pinned fork runtime.
    fork_commit: Option<String>,
    comparison: &'static str,
    baseline_build: PathBuf,
    candidate_build: PathBuf,
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
    exact: bool,
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
        Solver::Plain | Solver::Engine(_) => String::new(),
    };
    let note = format!(
        "test: {id}\n\
         input: {input}\n\n\
         Verus's own rerun line (from the source build's `source` directory):\n{rerun}\n\n\
         Rerun the test itself, in {pkg}:\n\
         plain: VERUS_IN_VARGO=1 RUSTC_BOOTSTRAP=1 RUST_MIN_STACK=20971520 VERUS_EXTRA_ARGS=\"{extra}\" VERUS_Z3_PATH={z3} {bin} --exact {name} --nocapture --test-threads 1\n\
         sail: VERUS_IN_VARGO=1 RUSTC_BOOTSTRAP=1 RUST_MIN_STACK=20971520 VERUS_EXTRA_ARGS=\"{extra}\" VERUS_Z3_PATH={sail_z3} SAIL_Z3_PATH={z3} {bin} --exact {name} --nocapture --test-threads 1\n\
         (with Verus's toolchain library directory on the dynamic library path)\n",
        id = test.id(),
        rerun = rerun.join("\n"),
        pkg = cx.suite.package_dir().display(),
        z3 = cx.suite.z3.display(),
        bin = test.path.display(),
        name = test.name,
        extra = cx.suite.extra_verus_args(test),
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
    let classify = if cx.exact {
        classify_tests_exact
    } else {
        classify_tests
    };
    let mut c = classify(&plains, &sail);
    for _ in 0..cx.confirm_runs {
        if c.verdict != Verdict::Different {
            break;
        }
        match cx.suite.run(test, &Solver::Plain, false) {
            Ok(p) => plains.push(p),
            Err(e) => return infra(test, e.to_string()),
        }
        c = classify(&plains, &sail);
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
            // `rust_verify_test` has no engine arm, so no test lacks a feature.
            Verdict::InfraError | Verdict::EngineLacksFeature => s.infra_error += 1,
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
        // Each test changes directory into the source suite. Resolve an
        // explicit relative path while it still names the caller's file.
        shim.canonicalize().map_err(|e| {
            format!(
                "error[SAIL3030]: cannot resolve shim {}: {e}",
                shim.display()
            )
        })
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

fn open_suite(
    installed: &Installed,
    threads: u32,
    fork: bool,
) -> Result<(Suite, std::fs::File), String> {
    let verus = &installed.pins.verus;
    let cache = sail_verus::cache_dir().map_err(|e| e.to_string())?;
    let features = if fork {
        source_build::fork_test_features()
    } else {
        Vec::new()
    };
    let default =
        source_build::variant_dir(&cache, &verus.commit, &verus.rust_toolchain, &features);
    let selected = std::env::var_os("SAIL_VERUS_SOURCE_BUILD");
    let stock_override = !fork && selected.is_some();
    let build = selected.map_or(default, PathBuf::from);
    let ownership = source_build::lock_build(&build).map_err(|e| e.to_string())?;
    let mut suite =
        Suite::open(&build, &verus.rust_toolchain, &installed.z3).map_err(|e| e.to_string())?;
    let z3 = installed
        .pins
        .assets
        .get(sail_verus::toolchain::host())
        .ok_or_else(|| "no solver pin for this host".to_owned())?;
    // Installed::locate already validates a stock override. Keep its legacy
    // ordinary/Singular suite selection, including the override's features;
    // the default stock directory and every fork selector stay exact variants.
    let features_match = if stock_override {
        suite
            .manifest
            .features
            .iter()
            .all(|feature| feature == "singular")
    } else {
        suite.manifest.features == features
    };
    if suite.manifest.commit != verus.commit
        || !features_match
        || suite.manifest.z3_sha256 != z3.z3_sha256
    {
        return Err(format!(
            "error[SAIL3040]: {} does not match the requested {} source build; rebuild with cargo xtask build-verus-source{}",
            build.display(),
            if fork { "fork" } else { "stock" },
            if fork { " --fork" } else { "" }
        ));
    }
    suite.verus_threads = (threads > 0).then_some(threads);
    Ok((suite, ownership))
}

/// Runs the command; exit 0 if every test is the same (or unstable), 1 on
/// any difference, 2 on infrastructure errors.
#[allow(
    clippy::too_many_lines,
    reason = "Owns the selected source build through test execution and report publication."
)]
pub fn main(args: &[String]) -> Result<ExitCode, String> {
    let o = parse(args)?;
    let pins = Pins::load(Path::new("corpus/toolchain.toml")).map_err(|e| e.to_string())?;
    // The fork selector names a suite variant, not an ordinary stock source
    // installation. Validate it in open_suite; obtain its pinned solver from
    // the release without applying Installed::locate's stock-only override.
    let installed = if o.stock_baseline {
        Installed::locate(pins)
    } else {
        Installed::locate_release(pins)
    }
    .map_err(|e| e.to_string())?;
    let verus = &installed.pins.verus;
    let fork = sail_verus::engine::fork().map_err(|e| e.to_string())?;
    if !o.stock_baseline && fork.upstream != verus.commit {
        return Err("the fork's upstream base does not match the selected Verus pin".to_owned());
    }
    let (suite, _ownership) = open_suite(&installed, o.verus_threads, !o.stock_baseline)?;
    let cache = sail_verus::cache_dir().map_err(|e| e.to_string())?;
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
        "rust_verify_test {}: {} tests ({} ignored) with {} jobs; sail side {}; output in {}",
        if o.stock_baseline {
            format!("stock {} (ADR 0042)", verus.commit)
        } else {
            format!("fork {} (exact)", fork.commit)
        },
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
        exact: !o.stock_baseline,
        sail: Solver::Shim {
            shim,
            trace_dir: None,
            daemon_socket: daemon_socket.clone(),
            observation_logs: false,
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
        fork_commit: (!o.stock_baseline).then_some(fork.commit),
        comparison: if o.stock_baseline {
            "stock-vs-shim"
        } else {
            "fork-vs-shim"
        },
        baseline_build: suite.build.clone(),
        candidate_build: suite.build.clone(),
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

#[cfg(test)]
mod tests {
    use super::{find_shim, parse};

    #[test]
    fn explicit_relative_shim_is_resolved_before_suite_changes_directory() {
        // Only path resolution is under test; no solver is launched.
        let relative = std::path::Path::new("Cargo.toml");
        let resolved = find_shim(Some(relative)).unwrap_or_else(|e| unreachable!("{e}"));
        assert!(resolved.is_absolute());
        assert_eq!(relative.canonicalize().ok().as_ref(), Some(&resolved));
    }

    #[test]
    fn baseline_is_fork_unless_stock_is_explicitly_requested() {
        assert!(
            !parse(&[])
                .unwrap_or_else(|e| unreachable!("{e}"))
                .stock_baseline
        );
        let stock = ["--baseline".to_owned(), "stock".to_owned()];
        assert!(
            parse(&stock)
                .unwrap_or_else(|e| unreachable!("{e}"))
                .stock_baseline
        );
        assert!(parse(&["--baseline".to_owned(), "other".to_owned()]).is_err());
    }
}
