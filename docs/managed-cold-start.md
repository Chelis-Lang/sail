# Managed runs: first-run cost, reuse and timings

`sail verify --incremental` (and every managed job from the CLI or MCP) builds
in its own Cargo target under the Sail cache and keeps a proof receipt for each
target that verified completely. The next run with unchanged inputs reuses
those receipts and starts no verifier. See
[ADR 0023](decisions/0023-managed-incremental-workspace-verification.md) and
[freshness](freshness.md) for what "unchanged" means.

## What the first run costs

A new proof context has no compiled dependencies and no receipts. Even a
one-function project first compiles its macro dependencies and verifies vstd,
which takes tens of seconds. A context changes when the arguments, the engine
selection, the relevant environment, the Cargo configuration or the installed
toolchain (Verus, Cargo, rustc, Z3, Sail, the sysroot libraries) changes;
installed files are identified by device, inode, size and modification time,
so reinstalling or rebuilding any of them makes the next run fresh.

Ordinary Cargo dependencies are shared by contexts that differ only in the
repair function selected ([ADR 0036](decisions/0036-managed-compiler-artifact-families.md));
proof receipts are never shared between contexts. Cargo reporting a target as
`fresh` is never enough: only an intact receipt for the exact context and
inputs skips verification.

## Concurrent runs

Two managed runs of the same workspace and context share a Cargo target. The
second waits for the first to finish (polling every 50 ms), for up to
`SAIL_MANAGED_LOCK_WAIT_SECS` seconds (default 1,800), and then fails with
`SAIL3104`. The daemon also queues jobs of one workspace behind each other.

## What a warm run costs

On a native `c7i.4xlarge`, a warm HumanEval run with nothing changed takes
under a second, and a single-function edit run spends about a second in Sail
around Verus (plain `cargo verus` spends about 0.5 s). That Sail work is Cargo
metadata, identifying the toolchain, the input snapshot and its stamps,
receipt checks and the final checks. Independent work overlaps: Cargo metadata
runs while the toolchain is identified, the two sysroot probes run in parallel,
and large files (input snapshots, receipt artifacts, the driver's AIR and SMT
logs) are read by up to eight workers. A private alias that is a hard link to
the installed `sail` is not hashed again, and the targeted `cargo clean` leaves
the target's rustc probe cache alone, so the next build need not probe the
compiler again. See the [L7c report](reports/rescue/l7c-managed-overhead.md)
for measurements.

## Optional setup timings

With `SAIL_JOB_EVENTS` set to a file path, each managed run also writes an
adjacent `*.setup-PID-SERIAL.jsonl` file of fixed phase names with relative
start and elapsed milliseconds: `runtime-context`, `metadata` (the remaining
wait for Cargo metadata, which overlaps the context), `inputs` (owned paths,
snapshot and input checks), `target-cleanup`, `invocation` (Cargo-Verus),
`target-records` (receipt and artifact checks, and any retry), `post-run` (the
second metadata, snapshot and final checks) and `publication`. The files hold
no arguments, paths, source, environment values or diagnostics, are created
exclusively with mode 0600, and are limited to 64 files per process and 64
rows and 16 KiB per file. Any failure disables them without changing the run.
Choose a destination outside the workspace, so writing timings does not change
an input.
