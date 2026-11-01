//! `sail cache projects list|prune`: the managed workspaces' compiled targets,
//! proof receipts and run records under `<cache>/projects/`.

use sail_verus::managed::projects::{self, Project, Pruned, Selection};
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const EXIT_USAGE: u8 = 2;

pub(crate) const USAGE: &str = "\
List and remove managed workspaces' compiled targets, receipts and run records.

USAGE:
    sail cache projects list [--json]
    sail cache projects prune (--all | [--older-than DAYS] [--missing]) [--dry-run] [--json]
    sail cache projects prune --to-bytes N [--dry-run] [--json]
    sail cache projects budget [--max-bytes N | --disable] [--json]

Managed runs (`sail verify`, jobs, MCP and repairs) keep each
workspace's compiled dependencies, proof receipts and run records under
<cache>/projects/, where <cache> is $SAIL_CACHE_DIR or ~/.cache/sail.

COMMANDS:
    list                   each workspace: size, when a run last started, path,
                           and whether the path still exists
    prune                  remove workspaces' directories; the next run of a
                           removed workspace compiles and verifies again
    budget                 inspect or configure opt-in background eviction

PRUNE OPTIONS (criteria combine: a workspace must meet each one given):
    --older-than DAYS      last used at least DAYS days ago
    --missing              its recorded path no longer exists
    --all                  every workspace
    --to-bytes N           evict oldest idle workspaces to a soft byte cap,
                           protecting the most recently used workspace
    --dry-run              show what would be removed; remove nothing

A workspace that a running managed run or job holds is skipped and reported.
";

/// `sail cache projects ...` (`args` follow `projects`).
pub fn run(
    args: &[String],
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> ExitCode {
    if args.is_empty() {
        let _ = write!(err, "{USAGE}");
        return ExitCode::from(EXIT_USAGE);
    }
    if args
        .iter()
        .any(|a| matches!(a.as_str(), "-h" | "--help" | "help"))
    {
        let _ = write!(out, "{USAGE}");
        return ExitCode::SUCCESS;
    }
    let parsed = match parse(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            let _ = writeln!(err, "error[SAIL0002]: {message}\n\n{USAGE}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    let cache = match sail_verus::cache_dir() {
        Ok(cache) => cache,
        Err(error) => {
            let _ = writeln!(err, "error[{}]: {}", error.code, error.message);
            return ExitCode::from(1);
        }
    };
    let listed = match projects::list(&cache) {
        Ok(listed) => listed,
        Err(error) => {
            let _ = writeln!(
                err,
                "error[SAIL0504]: cannot read {}: {error}",
                projects::root(&cache).display()
            );
            return ExitCode::from(1);
        }
    };
    match parsed {
        Parsed::List { json } => {
            list(&cache, &listed, json, out);
            ExitCode::SUCCESS
        }
        Parsed::Budget { setting, json } => budget(&cache, &listed, setting, json, out, err),
        Parsed::Trim {
            max_bytes,
            dry_run,
            json,
        } => {
            let result = projects::budget::enforce(&cache, max_bytes, dry_run);
            match result {
                Ok(report) => {
                    if json {
                        print_json(out, &serde_json::json!(report));
                    } else {
                        let _ = writeln!(
                            out,
                            "{} {}; {} remains (cap {}); {} busy, {} failed{}",
                            if dry_run { "would free" } else { "freed" },
                            crate::ops::human(report.freed_bytes),
                            crate::ops::human(report.remaining_bytes),
                            crate::ops::human(max_bytes),
                            report.busy.len(),
                            report.failed.len(),
                            if report.over_budget {
                                "; over budget: protected, busy or failed data remains"
                            } else {
                                ""
                            }
                        );
                    }
                    if report.failed.is_empty() {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::from(1)
                    }
                }
                Err(error) => budget_error(&error, err),
            }
        }
        Parsed::Prune {
            selection,
            dry_run,
            json,
        } => prune(&cache, &listed, selection, dry_run, json, out, err),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum BudgetSetting {
    Set(u64),
    Disable,
}

#[derive(Debug, PartialEq, Eq)]
enum Parsed {
    Budget {
        setting: Option<BudgetSetting>,
        json: bool,
    },
    Trim {
        max_bytes: u64,
        dry_run: bool,
        json: bool,
    },
    List {
        json: bool,
    },
    Prune {
        selection: Selection,
        dry_run: bool,
        json: bool,
    },
}

fn parse(args: &[String]) -> Result<Parsed, String> {
    let (command, rest) = args.split_first().ok_or("missing command")?;
    let mut json = false;
    let mut dry_run = false;
    let mut selection = Selection::default();
    let mut setting = None;
    let mut to_bytes = None;
    let mut it = rest.iter();
    while let Some(arg) = it.next() {
        match (command.as_str(), arg.as_str()) {
            (_, "--json") => json = true,
            ("budget", "--max-bytes") if setting.is_none() => {
                setting = Some(BudgetSetting::Set(byte_limit(it.next())?));
            }
            ("budget", "--disable") if setting.is_none() => setting = Some(BudgetSetting::Disable),
            ("prune", "--to-bytes") if to_bytes.is_none() => {
                to_bytes = Some(byte_limit(it.next())?);
            }
            ("prune", "--dry-run") => dry_run = true,
            ("prune", "--all") => selection.all = true,
            ("prune", "--missing") => selection.missing = true,
            ("prune", "--older-than") => {
                let value = it.next().ok_or("--older-than needs a number of days")?;
                let days: u64 = value
                    .parse()
                    .map_err(|_| format!("--older-than needs a number of days, not `{value}`"))?;
                selection.older_than = Some(Duration::from_secs(days.saturating_mul(86_400)));
            }
            ("list" | "prune", other) => {
                return Err(format!(
                    "unknown argument `{other}` for `projects {command}`"
                ));
            }
            (other, _) => return Err(format!("unknown projects command `{other}`")),
        }
    }
    match command.as_str() {
        "list" => Ok(Parsed::List { json }),
        "budget" => Ok(Parsed::Budget { setting, json }),
        "prune" if to_bytes.is_some() && !selection.is_empty() => {
            Err("--to-bytes cannot be combined with --all, --older-than or --missing".into())
        }
        "prune" if to_bytes.is_some() => Ok(Parsed::Trim {
            max_bytes: to_bytes.unwrap_or_default(),
            dry_run,
            json,
        }),
        "prune" if selection.is_empty() => {
            Err("`projects prune` needs --all, --older-than DAYS or --missing".into())
        }
        "prune" if selection.all && (selection.missing || selection.older_than.is_some()) => {
            Err("--all cannot be combined with --older-than or --missing".into())
        }
        "prune" => Ok(Parsed::Prune {
            selection,
            dry_run,
            json,
        }),
        other => Err(format!("unknown projects command `{other}`")),
    }
}

fn byte_limit(value: Option<&String>) -> Result<u64, String> {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|bytes| *bytes > 0)
        .ok_or_else(|| "byte limit requires a positive whole number of bytes".into())
}

fn budget_error(error: &std::io::Error, err: &mut dyn std::io::Write) -> ExitCode {
    let _ = writeln!(
        err,
        "error[SAIL0504]: cannot manage project budget: {error}"
    );
    ExitCode::from(1)
}

fn budget(
    cache: &Path,
    listed: &[Project],
    setting: Option<BudgetSetting>,
    json: bool,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> ExitCode {
    let result = setting.map_or_else(
        || projects::budget::load(cache),
        |setting| {
            let limit = match setting {
                BudgetSetting::Set(bytes) => Some(bytes),
                BudgetSetting::Disable => None,
            };
            projects::budget::save(cache, limit).map(|()| limit)
        },
    );
    let limit = match result {
        Ok(limit) => limit,
        Err(error) => return budget_error(&error, err),
    };
    let bytes = listed
        .iter()
        .fold(0u64, |sum, p| sum.saturating_add(p.bytes));
    if json {
        print_json(
            out,
            &serde_json::json!({ "enabled": limit.is_some(), "max_bytes": limit,
            "bytes": bytes, "over_budget": limit.is_some_and(|cap| bytes > cap) }),
        );
    } else if let Some(cap) = limit {
        let _ = writeln!(
            out,
            "managed project budget: {}; accounted {}{}",
            crate::ops::human(cap),
            crate::ops::human(bytes),
            if bytes > cap { " (over budget)" } else { "" }
        );
    } else {
        let _ = writeln!(
            out,
            "managed project budget: disabled; accounted {}",
            crate::ops::human(bytes)
        );
    }
    ExitCode::SUCCESS
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn ago(now: u64, then: u64) -> String {
    let secs = now.saturating_sub(then);
    match secs {
        0..3_600 => format!("{}m ago", secs / 60),
        3_600..86_400 => format!("{}h ago", secs / 3_600),
        _ => format!("{}d ago", secs / 86_400),
    }
}

fn name(project: &Project) -> String {
    match (&project.workspace, project.exists) {
        (Some(path), Some(false)) => format!("{} (missing)", path.display()),
        (Some(path), _) => path.display().to_string(),
        (None, _) => format!("(path unknown; key {})", &project.key[..12]),
    }
}

fn print_json(out: &mut dyn std::io::Write, value: &serde_json::Value) {
    let _ = writeln!(
        out,
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

fn list(cache: &Path, listed: &[Project], json: bool, out: &mut dyn std::io::Write) {
    let root = projects::root(cache);
    let total: u64 = listed.iter().map(|p| p.bytes).sum();
    if json {
        print_json(
            out,
            &serde_json::json!({ "root": root, "bytes": total, "projects": listed }),
        );
        return;
    }
    let _ = writeln!(
        out,
        "managed workspaces in {}: {}, {}",
        root.display(),
        listed.len(),
        crate::ops::human(total)
    );
    if listed.is_empty() {
        return;
    }
    let now = now();
    let _ = writeln!(out, "  {:>10}  {:<9}  WORKSPACE", "SIZE", "LAST USED");
    for project in listed {
        let _ = writeln!(
            out,
            "  {:>10}  {:<9}  {}",
            crate::ops::human(project.bytes),
            ago(now, project.last_used_unix),
            name(project)
        );
    }
}

fn prune(
    cache: &Path,
    listed: &[Project],
    selection: Selection,
    dry_run: bool,
    json: bool,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
) -> ExitCode {
    let now = now();
    let mut removed = Vec::new();
    let mut busy = Vec::new();
    let mut failed = Vec::new();
    for project in listed.iter().filter(|p| selection.selects(p, now)) {
        if dry_run {
            removed.push((project, project.bytes));
            continue;
        }
        match projects::prune(cache, &project.key) {
            Ok(Pruned::Removed(bytes)) => removed.push((project, bytes)),
            Ok(Pruned::Busy) => busy.push(project),
            Err(error) => failed.push((project, error.to_string())),
        }
    }
    let freed: u64 = removed.iter().map(|(_, bytes)| bytes).sum();
    if json {
        let entry = |p: &Project| serde_json::json!({ "key": p.key, "workspace": p.workspace });
        print_json(
            out,
            &serde_json::json!({
                "root": projects::root(cache),
                "dry_run": dry_run,
                "freed_bytes": freed,
                "removed": removed.iter().map(|(p, bytes)| {
                    let mut e = entry(p);
                    e["bytes"] = (*bytes).into();
                    e
                }).collect::<Vec<_>>(),
                "busy": busy.iter().map(|p| entry(p)).collect::<Vec<_>>(),
                "failed": failed.iter().map(|(p, error)| {
                    let mut e = entry(p);
                    e["error"] = error.clone().into();
                    e
                }).collect::<Vec<_>>(),
            }),
        );
    } else {
        let verb = if dry_run { "would remove" } else { "removed" };
        for (project, bytes) in &removed {
            let _ = writeln!(
                out,
                "{verb} {:>10}  {}",
                crate::ops::human(*bytes),
                name(project)
            );
        }
        for project in &busy {
            let _ = writeln!(
                out,
                "skipped: in use by a running managed run or job  {}",
                name(project)
            );
        }
        let _ = writeln!(
            out,
            "{} {} workspace{}, {}{}",
            if dry_run { "would prune" } else { "pruned" },
            removed.len(),
            if removed.len() == 1 { "" } else { "s" },
            crate::ops::human(freed),
            if busy.is_empty() {
                String::new()
            } else {
                format!(
                    "; {} in use, skipped (prune again when it finishes)",
                    busy.len()
                )
            }
        );
    }
    for (project, error) in &failed {
        let _ = writeln!(
            err,
            "error[SAIL0505]: cannot remove the managed workspace {}: {error}",
            name(project)
        );
    }
    if failed.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(text: &str) -> Vec<String> {
        text.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn parses_list_and_prune_selections() {
        assert_eq!(parse(&args("list")), Ok(Parsed::List { json: false }));
        assert_eq!(parse(&args("list --json")), Ok(Parsed::List { json: true }));
        assert_eq!(
            parse(&args("prune --older-than 7 --missing --dry-run")),
            Ok(Parsed::Prune {
                selection: Selection {
                    all: false,
                    older_than: Some(Duration::from_hours(7 * 24)),
                    missing: true,
                },
                dry_run: true,
                json: false,
            })
        );
        assert!(matches!(
            parse(&args("prune --all --json")),
            Ok(Parsed::Prune {
                selection: Selection { all: true, .. },
                json: true,
                ..
            })
        ));
        // Pruning needs an explicit selection, and --all stands alone.
        assert!(parse(&args("prune")).is_err());
        assert!(parse(&args("prune --dry-run")).is_err());
        assert!(parse(&args("prune --all --missing")).is_err());
        assert!(parse(&args("prune --older-than")).is_err());
        assert!(parse(&args("prune --older-than soon")).is_err());
        assert!(parse(&args("list --all")).is_err());
        assert!(parse(&args("remove --all")).is_err());
    }

    #[test]
    fn ages_read_in_the_largest_whole_unit() {
        assert_eq!(ago(100, 100), "0m ago");
        assert_eq!(ago(4_000, 100), "1h ago");
        assert_eq!(ago(3 * 86_400 + 5, 0), "3d ago");
        assert_eq!(ago(0, 100), "0m ago");
    }
}
