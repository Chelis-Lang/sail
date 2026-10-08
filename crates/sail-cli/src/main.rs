//! The `sail` binary.
//!
//! Status: every command named by the charter is recognised and refuses with
//! a stable "not implemented" exit code until its backend exists.
#![forbid(unsafe_code)]

mod verify;

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
        Some("serve") => serve(args.get(1..).unwrap_or_default(), err),
        Some("verify") => verify::verify(args.get(1..).unwrap_or_default(), err),
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

/// The default daemon socket: `<cache>/daemon-v<protocol>.sock`, so clients
/// of one protocol version never reach a daemon of another.
pub(crate) fn default_socket(cache: &std::path::Path) -> std::path::PathBuf {
    cache.join(format!(
        "daemon-v{}.sock",
        sail_api::protocol::PROTOCOL_VERSION
    ))
}

/// `sail serve [--socket PATH] [--cache DIR]`: runs the daemon in the
/// foreground (ADR 0011). Defaults: `<cache>/daemon-v<N>.sock` and
/// `<cache>/results`, where `<cache>` is `$SAIL_CACHE_DIR` or
/// `~/.cache/sail`.
#[cfg(unix)]
fn serve(args: &[String], err: &mut impl std::io::Write) -> ExitCode {
    let cache = match sail_verus::cache_dir() {
        Ok(c) => c,
        Err(e) => {
            let _ = writeln!(err, "{e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let (mut socket, mut root) = (default_socket(&cache), cache.join("results"));
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let Some(value) = it.next() else {
            let _ = writeln!(err, "error[SAIL0002]: {arg} needs a value");
            return ExitCode::from(EXIT_USAGE);
        };
        match arg.as_str() {
            "--socket" => socket = value.into(),
            "--cache" => root = value.into(),
            _ => {
                let _ = writeln!(err, "error[SAIL0002]: unknown argument `{arg}`");
                return ExitCode::from(EXIT_USAGE);
            }
        }
    }
    let _ = writeln!(
        err,
        "sail daemon: socket {}, cache {}",
        socket.display(),
        root.display()
    );
    let stats = std::sync::Arc::new(sail_daemon::Stats::default());
    match sail_daemon::serve(&socket, &root, stats) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let _ = writeln!(err, "error[SAIL0003]: daemon failed: {e}");
            ExitCode::from(1)
        }
    }
}

#[cfg(not(unix))]
fn serve(_args: &[String], err: &mut impl std::io::Write) -> ExitCode {
    let _ = writeln!(err, "error[SAIL0001]: `sail serve` needs Unix sockets");
    ExitCode::from(EXIT_UNIMPLEMENTED)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run(&args, &mut std::io::stdout(), &mut std::io::stderr())
}
