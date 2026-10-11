//! The `sail` binary.
//!
//! Verification, persistent jobs, workspace watch, MCP, cache management,
//! fragility measurement and local benchmark commands.
#![forbid(unsafe_code)]

mod engine_cli;
mod fragility;
mod fragility_join;
mod jobs;
mod managed_cli;
mod measurement;
mod ops;
mod portfolio;
mod portfolio_scope;
mod product_cli;
mod remote_cache;
mod report;
#[cfg(test)]
mod scope;
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
    (
        "repair-check",
        "focus a repair, then independently validate its full target",
    ),
    (
        "engine",
        "install or inspect the optional interactive engine",
    ),
    ("explain", "show exact failure evidence from a retained job"),
    ("baseline", "save and list immutable verification baselines"),
    ("compare", "compare completed jobs or a saved baseline"),
    ("watch", "re-verify the workspace as files change"),
    ("jobs", "inspect, follow, or cancel local verification jobs"),
    (
        "mcp",
        "serve verification to agents over MCP (stdio; see docs/mcp.md)",
    ),
    ("serve", "run the S.A.I.L. daemon in the foreground"),
    ("status", "show daemon and cache state"),
    ("cache", "inspect and manage the result cache"),
    ("bench", "run benchmark workloads"),
    (
        "fragility",
        "measure proof fragility across solver seeds (opt-in; changes no verdict)",
    ),
];

/// Exit code for a malformed command line.
const EXIT_USAGE: u8 = 2;

fn usage() -> String {
    let mut text = format!(
        "sail {}\nSatisfiability Analysis & Inference Layer\n\nUSAGE:\n    sail <COMMAND> [ARGS]\n\nCOMMANDS:\n",
        env!("CARGO_PKG_VERSION")
    );
    for (name, help) in COMMANDS {
        let _ = writeln!(text, "    {name:<10} {help}");
    }
    text.push_str("\nVERIFY OPTIONS (before --):\n    --output json\n    --incremental\n    --background\n    --engine stock|interactive\n    --measure-fragility [--fragility-seeds N]\n    --portfolio accept-first-proof [--portfolio-seeds 1,2,...]\n    --portfolio-timeout-secs N\n");
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
        Some("_serve-jobs") => serve_jobs(args.get(1..).unwrap_or_default(), err),
        Some("verify") => verify::verify(args.get(1..).unwrap_or_default(), out, err),
        Some("_portfolio-attempt") => {
            verify::portfolio_attempt(args.get(1..).unwrap_or_default(), out, err)
        }
        Some("engine") => engine_cli::run(&args[1..], out, err),
        Some("repair-check") => product_cli::repair(&args[1..], out, err),
        Some("_repair-worker") => product_cli::worker(&args[1..], out, err),
        Some("explain" | "baseline" | "compare") => product_cli::evidence(args, out, err),
        Some("watch") => watch::watch(args.get(1..).unwrap_or_default(), out, err),
        Some("jobs") => jobs::run(args.get(1..).unwrap_or_default(), out, err),
        Some("status") => ops::status(args.get(1..).unwrap_or_default(), out, err),
        Some("cache") => ops::cache(args.get(1..).unwrap_or_default(), out, err),
        Some("mcp") => mcp(args.get(1..).unwrap_or_default(), err),
        Some("fragility") => fragility::fragility(args.get(1..).unwrap_or_default(), out, err),
        Some("bench") => bench(args.get(1..).unwrap_or_default(), err),
        Some(other) => {
            let _ = writeln!(err, "error[SAIL0002]: unknown command `{other}`\n");
            let _ = write!(err, "{}", usage());
            ExitCode::from(EXIT_USAGE)
        }
    }
}

/// Runs the installed benchmark tool with identical arguments and stdio.
fn bench(args: &[String], err: &mut dyn std::io::Write) -> ExitCode {
    let result = std::env::current_exe().and_then(|exe| {
        std::process::Command::new(exe.with_file_name("sail-bench"))
            .args(args)
            .status()
    });
    match result {
        Ok(status) => ExitCode::from(
            status
                .code()
                .and_then(|c| u8::try_from(c).ok())
                .unwrap_or(1),
        ),
        Err(e) => {
            let _ = writeln!(
                err,
                "error[SAIL0003]: cannot run sail-bench: {e}; build/install sail-bench next to sail"
            );
            ExitCode::from(2)
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
    match sail_mcp::serve_with_job_clients(config, stdin.lock(), std::io::stdout()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let _ = writeln!(err, "error[SAIL0302]: sail mcp: cannot read stdin: {e}");
            ExitCode::from(1)
        }
    }
}

/// The default daemon socket: `<cache>/daemon-v<protocol>-workspace-v2-proof-v1.sock`, so clients
/// of one protocol version never reach a daemon of another.
pub(crate) fn default_socket(cache: &std::path::Path) -> std::path::PathBuf {
    sail_api::jobs::default_cache_socket(cache)
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
/// `<cache>/daemon-v<N>-workspace-v2-proof-v1.sock` and `<cache>/results`, where `<cache>` is
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
    let executable = match std::env::current_exe() {
        Ok(path) => path,
        Err(e) => {
            let _ = writeln!(err, "error[SAIL0800]: cannot locate job worker: {e}");
            return ExitCode::from(2);
        }
    };
    let job_server = match sail_daemon::jobs::bind(
        &sail_api::jobs::socket_path(&socket),
        &root.join("projects/jobs"),
        &executable,
    )
    .and_then(sail_daemon::jobs::JobServer::spawn)
    {
        Ok(handle) => handle,
        Err(e) => {
            let _ = writeln!(err, "error[SAIL0800]: cannot start job service: {e}");
            return ExitCode::from(2);
        }
    };
    let mut job_server = job_server;
    // The optional endpoint owns independent background guards. A refusal
    // keeps managed verification on its original local content-hashing path.
    let mut sysroot_server = sail_daemon::sysroot::bind(&socket)
        .and_then(sail_daemon::sysroot::Server::spawn)
        .ok();
    let activity = job_server.activity();
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
    let result = server.run_with_activity(activity);
    job_server.shutdown();
    if let Some(server) = sysroot_server.as_mut() {
        server.shutdown();
    }
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let _ = writeln!(err, "error[SAIL0003]: daemon failed: {e}");
            ExitCode::from(1)
        }
    }
}

/// A jobs-only sidecar for an explicitly selected, older cache daemon.
#[cfg(unix)]
fn serve_jobs(args: &[String], err: &mut dyn std::io::Write) -> ExitCode {
    use std::sync::atomic::{AtomicBool, Ordering};
    let setup = (|| {
        let (socket, root, config) = serve_config(args)?;
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let jobs = sail_daemon::jobs::bind(
            &sail_api::jobs::socket_path(&socket),
            &root.join("projects/jobs"),
            &executable,
        )
        .and_then(sail_daemon::jobs::JobServer::spawn)
        .map_err(|error| error.to_string())?;
        Ok::<_, String>((jobs, config.idle_timeout))
    })();
    let (mut jobs, timeout) = match setup {
        Ok(setup) => setup,
        Err(error) => {
            let _ = writeln!(err, "error[SAIL0800]: cannot start job sidecar: {error}");
            return ExitCode::from(2);
        }
    };
    let stopped = std::sync::Arc::new(AtomicBool::new(false));
    for signal in [
        signal_hook::consts::SIGINT,
        signal_hook::consts::SIGTERM,
        signal_hook::consts::SIGHUP,
    ] {
        if let Err(error) = signal_hook::flag::register(signal, stopped.clone()) {
            let _ = writeln!(
                err,
                "error[SAIL0800]: cannot register job shutdown: {error}"
            );
            jobs.shutdown();
            return ExitCode::from(2);
        }
    }
    let activity = jobs.activity();
    let mut idle_since = std::time::Instant::now();
    while !stopped.load(Ordering::Relaxed) {
        if activity.load(Ordering::Relaxed) > 0 {
            idle_since = std::time::Instant::now();
        } else if timeout.is_some_and(|timeout| idle_since.elapsed() >= timeout) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    jobs.shutdown();
    ExitCode::SUCCESS
}

#[cfg(not(unix))]
fn serve_jobs(_args: &[String], err: &mut dyn std::io::Write) -> ExitCode {
    let _ = writeln!(err, "error[SAIL0800]: local jobs need Unix sockets");
    ExitCode::from(2)
}

#[cfg(not(unix))]
fn serve(_args: &[String], err: &mut dyn std::io::Write) -> ExitCode {
    let _ = writeln!(err, "error[SAIL0001]: `sail serve` needs Unix sockets");
    ExitCode::from(EXIT_UNIMPLEMENTED)
}

fn main() -> ExitCode {
    if let Some(exit) = sail_verus::adapter::dispatch() {
        return exit;
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    run(&args, &mut std::io::stdout(), &mut std::io::stderr())
}
