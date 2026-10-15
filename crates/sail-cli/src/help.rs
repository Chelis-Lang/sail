//! `--help` for every command: `sail COMMAND --help`, `sail COMMAND -h` and
//! `sail help COMMAND` print the command's usage and exit 0, without running
//! it. Only arguments before a `--` count, so `-- --help` still reaches
//! Verus. `sail bench` (a contributor tool, not in `sail --help`) keeps
//! delegating to `sail-bench`, which has its own.

use std::io::Write;
use std::process::{Command, ExitCode};

/// `sail verify`'s own options. Everything else goes to `cargo verus verify`.
pub(crate) const VERIFY_OPTIONS: &str = "\
SAIL OPTIONS (before --):
    --output json                 print one sail-project-result document on stdout
                                  (complete per-target reports, reused targets included)
    --background                  submit a job and print its id (`sail jobs`)
    --engine stock|interactive    engine (interactive needs `sail engine status`)
    --incremental                 deprecated, does nothing: managed verification is
                                  the default
    --measure-fragility [--fragility-seeds N]
                                  also measure proof fragility (changes no verdict)
    --portfolio accept-first-proof [--portfolio-seeds 1,2,...]
    --portfolio-timeout-secs N    opt-in seed portfolio (verdict-changing; docs/divergences.md)
    --verus-source FILE           with a portfolio, verify a single file with `verus FILE`
";

/// Help for the commands whose usage is fixed text.
const HELP: &[(&str, &str)] = &[
    (
        "repair-check",
        "\
Focus a repair on one function, then independently verify the full target
with the optional interactive engine (docs/proof-repair.md).

USAGE:
    sail repair-check --engine interactive --package P (--lib | --bin NAME)
                      --function CRATE::PATH [--background] [--output json]
                      [CARGO VERUS VERIFY ARGS] [-- VERUS ARGS]

Exits 0 when the repair is accepted, 1 when it is not, 2 on a setup or
service error (SAIL3120 when the engine is not installed).
",
    ),
    (
        "engine",
        "\
Inspect or install Sail's Verus: the runtime built from Sail's Verus fork
that every verification runs, which also contains the interactive engine
used by repair-check.

USAGE:
    sail engine status [--output json]
    sail engine install --from PATH [--output json]

`status` checks every runtime file against its pin. The release archives
bundle Sail's Verus beside `sail`, so it needs no setup. SAIL_VERUS_BUILD
names a build to use instead. In a Sail source checkout, download it with
`python3 ci/engine-artifact.py fetch --pin engine/verus/artifact-linux.json
--cache ~/.cache/sail` (artifact-macos.json on macOS), or build it with
`cargo xtask build-verus-engine`.
",
    ),
    (
        "explain",
        "\
Show the exact failure evidence for one function from a retained job.

USAGE:
    sail explain JOB --package-id ID --target NAME --kind lib|bin
                 --function CRATE::PATH [--obligation ID] [--output json]
",
    ),
    (
        "baseline",
        "\
Save, list and read immutable verification baselines.

USAGE:
    sail baseline save NAME --job JOB [--output json]
    sail baseline list [--output json]
    sail baseline get NAME [--output json]
",
    ),
    (
        "compare",
        "\
Compare two completed jobs, or a job against a saved baseline.

USAGE:
    sail compare BEFORE_JOB AFTER_JOB [--output json]
    sail compare JOB --baseline NAME [--output json]
",
    ),
    (
        "watch",
        "\
Re-verify the workspace (managed, like `sail verify`) 300 ms after files
stop changing. Each edit cancels the previous run. Ctrl-C stops.

USAGE:
    sail watch [--output jsonl] [CARGO VERUS VERIFY ARGS] [-- VERUS ARGS]
",
    ),
    (
        "jobs",
        "\
Inspect, follow or cancel local verification jobs.

USAGE:
    sail jobs list [--output json]
    sail jobs status|result|cancel ID [--output json]
    sail jobs logs ID [--follow]
    sail jobs events ID [--stream S --after N] [--output json | --follow --output jsonl]
",
    ),
    (
        "mcp",
        "\
Serve verification to agents over MCP on stdin and stdout (docs/mcp.md).
Takes no arguments; run it from the project directory, for example:

    claude mcp add --transport stdio sail -- sail mcp
",
    ),
    (
        "serve",
        "\
Run the Sail cache daemon in the foreground. `sail verify` starts one on
demand, so this is only needed to watch it or to choose its settings.

USAGE:
    sail serve [--socket PATH] [--cache DIR] [--memory-bytes N]
               [--idle-timeout SECS] [--shared DIR] [--shared-timeout-ms MS]
",
    ),
    (
        "status",
        "\
Show the daemon's and the cache's state.

USAGE:
    sail status [--socket PATH] [--json]

Exits 4 when no daemon is listening.
",
    ),
    (
        "fragility",
        "\
Measure how fragile this crate's proofs are across solver seeds and query
shuffles. Opt-in; changes no verdict.

USAGE:
    sail fragility [--seeds N] [--shuffles K] [--jobs J] [--near-limit F] [--top K]
                   [--timeout-secs S] [--json | --output json] [--keep-traces DIR]
                   [--traces DIR] [--focus] [CARGO VERUS VERIFY ARGS] [-- VERUS ARGS]
",
    ),
];

fn text(command: &str) -> Option<String> {
    match command {
        "verify" => Some(format!(
            "Verify a Cargo crate with Sail's managed verification: the output, verdicts,
diagnostics and exit code of `cargo verus verify`, with Sail's solver cache in
front of Z3 and proof receipts that skip unchanged targets. Sail builds in its
own Cargo target under the Sail cache; your `target/` is never touched.

USAGE:
    sail verify [SAIL OPTIONS] [CARGO VERUS VERIFY ARGS] [-- VERUS ARGS]

{VERIFY_OPTIONS}
All other arguments go to `cargo verus verify`, run with Sail's own Verus
(built from Sail's Verus fork and shipped with Sail); its help follows when
that Verus is installed (`sail engine status` checks it).
"
        )),
        "cache" => Some(crate::ops::CACHE_USAGE.to_owned()),
        "new" => Some(crate::new::USAGE.to_owned()),
        "cache projects" => Some(crate::projects_cli::USAGE.to_owned()),
        _ => HELP
            .iter()
            .find(|(name, _)| *name == command)
            .map(|(_, text)| (*text).to_owned()),
    }
}

/// The help topic for a command line (`args[0]` is the command): the command,
/// or `cache projects`, which has help of its own.
pub(crate) fn topic(args: &[String]) -> String {
    match args {
        [cache, projects, ..] if cache == "cache" && projects == "projects" => {
            "cache projects".to_owned()
        }
        [command, ..] => command.clone(),
        [] => String::new(),
    }
}

/// Whether `args` (a command's arguments) ask for help before any `--`.
pub(crate) fn requested(args: &[String]) -> bool {
    args.iter()
        .take_while(|arg| arg.as_str() != "--")
        .any(|arg| arg == "--help" || arg == "-h")
}

/// Prints `command`'s help. `None` when `command` has no help of its own.
pub(crate) fn print(command: &str, out: &mut dyn Write) -> Option<ExitCode> {
    let body = text(command)?;
    let _ = write!(out, "{body}");
    if command == "verify"
        && let Ok(cargo_verus) = crate::verify::cargo_verus()
    {
        let _ = writeln!(out, "\n`cargo verus verify --help`:\n");
        let _ = out.flush();
        if let Ok(output) = Command::new(cargo_verus)
            .args(["verus", "verify", "--help"])
            .output()
        {
            let _ = out.write_all(&output.stdout);
        }
    }
    Some(ExitCode::SUCCESS)
}
