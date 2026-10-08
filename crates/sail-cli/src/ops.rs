//! `sail status` and `sail cache`: daemon state and cache maintenance
//! (ADR 0014).
//!
//! Exit codes: 0 success; 2 usage error; 4 no daemon is listening
//! (`sail status`, `SAIL0401`); 5 `sail cache verify` found bad files and
//! did not remove them (`SAIL0501`); 1 any other failure.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use sail_api::protocol::{Info, StatsSnapshot};
use sail_cache::DiskCache;
use sail_cache::maint::Usage;

/// Exit code: no daemon is listening.
pub const EXIT_NO_DAEMON: u8 = 4;
/// Exit code: `sail cache verify` found bad files and left them.
pub const EXIT_BAD_ENTRIES: u8 = 5;
const EXIT_USAGE: u8 = 2;

/// How long `sail status` and `sail cache` wait on the daemon.
const TIMEOUT: Duration = Duration::from_secs(5);

/// Bytes for people: `512 B`, `1.5 KiB`, `20.0 MiB`.
pub fn human(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    #[allow(clippy::cast_precision_loss, reason = "a rounded figure for display")]
    let mut v = bytes as f64 / 1024.0;
    let mut unit = UNITS[0];
    for u in UNITS.iter().skip(1) {
        if v < 1024.0 {
            break;
        }
        v /= 1024.0;
        unit = u;
    }
    format!("{v:.1} {unit}")
}

/// Parses a byte count: digits with an optional `K`, `M`, `G` or `T`
/// suffix (powers of 1024), optionally followed by `iB` or `B`.
pub fn parse_bytes(text: &str) -> Option<u64> {
    let t = text.trim();
    let digits = t.bytes().take_while(u8::is_ascii_digit).count();
    let (num, suffix) = t.split_at(digits);
    let n: u64 = num.parse().ok()?;
    let suffix = suffix.trim().to_ascii_lowercase();
    let shift = match suffix.trim_end_matches("ib").trim_end_matches('b') {
        "" => 0,
        "k" => 10,
        "m" => 20,
        "g" => 30,
        "t" => 40,
        _ => return None,
    };
    n.checked_mul(1u64 << shift)
}

fn duration(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    match (h, m) {
        (0, 0) => format!("{s}s"),
        (0, _) => format!("{m}m {s}s"),
        _ => format!("{h}h {m}m {s}s"),
    }
}

/// The daemon socket: `$SAIL_DAEMON_SOCKET`, else the default socket under
/// sail's cache directory.
fn default_socket() -> Result<PathBuf, String> {
    match std::env::var_os("SAIL_DAEMON_SOCKET").filter(|v| !v.is_empty()) {
        Some(s) => Ok(PathBuf::from(s)),
        None => sail_verus::cache_dir()
            .map(|c| crate::default_socket(&c))
            .map_err(|e| e.to_string()),
    }
}

/// What the daemon at `socket` reports: counters, and (from daemons that
/// know `INFO`) its configuration.
#[cfg(unix)]
fn ask(socket: &Path) -> Option<(StatsSnapshot, Option<Info>)> {
    use sail_api::client::Client;
    let mut c = Client::connect_with_timeout(socket, TIMEOUT).ok()?;
    let stats = c.stats().ok()?;
    // A daemon that predates INFO closes the connection.
    let info = c.info().ok();
    Some((stats, info))
}

#[cfg(not(unix))]
fn ask(_socket: &Path) -> Option<(StatsSnapshot, Option<Info>)> {
    None
}

fn usage_error(err: &mut dyn std::io::Write, msg: &str) -> ExitCode {
    let _ = writeln!(err, "error[SAIL0002]: {msg}");
    ExitCode::from(EXIT_USAGE)
}

fn print_json(out: &mut dyn std::io::Write, value: &serde_json::Value) {
    let _ = writeln!(
        out,
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

fn status_text(
    socket: &Path,
    info: Option<&Info>,
    disk: Option<Usage>,
    stats: &StatsSnapshot,
) -> String {
    let mut t = String::new();
    let _ = writeln!(t, "sail daemon: running on {}", socket.display());
    match info {
        Some(i) => {
            let _ = writeln!(t, "  pid          {}", i.pid);
            let _ = writeln!(
                t,
                "  version      sail {}, protocol {}",
                i.sail_version, i.protocol_version
            );
            let _ = writeln!(t, "  uptime       {}", duration(i.uptime_secs));
            let _ = writeln!(
                t,
                "  idle stop    {}",
                if i.idle_timeout_secs == 0 {
                    "never".to_owned()
                } else {
                    format!(
                        "after {} without connections",
                        duration(i.idle_timeout_secs)
                    )
                }
            );
            let _ = writeln!(t, "  cache root   {}", i.cache_root);
            if let Some(log) = &i.log {
                let _ = writeln!(t, "  log          {log}");
            }
            if let Some(d) = disk {
                let _ = writeln!(
                    t,
                    "  disk tier    {} entries, {}",
                    d.entries,
                    human(d.bytes)
                );
            }
            let m = i.memory;
            if m.capacity_bytes == 0 {
                let _ = writeln!(t, "  memory tier  off");
            } else {
                let _ = writeln!(
                    t,
                    "  memory tier  {} entries, {} of {} ({} pending write)",
                    m.entries,
                    human(m.bytes),
                    human(m.capacity_bytes),
                    m.pending
                );
            }
            let _ = writeln!(
                t,
                "  write queue  {} entries, {} of {}",
                i.queue.entries,
                human(i.queue.bytes),
                human(i.queue.capacity_bytes)
            );
        }
        None => {
            let _ = writeln!(
                t,
                "  (this daemon predates the INFO operation: no configuration to show)"
            );
        }
    }
    let _ = writeln!(
        t,
        "  since start  {} sessions, {} gets, {} hits ({} from memory), {} puts, {} dropped puts",
        stats.hellos, stats.gets, stats.hits, stats.memory_hits, stats.puts, stats.dropped_puts
    );
    t
}

/// `sail status [--socket PATH] [--json]`.
pub fn status(
    args: &[String],
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> ExitCode {
    let (mut socket, mut json) = (None, false);
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--json" => json = true,
            "--socket" => match it.next() {
                Some(v) => socket = Some(PathBuf::from(v)),
                None => return usage_error(err, "--socket needs a value"),
            },
            other => return usage_error(err, &format!("unknown argument `{other}`")),
        }
    }
    let socket = match socket.map_or_else(default_socket, Ok) {
        Ok(s) => s,
        Err(e) => return usage_error(err, &e),
    };
    let Some((stats, info)) = ask(&socket) else {
        if json {
            print_json(
                out,
                &serde_json::json!({ "running": false, "socket": socket.display().to_string() }),
            );
        }
        let _ = writeln!(
            err,
            "error[SAIL0401]: no sail daemon is listening on {} (`sail serve` starts one; `sail verify` starts one on demand)",
            socket.display()
        );
        return ExitCode::from(EXIT_NO_DAEMON);
    };
    let disk = info.as_ref().map(|i| DiskCache::new(&i.cache_root).usage());
    if json {
        print_json(
            out,
            &serde_json::json!({
                "running": true,
                "socket": socket.display().to_string(),
                "info": info,
                "stats": stats,
                "disk": disk,
            }),
        );
        return ExitCode::SUCCESS;
    }
    let _ = write!(out, "{}", status_text(&socket, info.as_ref(), disk, &stats));
    ExitCode::SUCCESS
}

/// Parsed `sail cache` options.
#[derive(Default)]
struct CacheArgs {
    sub: Option<String>,
    cache: Option<PathBuf>,
    socket: Option<PathBuf>,
    json: bool,
    yes: bool,
    remove: bool,
    max_bytes: Option<u64>,
}

fn parse_cache_args(args: &[String]) -> Result<CacheArgs, String> {
    let mut a = CacheArgs::default();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = |name: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--json" => a.json = true,
            "--yes" => a.yes = true,
            "--remove" => a.remove = true,
            "--cache" => a.cache = Some(PathBuf::from(value("--cache")?)),
            "--socket" => a.socket = Some(PathBuf::from(value("--socket")?)),
            "--max-bytes" => {
                let v = value("--max-bytes")?;
                a.max_bytes = Some(parse_bytes(&v).ok_or_else(|| format!("bad byte count `{v}`"))?);
            }
            s if !s.starts_with('-') && a.sub.is_none() => a.sub = Some(s.to_owned()),
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(a)
}

const CACHE_USAGE: &str = "USAGE:
    sail cache [--cache DIR | --socket PATH] [--json] <COMMAND>

COMMANDS:
    stats                  entries and bytes by command, unknown answers, bad files
    verify [--remove]      check every entry's integrity; --remove deletes bad files
    gc --max-bytes N       evict least recently used entries down to N bytes (K/M/G suffixes)
    clear --yes            remove every entry

The cache is the running daemon's (asked over its socket), or DIR.
";

/// The cache root to work on: `--cache`, else the daemon's, else the
/// default root `sail serve` uses.
fn cache_root(a: &CacheArgs, err: &mut dyn std::io::Write) -> Result<PathBuf, String> {
    if let Some(c) = &a.cache {
        return Ok(c.clone());
    }
    let socket = a.socket.clone().map_or_else(default_socket, Ok)?;
    if let Some((_, Some(info))) = ask(&socket) {
        return Ok(PathBuf::from(info.cache_root));
    }
    let root = sail_verus::cache_dir()
        .map_err(|e| e.to_string())?
        .join("results");
    let _ = writeln!(
        err,
        "note: no daemon answered on {}; using the default cache root {}",
        socket.display(),
        root.display()
    );
    Ok(root)
}

fn usage_line(u: Usage) -> String {
    format!("{} entries, {}", u.entries, human(u.bytes))
}

fn stats_text(root_text: &str, s: &sail_cache::maint::Summary) -> String {
    let mut t = String::new();
    let _ = writeln!(t, "cache {root_text}");
    let _ = writeln!(
        t,
        "  {}",
        usage_line(Usage {
            entries: s.entries,
            bytes: s.bytes
        })
    );
    for (cmd, usage) in &s.by_command {
        let _ = writeln!(t, "    {cmd:<20} {}", usage_line(*usage));
    }
    let u = &s.unknown;
    let _ = writeln!(
        t,
        "  unknown answers: {} ({} with effort, {} used at most half their limit)",
        u.entries, u.with_effort, u.within_half_limit
    );
    for (reason, n) in &u.by_reason {
        let _ = writeln!(t, "    {n:>6}  {reason}");
    }
    if let Some(f) = u.used_fraction {
        let _ = writeln!(
            t,
            "    effort used / limit: min {:.3}, median {:.3}, max {:.3}",
            f.min, f.median, f.max
        );
    }
    let _ = writeln!(
        t,
        "  invalid files: {} (see `sail cache verify`); temporary files: {}",
        s.invalid, s.temp_files
    );
    t
}

fn verify(
    disk: &DiskCache,
    root_text: &str,
    a: &CacheArgs,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> ExitCode {
    let r = disk.verify(a.remove);
    let left = u64::try_from(r.bad.len()).unwrap_or(u64::MAX) - r.removed;
    if a.json {
        print_json(out, &serde_json::json!({ "root": root_text, "verify": r }));
    } else {
        let mut t = String::new();
        let _ = writeln!(
            t,
            "cache {root_text}: checked {} entries, {} valid, {} bad files ({} removed)",
            r.checked,
            r.valid,
            r.bad.len(),
            r.removed
        );
        for b in &r.bad {
            let _ = writeln!(
                t,
                "  {:<14} {}{}",
                b.problem.name(),
                b.path,
                if b.removed { " (removed)" } else { "" }
            );
        }
        let _ = write!(out, "{t}");
    }
    if left > 0 {
        let _ = writeln!(
            err,
            "error[SAIL0501]: {left} bad cache files; `sail cache verify --remove` deletes them (each is a miss, never a wrong answer)"
        );
        return ExitCode::from(EXIT_BAD_ENTRIES);
    }
    ExitCode::SUCCESS
}

/// `sail cache ...`.
pub fn cache(
    args: &[String],
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> ExitCode {
    let a = match parse_cache_args(args) {
        Ok(a) => a,
        Err(e) => {
            let _ = writeln!(err, "error[SAIL0002]: {e}\n\n{CACHE_USAGE}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let Some(sub) = a.sub.clone() else {
        let _ = write!(err, "{CACHE_USAGE}");
        return ExitCode::from(EXIT_USAGE);
    };
    if !matches!(sub.as_str(), "stats" | "verify" | "gc" | "clear") {
        let _ = writeln!(
            err,
            "error[SAIL0002]: unknown cache command `{sub}`\n\n{CACHE_USAGE}"
        );
        return ExitCode::from(EXIT_USAGE);
    }
    if sub == "gc" && a.max_bytes.is_none() {
        return usage_error(err, "`sail cache gc` needs --max-bytes N");
    }
    if sub == "clear" && !a.yes {
        let _ = writeln!(
            err,
            "error[SAIL0502]: `sail cache clear` removes every entry; pass --yes to confirm"
        );
        return ExitCode::from(EXIT_USAGE);
    }
    let root = match cache_root(&a, err) {
        Ok(r) => r,
        Err(e) => return usage_error(err, &e),
    };
    let disk = DiskCache::new(&root);
    let root_text = root.display().to_string();
    match sub.as_str() {
        "stats" => {
            let s = disk.summarize();
            if a.json {
                print_json(out, &serde_json::json!({ "root": root_text, "stats": s }));
                return ExitCode::SUCCESS;
            }
            let _ = write!(out, "{}", stats_text(&root_text, &s));
            ExitCode::SUCCESS
        }
        "verify" => verify(&disk, &root_text, &a, out, err),
        "gc" => {
            let r = disk.gc(a.max_bytes.unwrap_or(u64::MAX));
            if a.json {
                print_json(out, &serde_json::json!({ "root": root_text, "gc": r }));
            } else {
                let _ = writeln!(
                    out,
                    "cache {root_text}: evicted {}; {} remain (budget {}); {} stale temporary files removed",
                    usage_line(r.evicted),
                    usage_line(r.after),
                    human(r.max_bytes),
                    r.temp_removed
                );
            }
            ExitCode::SUCCESS
        }
        _ => match disk.clear() {
            Ok(u) => {
                if a.json {
                    print_json(out, &serde_json::json!({ "root": root_text, "cleared": u }));
                } else {
                    let _ = writeln!(out, "cache {root_text}: removed {}", usage_line(u));
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                let _ = writeln!(err, "error[SAIL0503]: cannot clear {root_text}: {e}");
                ExitCode::from(1)
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_counts_parse_with_suffixes() {
        assert_eq!(parse_bytes("123"), Some(123));
        assert_eq!(parse_bytes("64K"), Some(64 * 1024));
        assert_eq!(parse_bytes("64KiB"), Some(64 * 1024));
        assert_eq!(parse_bytes("2mb"), Some(2 << 20));
        assert_eq!(parse_bytes("1G"), Some(1 << 30));
        assert_eq!(parse_bytes("1X"), None);
        assert_eq!(parse_bytes(""), None);
        assert_eq!(parse_bytes("99999999999999T"), None);
    }

    #[test]
    fn human_sizes() {
        assert_eq!(human(10), "10 B");
        assert_eq!(human(1536), "1.5 KiB");
        assert_eq!(human(3 << 20), "3.0 MiB");
        assert_eq!(duration(3725), "1h 2m 5s");
        assert_eq!(duration(65), "1m 5s");
    }
}
