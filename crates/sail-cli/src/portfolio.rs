//! Sequential, independently verified, complete-run seed portfolios.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sail_api::output::{
    PortfolioAttempt, PortfolioReport, Report, SolverConfiguration, VerdictName,
};

/// An explicitly requested complete-run portfolio.
pub(crate) struct Options {
    seeds: Vec<u32>,
    timeout: Duration,
    json: bool,
    args: Vec<String>,
}

/// Parses only sail's portfolio flags before the Verus argument separator.
pub(crate) fn parse(args: &[String]) -> Result<Option<Options>, String> {
    let mut enabled = false;
    let mut seeds = (1..8).collect::<Vec<u32>>();
    let mut timeout = 3600;
    let mut customized = false;
    let mut json = false;
    let mut rest = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        if arg == "--" {
            rest.push(arg.clone());
            rest.extend(it.cloned());
            break;
        }
        let (key, inline) = arg
            .split_once('=')
            .map_or((arg.as_str(), None), |(k, v)| (k, Some(v)));
        let mut value = || {
            inline
                .map(str::to_owned)
                .or_else(|| it.next().cloned())
                .ok_or_else(|| format!("{key} requires a value"))
        };
        match key {
            "--portfolio" => {
                if value()? != "accept-first-proof" {
                    return Err("only --portfolio accept-first-proof is supported".to_owned());
                }
                enabled = true;
            }
            "--portfolio-seeds" => {
                customized = true;
                seeds = value()?
                    .split(',')
                    .map(|s| {
                        s.parse::<u32>()
                            .map_err(|_| "portfolio seeds must be positive u32 integers".to_owned())
                    })
                    .collect::<Result<_, _>>()?;
                let distinct: std::collections::BTreeSet<_> = seeds.iter().collect();
                if seeds.is_empty() || seeds.contains(&0) || distinct.len() != seeds.len() {
                    return Err(
                        "portfolio seeds must be distinct and positive; baseline is always first"
                            .to_owned(),
                    );
                }
            }
            "--verus-source" => {
                customized = true;
                rest.extend(["--verus-source".to_owned(), value()?]);
            }
            "--portfolio-timeout-secs" => {
                customized = true;
                timeout = value()?
                    .parse()
                    .map_err(|_| "invalid portfolio timeout".to_owned())?;
                if timeout == 0 {
                    return Err("portfolio timeout must be positive".to_owned());
                }
            }
            "--output" => {
                let v = value()?;
                json = v == "json";
                rest.extend([key.to_owned(), v]);
            }
            _ => rest.push(arg.clone()),
        }
    }
    if !enabled {
        return if customized {
            Err("portfolio options require --portfolio accept-first-proof".to_owned())
        } else {
            Ok(None)
        };
    }
    validate(&rest)?;
    // The worker always produces JSON; the parent chooses the user format.
    let mut clean = Vec::new();
    let mut it = rest.into_iter();
    while let Some(arg) = it.next() {
        if arg == "--" {
            clean.push(arg);
            clean.extend(it);
            break;
        }
        if arg == "--output" {
            if it.next().as_deref() != Some("json") {
                return Err("only --output json is supported".to_owned());
            }
        } else {
            clean.push(arg);
        }
    }
    seeds.insert(0, 0);
    Ok(Some(Options {
        seeds,
        timeout: Duration::from_secs(timeout),
        json,
        args: clean,
    }))
}

fn validate(rest: &[String]) -> Result<(), String> {
    if rest.iter().any(|s| {
        s == "--target-dir"
            || s.starts_with("--target-dir=")
            || s == "--no-verify"
            || s.starts_with("--no-verify=")
            || s.starts_with("smt.random_seed=")
            || s.starts_with("sat.random_seed=")
            || s.contains("=smt.random_seed=")
            || s.contains("=sat.random_seed=")
    }) {
        return Err(
            "portfolio controls seeds and target directory, and requires verification".to_owned(),
        );
    }
    Ok(())
}

/// Whether the requested Verus scope requires cargo-verus focus.
pub(crate) fn partial(args: &[String]) -> bool {
    args.iter().skip_while(|s| *s != "--").any(|s| {
        [
            "--verify-module",
            "--verify-only-module",
            "--verify-function",
            "--verify-root",
        ]
        .iter()
        .any(|flag| s == flag || s.starts_with(&format!("{flag}=")))
    })
}

/// Only a complete, successful verification can win a portfolio.
fn complete(report: &Report) -> bool {
    report.exit_code == Some(0)
        && report.error.is_none()
        && !report.crates.is_empty()
        && report.invariant_violations().is_empty()
        && report.crates.iter().all(|c| {
            c.verus.errors == Some(0)
                && c.verus.success != Some(false)
                && !c.functions.is_empty()
                && c.functions
                    .iter()
                    .all(|f| f.verdict == VerdictName::Verified)
        })
        && report
            .crates
            .iter()
            .any(|c| c.verus.verified.is_some_and(|n| n > 0))
}

fn manifest_args(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = args.iter().take_while(|s| *s != "--");
    while let Some(arg) = it.next() {
        if arg == "--manifest-path" {
            out.push(arg.clone());
            if let Some(value) = it.next() {
                out.push(value.clone());
            }
        } else if arg.starts_with("--manifest-path=") {
            out.push(arg.clone());
        }
    }
    out
}

/// Cleans only verification-enabled packages, only in this request's target.
pub(crate) fn force_verification(args: &[String], target: &Path) -> Result<Vec<String>, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let manifests = manifest_args(args);
    let output = Command::new(&cargo)
        .args(["metadata", "--format-version", "1"])
        .args(&manifests)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    let required = crate::portfolio_scope::required(&metadata, args)?;
    let members = metadata
        .get("workspace_members")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let packages: Vec<(&str, &str)> = metadata
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter(|p| {
            members.iter().any(|id| p.get("id") == Some(id))
                && p.pointer("/metadata/verus/verify")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
        })
        .filter_map(|p| Some((p.get("id")?.as_str()?, p.get("name")?.as_str()?)))
        .collect();
    if packages.is_empty() {
        return Err("no verification-enabled packages in Cargo metadata".to_owned());
    }
    // cargo-verus focus puts its artifacts in a child target directory.
    let clean_target = if partial(args) {
        target.join("verus-partial")
    } else {
        target.to_path_buf()
    };
    let mut clean = Command::new(cargo);
    clean
        .arg("clean")
        .args(&manifests)
        .arg("--target-dir")
        .arg(clean_target);
    for (package, _) in &packages {
        clean.args(["-p", *package]);
    }
    let output = clean.output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(required)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).into_owned())
    }
}

fn worker_args(args: &[String], seed: u32, target: &Path) -> Vec<String> {
    let split = args.iter().position(|s| s == "--").unwrap_or(args.len());
    let mut out = args[..split].to_vec();
    out.extend([
        "--target-dir".to_owned(),
        target.to_string_lossy().into_owned(),
        "--".to_owned(),
    ]);
    out.extend(args.get(split + 1..).unwrap_or_default().iter().cloned());
    if seed != 0 {
        out.extend([
            "--smt-option".to_owned(),
            format!("smt.random_seed={seed}"),
            "--smt-option".to_owned(),
            format!("sat.random_seed={seed}"),
        ]);
    }
    out
}

#[cfg(unix)]
fn kill_group(pid: u32) {
    let _ = Command::new("kill")
        .args(["-s", "KILL", "--", &format!("-{pid}")])
        .status();
}

/// A worker's stdin is a liveness pipe, not solver input. Parent death or
/// cancellation closes it and kills the whole worker group.
pub(crate) fn guard_parent() {
    #[cfg(unix)]
    std::thread::spawn(|| {
        let mut byte = [0];
        let mut stdin = std::io::stdin().lock();
        loop {
            match stdin.read(&mut byte) {
                Ok(0) => {
                    kill_group(std::process::id());
                    return;
                }
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => {
                    kill_group(std::process::id());
                    return;
                }
            }
        }
    });
}

fn attempt(
    options: &Options,
    seed: u32,
    target: &Path,
    stop: &AtomicBool,
) -> Result<PortfolioAttempt, String> {
    let mut cmd = Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    cmd.arg("_portfolio-attempt")
        .arg(seed.to_string())
        .args(worker_args(&options.args, seed, target))
        .env("CARGO_TARGET_DIR", target)
        .env("SAIL_PORTFOLIO_TARGET", target)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        cmd.process_group(0);
    }
    let started = Instant::now();
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let heartbeat = child.stdin.take();
    let stdout = child.stdout.take().map(|mut s| {
        std::thread::spawn(move || {
            let mut b = Vec::new();
            let _ = s.read_to_end(&mut b);
            b
        })
    });
    let stderr = child.stderr.take().map(|mut s| {
        std::thread::spawn(move || {
            let mut b = Vec::new();
            let _ = s.read_to_end(&mut b);
            b
        })
    });
    let mut error = None;
    let exit_code = loop {
        if stop.load(Ordering::SeqCst) || started.elapsed() >= options.timeout {
            error = Some(
                if stop.load(Ordering::SeqCst) {
                    "cancelled"
                } else {
                    "attempt timed out"
                }
                .to_owned(),
            );
            #[cfg(unix)]
            kill_group(child.id());
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => {
                error = Some(e.to_string());
                #[cfg(unix)]
                kill_group(child.id());
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    drop(heartbeat);
    let stdout = stdout.and_then(|h| h.join().ok()).unwrap_or_default();
    let stderr = stderr.and_then(|h| h.join().ok()).unwrap_or_default();
    let report = serde_json::from_slice::<Report>(&stdout).ok();
    if report.is_none() && error.is_none() {
        error = Some("Verus produced no valid report".to_owned());
    }
    if error.is_none()
        && exit_code == Some(0)
        && report
            .as_ref()
            .is_some_and(|r| r.crates.iter().all(|c| c.functions.is_empty()))
    {
        error = Some("Verus produced no function verification results".to_owned());
    }
    let verified = error.is_none() && exit_code == Some(0) && report.as_ref().is_some_and(complete);
    Ok(PortfolioAttempt {
        configuration: SolverConfiguration { seed },
        verified,
        exit_code,
        wall_ms: started.elapsed().as_secs_f64() * 1000.0,
        error,
        report: report.map(Box::new),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    })
}

fn directory() -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let dir = sail_verus::cache_dir()
        .map_err(|e| e.to_string())?
        .join("portfolio")
        .join(format!("{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Runs the sequential portfolio and renders only the selected run.
pub(crate) fn run(options: Options, out: &mut dyn Write, err: &mut dyn Write) -> ExitCode {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    if let Err(e) = ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst)) {
        let _ = writeln!(
            err,
            "error[SAIL0111]: cannot handle portfolio cancellation: {e}"
        );
        return ExitCode::from(2);
    }
    let dir = match directory() {
        Ok(d) => d,
        Err(e) => {
            let _ = writeln!(err, "error[SAIL0111]: {e}");
            return ExitCode::from(2);
        }
    };
    let mut attempts = Vec::new();
    for &seed in &options.seeds {
        let _ = writeln!(
            err,
            "sail portfolio accept-first-proof: attempting seed {seed}"
        );
        let result = attempt(&options, seed, &dir.join("target"), &stop);
        let attempt = result.unwrap_or_else(|e| PortfolioAttempt {
            configuration: SolverConfiguration { seed },
            verified: false,
            exit_code: None,
            wall_ms: 0.0,
            error: Some(e),
            report: None,
            stderr: String::new(),
        });
        let won = attempt.verified;
        if let Ok(bytes) = serde_json::to_vec_pretty(&attempt)
            && let Err(e) = std::fs::write(dir.join(format!("seed-{seed}.json")), bytes)
        {
            let _ = writeln!(err, "error[SAIL0112]: cannot retain attempt evidence: {e}");
            return ExitCode::from(2);
        }
        attempts.push(attempt);
        if won || stop.load(Ordering::SeqCst) {
            break;
        }
    }
    render(
        options,
        attempts,
        &dir,
        stop.load(Ordering::SeqCst),
        out,
        err,
    )
}

fn render(
    options: Options,
    attempts: Vec<PortfolioAttempt>,
    dir: &Path,
    cancelled: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> ExitCode {
    let winner = attempts.iter().position(|a| a.verified);
    let selected = winner.unwrap_or(0);
    let mut report = attempts
        .get(selected)
        .and_then(|a| a.report.as_deref())
        .cloned()
        .unwrap_or_else(|| {
            crate::report::setup_failure(
                &options.args,
                "SAIL0113",
                "portfolio established no verification result",
            )
        });
    "accept-first-proof".clone_into(&mut report.mode);
    let _ = writeln!(
        err,
        "sail portfolio accept-first-proof: {}; evidence in {}",
        winner.map_or_else(
            || "no complete proof".to_owned(),
            |i| format!("proved with seed {}", attempts[i].configuration.seed)
        ),
        dir.display()
    );
    if let Some(a) = attempts.get(selected) {
        let _ = err.write_all(a.stderr.as_bytes());
    }
    let exit = if cancelled {
        130
    } else if winner.is_some() {
        0
    } else {
        attempts
            .first()
            .and_then(|a| a.exit_code)
            .filter(|&c| c != 0)
            .unwrap_or(2)
    };
    report.exit_code = Some(exit);
    report.portfolio = Some(PortfolioReport {
        seeds: options.seeds,
        winner_seed: winner.map(|i| attempts[i].configuration.seed),
        attempts,
    });
    if options.json {
        let _ = writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_default()
        );
    } else {
        let _ = writeln!(
            out,
            "portfolio accept-first-proof: {} verified, {} failed, {} unknown",
            report.summary.verified, report.summary.failed, report.summary.unknown
        );
    }
    ExitCode::from(u8::try_from(exit).unwrap_or(2))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(s: &[&str]) -> Vec<String> {
        s.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn explicit_portfolio_order_and_invalid_options() {
        let o = parse(&args(&[
            "--portfolio",
            "accept-first-proof",
            "--portfolio-seeds",
            "4,2",
        ]))
        .ok()
        .flatten();
        assert_eq!(o.map(|o| o.seeds), Some(vec![0, 4, 2]));
        for s in ["0", "4,4", "-1", "4294967296", ""] {
            assert!(
                parse(&args(&[
                    "--portfolio",
                    "accept-first-proof",
                    "--portfolio-seeds",
                    s
                ]))
                .is_err()
            );
        }
        assert!(parse(&args(&["--portfolio-seeds", "4"])).is_err());
        assert!(
            parse(&args(&[
                "--portfolio",
                "accept-first-proof",
                "--",
                "--no-verify"
            ]))
            .is_err()
        );
        assert!(
            parse(&args(&[
                "--portfolio",
                "accept-first-proof",
                "--",
                "--smt-option",
                "smt.random_seed=4"
            ]))
            .is_err()
        );
        assert!(
            parse(&args(&["--", "--portfolio", "x"]))
                .ok()
                .flatten()
                .is_none()
        );
    }

    #[test]
    fn baseline_is_unmodified_and_alternatives_set_both_seeds() {
        let original = args(&["-p", "demo", "--", "--rlimit", "3"]);
        let baseline = worker_args(&original, 0, Path::new("/owned/target"));
        assert!(!baseline.iter().any(|s| s.contains("random_seed")));
        let seeded = worker_args(&original, 4, Path::new("/owned/target"));
        assert!(seeded.contains(&"smt.random_seed=4".to_owned()));
        assert!(seeded.contains(&"sat.random_seed=4".to_owned()));
        assert!(partial(&args(&["--", "--verify-function=f"])));
    }

    #[test]
    fn empty_or_failed_reports_never_establish_a_proof() {
        let mut r = crate::report::setup_failure(&[], "SAIL0113", "no evidence");
        r.exit_code = Some(0);
        r.error = None;
        assert!(!complete(&r));
    }
    #[test]
    fn independent_runs_cannot_combine_partial_proofs() {
        let mut r: Report =
            serde_json::from_str(include_str!("../tests/fixtures/attr/report.golden.json"))
                .unwrap_or_else(|e| unreachable!("fixture: {e}"));
        r.exit_code = Some(0);
        for c in &mut r.crates {
            c.verus.success = Some(true);
            c.verus.errors = Some(0);
            for f in &mut c.functions {
                f.verdict = VerdictName::Verified;
            }
        }
        r.summary = sail_api::output::Summary::of(&r.crates);
        assert!(complete(&r));
        let mut first = r.clone();
        first.crates[0].functions[0].verdict = VerdictName::Failed;
        first.summary = sail_api::output::Summary::of(&first.crates);
        let mut second = r.clone();
        second.crates[0].functions[1].verdict = VerdictName::Failed;
        second.summary = sail_api::output::Summary::of(&second.crates);
        assert!(!complete(&first));
        assert!(!complete(&second));
        r.crates[0].functions.clear();
        r.summary = sail_api::output::Summary::of(&r.crates);
        assert!(!complete(&r));
    }
}
