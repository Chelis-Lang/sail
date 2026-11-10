//! Spike prototype (#632, VI-0039): checks tee records against a run.
//!
//! ```text
//! sail-conformance-invocations check RUN
//! sail-conformance-invocations compare PLAIN_RUN SAIL_RUN
//! ```
//!
//! `RUN` is a directory with the combined `stdout` and the tee's `captures/`.
//! `check` reports whether the records account for the combined stdout;
//! `compare` checks both arms, then compares them invocation by invocation.
//! Each prints one JSON object and exits 0 only when nothing failed.
#![forbid(unsafe_code)]

use std::path::Path;
use std::process::ExitCode;

use sail_conformance::invocations::{ArmReport, Invocation, check_arm, compare_arms, load};
use serde_json::json;

const LIMIT: usize = 200_000;

fn arm(run: &Path) -> Result<(Vec<Invocation>, ArmReport), String> {
    let invocations = load(&run.join("captures"))?;
    let stdout =
        std::fs::read(run.join("stdout")).map_err(|e| format!("{}: {e}", run.display()))?;
    let report = check_arm(&invocations, &stdout, LIMIT);
    Ok((invocations, report))
}

fn arm_ok(report: &ArmReport) -> bool {
    report.stdout.explained
        && report.incomplete_documents == 0
        && report.stdout.unterminated_parts == 0
        && !report.stdout.combined_unterminated
}

fn run(args: &[String]) -> Result<bool, String> {
    match args {
        [command, run] if command == "check" => {
            let (_, report) = arm(Path::new(run))?;
            println!("{}", json!({ "run": run, "arm": report }));
            Ok(arm_ok(&report))
        }
        [command, plain, sail] if command == "compare" => {
            let (plain_invocations, plain_report) = arm(Path::new(plain))?;
            let (sail_invocations, sail_report) = arm(Path::new(sail))?;
            let arms = compare_arms(&plain_invocations, &sail_invocations);
            let ok = arm_ok(&plain_report) && arm_ok(&sail_report) && arms.differences.is_empty();
            println!(
                "{}",
                json!({ "plain": plain_report, "sail": sail_report, "arms": arms, "same": ok })
            );
            Ok(ok)
        }
        _ => Err(
            "usage: sail-conformance-invocations check RUN | compare PLAIN_RUN SAIL_RUN".to_owned(),
        ),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    }
}
