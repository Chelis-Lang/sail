//! The `sail` binary.
//!
//! Status: `verify`, `watch`, `serve`, `status` and `cache` work; the other commands
//! named by the charter are recognised and refuse with a stable "not
//! implemented" exit code until their backends exist.
#![forbid(unsafe_code)]

mod ops;
mod report;
mod shared_ops;
mod verify;
mod watch;

use std::fmt::Write as _;
use std::process::ExitCode;

/// Commands named by the charter, in the order `--help` lists them.
const COMMANDS: &[(&str, &str)] = &[
    (
        "verify",
        "verify a crate; accepts the same arguments as `cargo verus verify`",
    ),
    ("watch", "re-verify affected functions as files change"),
    (
        "mcp",
        "serve verification to agents over MCP (stdio; see docs/mcp.md)",
    ),
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

fn run(args: &[String], out: &mut dyn std::io::Write, err: &mut dyn std::io::Write) -> ExitCode {
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
        Some("verify") => verify::verify(args.get(1..).unwrap_or_default(), out, err),
        Some("watch") => watch::watch(args.get(1..).unwrap_or_default(), err),
        Some("status") => ops::status(args.get(1..).unwrap_or_default(), out, err),
        Some("cache") => ops::cache(args.get(1..).unwrap_or_default(), out, err),
        Some("mcp") => mcp(args.get(1..).unwrap_or_default(), err),
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

/// `sail mcp`: the MCP server on stdin and stdout (ADR 0016). Tool calls
/// run this same `sail` binary. Results are recorded under
/// `<cache>/mcp`, where `<cache>` is `$SAIL_CACHE_DIR` or `~/.cache/sail`.
fn mcp(args: &[String], err: &mut dyn std::io::Write) -> ExitCode {
    if let Some(arg) = args.first() {
        let _ = writeln!(
            err,
            "error[SAIL0002]: `sail mcp` takes no arguments (got `{arg}`); it speaks MCP on stdin and stdout"
        );
        return ExitCode::from(EXIT_USAGE);
    }
    let (sail, cwd) = match (std::env::current_exe(), std::env::current_dir()) {
        (Ok(s), Ok(c)) => (s, c),
        (Err(e), _) | (_, Err(e)) => {
            let _ = writeln!(err, "error[SAIL0301]: sail mcp cannot start: {e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let config = sail_mcp::Config {
        sail,
        store_dir: sail_verus::cache_dir().ok().map(|c| c.join("mcp")),
        cwd,
    };
    let stdin = std::io::stdin();
    match sail_mcp::serve(config, stdin.lock(), std::io::stdout()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let _ = writeln!(err, "error[SAIL0302]: sail mcp: cannot read stdin: {e}");
            ExitCode::from(1)
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

/// Reads a whole number from the environment variable `name`.
fn env_number(name: &str) -> Result<Option<u64>, String> {
    match std::env::var(name) {
        Ok(v) if !v.is_empty() => v
            .trim()
            .parse()
            .map(Some)
            .map_err(|_| format!("{name} must be a whole number, not `{v}`")),
        _ => Ok(None),
    }
}

/// The socket, cache root and daemon settings for
/// `sail serve [--socket PATH] [--cache DIR] [--memory-bytes N]
/// [--idle-timeout SECS] [--shared DIR] [--shared-timeout-ms MS]`, which runs the
/// daemon in the foreground (ADR 0011, ADR 0015). Defaults:
/// `<cache>/daemon-v<N>.sock` and `<cache>/results`, where `<cache>` is
/// `$SAIL_CACHE_DIR` or `~/.cache/sail`; a 256 MiB memory tier
/// (`$SAIL_DAEMON_MEMORY_BYTES`); a 30-minute idle timeout
/// (`$SAIL_DAEMON_IDLE_TIMEOUT`, seconds, 0 for never); no shared tier
/// (`$SAIL_SHARED_STORE`), looked up with a 250 ms timeout
/// (`$SAIL_SHARED_TIMEOUT_MS`) and trusting the keys in
/// `<cache root>/trust/*.pub` (ADR 0017). The log is
/// `<cache root>/daemon.log`. SIGINT, SIGTERM and SIGHUP stop it cleanly.
#[cfg(unix)]
fn serve_config(
    args: &[String],
) -> Result<(std::path::PathBuf, std::path::PathBuf, sail_daemon::Config), String> {
    let cache = sail_verus::cache_dir().map_err(|e| e.to_string())?;
    let mut config = sail_daemon::Config {
        idle_timeout: Some(sail_daemon::DEFAULT_IDLE_TIMEOUT),
        ..sail_daemon::Config::default()
    };
    let mut shared_store = std::env::var_os("SAIL_SHARED_STORE")
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from);
    let mut shared_timeout = sail_daemon::DEFAULT_SHARED_TIMEOUT;
    if let Some(n) = env_number("SAIL_DAEMON_MEMORY_BYTES")? {
        config.memory_bytes = n;
    }
    if let Some(n) = env_number("SAIL_DAEMON_IDLE_TIMEOUT")? {
        config.idle_timeout = (n > 0).then(|| std::time::Duration::from_secs(n));
    }
    if let Some(n) = env_number("SAIL_SHARED_TIMEOUT_MS")? {
        shared_timeout = std::time::Duration::from_millis(n);
    }
    // Fault injection only (ADR 0015): slow every disk write.
    if let Some(n) = env_number("SAIL_DAEMON_WRITE_DELAY_MS")? {
        config.write_delay = (n > 0).then(|| std::time::Duration::from_millis(n));
    }
    let (mut socket, mut root) = (default_socket(&cache), cache.join("results"));
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let value = it.next().ok_or_else(|| format!("{arg} needs a value"))?;
        match arg.as_str() {
            "--socket" => socket = value.into(),
            "--cache" => root = value.into(),
            "--memory-bytes" => {
                config.memory_bytes =
                    ops::parse_bytes(value).ok_or_else(|| format!("bad byte count `{value}`"))?;
            }
            "--idle-timeout" => {
                let n = value
                    .parse::<u64>()
                    .map_err(|_| format!("bad idle timeout `{value}` (seconds)"))?;
                config.idle_timeout = (n > 0).then(|| std::time::Duration::from_secs(n));
            }
            "--shared" => shared_store = Some(value.into()),
            "--shared-timeout-ms" => {
                let n = value
                    .parse::<u64>()
                    .map_err(|_| format!("bad shared timeout `{value}` (ms)"))?;
                shared_timeout = std::time::Duration::from_millis(n);
            }
            _ => return Err(format!("unknown argument `{arg}`")),
        }
    }
    config.log = Some(root.join("daemon.log"));
    config.shared = shared_store.map(|store| sail_daemon::SharedConfig {
        store,
        trust_dir: sail_cache::shared::trust_dir(&root),
        timeout: shared_timeout,
        backoff: sail_daemon::DEFAULT_SHARED_BACKOFF,
    });
    Ok((socket, root, config))
}

#[cfg(unix)]
fn serve(args: &[String], err: &mut dyn std::io::Write) -> ExitCode {
    let (socket, root, config) = match serve_config(args) {
        Ok(c) => c,
        Err(e) => {
            // Errors that carry their own code (the cache directory) keep it.
            let code = if e.starts_with("error[") {
                ""
            } else {
                "error[SAIL0002]: "
            };
            let _ = writeln!(err, "{code}{e}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let stats = std::sync::Arc::new(sail_daemon::Stats::default());
    let server = match sail_daemon::bind(&socket, &root, stats, &config) {
        Ok(s) => s,
        Err(e) => {
            let _ = writeln!(
                err,
                "error[SAIL0003]: cannot serve on {}: {e}",
                socket.display()
            );
            return ExitCode::from(1);
        }
    };
    // Only this foreground line goes to stderr; `sail verify` discards it.
    let _ = writeln!(
        err,
        "sail daemon: socket {}, cache {}, log {}",
        socket.display(),
        root.display(),
        root.join("daemon.log").display()
    );
    let handle = server.handle();
    let signals = signal_hook::iterator::Signals::new([
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGHUP,
    ]);
    if let Ok(mut signals) = signals {
        let _ = std::thread::Builder::new()
            .name("sail-signals".to_owned())
            .spawn(move || {
                if signals.forever().next().is_some() {
                    handle.shutdown();
                }
            });
    }
    match server.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let _ = writeln!(err, "error[SAIL0003]: daemon failed: {e}");
            ExitCode::from(1)
        }
    }
}

#[cfg(not(unix))]
fn serve(_args: &[String], err: &mut dyn std::io::Write) -> ExitCode {
    let _ = writeln!(err, "error[SAIL0001]: `sail serve` needs Unix sockets");
    ExitCode::from(EXIT_UNIMPLEMENTED)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    run(&args, &mut std::io::stdout(), &mut std::io::stderr())
}
