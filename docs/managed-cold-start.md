# Managed runs: first-run cost, reuse and timings

`sail verify` (managed verification, the default since
[ADR 0045](decisions/0045-managed-verification-by-default.md), and every
managed job from the CLI or MCP) builds
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

Contexts that differ only in the repair function selected share one Cargo
target ([ADR 0036](decisions/0036-managed-compiler-artifact-families.md)), so
a new repair function does not verify vstd again. Cargo reporting a target as
`fresh` is never enough on its own: a selected target is skipped only on an
intact receipt for the exact context and inputs. A dependency may also be
skipped on an intact receipt from another context of the same target that
covers its current bytes and inputs; the result names that receipt's run.

A run that ends in ordinary proof errors keeps vstd and the other compiled
dependencies, as plain `cargo verus verify` does, so the passing run after a
fix verifies only the edited crate. This needs a Current, unfocused run in
which every selected target's verifier finished normally with a complete
result. It holds for a project's first managed run too, which records the
build scripts' declared inputs: there Sail also requires that Cargo finished
its build and ran every build script it keeps a record for. Before this, a
project whose first runs failed verified vstd again on every run until one
passed (about 21–27 s each).

A run that ends in a compile error in a selected crate keeps them too, again
as plain Cargo does, when the run is Current and Cargo's own records show
that the failure is confined to that crate: Cargo started every selected
crate (so each of its dependencies had completed), every dependency verified
in that run succeeded, the failing crate's compiler stopped with an ordinary
error exit, Cargo finished the build, and Cargo ran (or found fresh) every
build script whose record is kept. On vest, a compile error then costs about
7 s instead of 34 s, and the fixed run 25 s instead of 52 s (plain
`cargo verus verify`: 6 s and 36 s; Sail's solver cache makes the fix faster).

A compile error in a dependency, a crash, an interrupted build, a partial
(`--verify-function`) run or uncertain inputs still withdraw the dependencies'
build-script records, and the next run rebuilds those packages. A failed proof
or compile error never produces a receipt, so the failing crate itself is
always verified again.

## Concurrent runs

Two managed runs that share a Cargo target (the same workspace and
arguments, whatever the repair function) are serialised. The
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
compiler again. Natively, these cut managed overhead around Verus on a
HumanEval single-function edit from 3.03 s to 0.94 s (plain `cargo verus`:
0.54 s), L7c (rescue lane report, tag `archive/rescue-m7-98f6292`).

Reuse works the same way on macOS. On an Apple M3 under other load:

| Project | Unchanged | Edit | Failing edit | Fix |
|---|---|---|---|---|
| one-function demo (load 14–16) | 0.35–0.47 s | 0.76 s | 0.67 s | 0.69 s |
| plain `cargo verus verify` | 0.17 s | 0.50 s | 0.43 s | 0.47 s |
| Vest `-p vest_lib` (load 7–9) | 1.6 s | 30.8 s | 18.0 s | 20.3 s |
| plain `cargo verus verify` (load 9–18) | 0.2–0.5 s | 28.0 s | 26.6 s | 31.6 s |

Before this release every macOS run rebuilt the macro dependencies and
verified vstd again (demo 22–72 s, Vest 38–44 s), whatever changed. See
[freshness](freshness.md#what-these-checks-do-not-catch) for the macOS rule
that changed.

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
