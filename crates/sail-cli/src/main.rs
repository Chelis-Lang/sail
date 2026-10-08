//! The `sail` binary.
//!
//! Status: every command named by the charter is recognised and refuses with
//! a stable "not implemented" exit code until its backend exists.
#![forbid(unsafe_code)]

use std::fmt::Write as _;
use std::process::ExitCode;

/// Commands named by the charter, in the order `--help` lists them.
const COMMANDS: &[(&str, &str)] = &[
    (
        "verify",
        "verify a crate; accepts the same arguments as `cargo verus verify`",
    ),
    ("watch", "re-verify affected functions as files change"),
    ("serve", "run the S.A.I.L. daemon in the foreground"),
    ("status", "show daemon, solver pool and cache state"),
    ("cache", "inspect and manage the result cache"),
    ("bench", "run benchmark workloads"),
];

/// Exit code for a malformed command line.
const EXIT_USAGE: u8 = 2;
/// Exit code for a command `sail` recognises but does not implement yet.
const EXIT_UNIMPLEMENTED: u8 = 3;

fn usage() -> String {
    let mut text = format!(
        "sail {}\nSatisfiability Analysis & Inference Layer\n\nUSAGE:\n    sail <COMMAND> [ARGS]\n\nCOMMANDS:\n",
        env!("CARGO_PKG_VERSION")
    );
    for (name, help) in COMMANDS {
        let _ = writeln!(text, "    {name:<8} {help}");
    }
    text
}

fn run(args: &[String], out: &mut impl std::io::Write, err: &mut impl std::io::Write) -> ExitCode {
    match args.first().map(String::as_str) {
        None => {
            let _ = write!(err, "{}", usage());
            ExitCode::from(EXIT_USAGE)
        }
        Some("-h" | "--help" | "help") => {
            let _ = write!(out, "{}", usage());
            ExitCode::SUCCESS
        }
        Some("-V" | "--version") => {
            let _ = writeln!(out, "sail {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some(cmd) if COMMANDS.iter().any(|(name, _)| *name == cmd) => {
            let _ = writeln!(
                err,
                "error[SAIL0001]: `sail {cmd}` is not implemented yet (see docs/ROADMAP.md)"
            );
            ExitCode::from(EXIT_UNIMPLEMENTED)
        }
        Some(other) => {
            let _ = writeln!(err, "error[SAIL0002]: unknown command `{other}`\n");
            let _ = write!(err, "{}", usage());
            ExitCode::from(EXIT_USAGE)
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run(&args, &mut std::io::stdout(), &mut std::io::stderr())
}
