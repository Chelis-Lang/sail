# 0046: Managed verification is the only `sail verify`

- **Status:** accepted (owner decision 2026-10-15)
- **Date:** 2026-10-15
- **Amends:** [ADR 0023](0023-managed-incremental-workspace-verification.md)
  ("The default CLI … unchanged"; "ordinary verification remains
  available") and [ADR 0014](0014-attribution-ledger-and-output-schema.md) (what
  `sail verify --output json` prints)

## Context

- **Two paths.** `sail verify` ran a thin pass-through: `cargo verus verify`
  in the user's target with `sail-z3` in front of Z3, and nothing else. The
  managed runner (ADR 0023, ADR 0036, ADR 0041: Sail's own Cargo target,
  proof receipts that skip unchanged targets, complete per-target reports,
  durable jobs) ran only with `--incremental`, `--background`, jobs, `watch`,
  MCP and repairs.
- **Owner direction (2026-10-15).** "Why would we allow skipping past our
  core infra to run cargo and be knowingly slower? What is that for?" Then:
  "if its tech debt, id just scrub it out as a legacy config and default to
  fast by default. seems otherwise actively harmful to have a suboptimal
  default."
- **What managed printed.** Measured on a `cargo verus new` project with one
  passing and one failing function (macOS, Sail's Verus `73dea021`), managed
  verification differed from plain `cargo verus verify` on stdout in every
  run. It printed its own `Current evidence: …` line instead of Verus's
  `verification results:: 1 verified, 1 errors`, because the driver always
  asks Verus for `--output-json --time` and kept that document. A user's own
  `-- --output-json` or `-- --time` failed outright ("Option 'time' given
  more than once"). Stderr and exit codes were already Cargo's and Verus's.
- **What managed cost.** See [Measurements](#measurements).

## Decision

1. **One path.** Every `sail verify` runs the managed runner. The
   pass-through mode is removed from the product: no flag, environment
   variable or fallback runs it. `--incremental` is accepted, does nothing,
   and is documented as deprecated, so existing scripts keep working.
   `--background`, `--engine` and `--output json` keep their meaning. Jobs,
   `watch`, MCP and repairs already ran this path.
2. **Rule 1 output.** Without `--output json`, `sail verify` prints what
   plain `cargo verus verify` prints, from the same work:
   - stderr: Cargo's status lines and Verus's diagnostics, relayed as they
     arrive (unchanged);
   - stdout: Verus's own stdout. The managed driver passes `--output-json`
     and `--time` only when the user did not, and rebuilds the user's stdout
     from the document: the user's own `--output-json` document (without
     `times-ms` unless they asked for `--time`), or Verus's
     `verification results::` line under exactly the conditions Verus
     prints it (`rust_verify/src/verifier.rs`, `finish_verus`), followed by
     Verus's `--time` report when asked for. Cargo forwards that stdout; the
     runner relays every line that is not one of Cargo's JSON messages;
   - the exit code: Cargo-Verus's.

   Sail prints nothing of its own on stdout or stderr, apart from the
   existing one-line `warning[SAIL0003]` when the cache daemon is
   unavailable, one `note[SAIL3112]` line when a run gets no proof reuse
   (decision 4), and errors that stop a run. On a terminal, Sail asks Cargo
   for colour (`--color=always`, unless the user chose a colour setting), as
   plain Cargo would print there; Cargo's progress bar is not shown, since
   Cargo writes to Sail's pipe and Sail cannot learn the terminal width
   without unsafe code. Partial verification (`-- --verify-function` and the
   other selectors) under `verify` gets plain Cargo-Verus's own refusal and
   exit code ("Partial verification must use `cargo verus focus`"); with
   `--output json`, and in jobs and MCP's scoped `verify`, it runs focused
   and the partial report says so in its notes. `-v` and `--message-format`
   would show Sail's driver and flags in Cargo's output, so they get the
   plain Cargo-Verus path of decision 4. One ordering limit remains in
   combined output (`2>&1`): Verus prints its summary line after
   verification and before its compile phase, and Sail can print it only
   once Verus's document arrives at the end, so a compile-phase error after
   verification (a lifetime or erasure error) comes before the summary
   instead of after it; separately, each stream is unchanged, and on proof
   and compile errors the combined order matches (checked by the e2e legacy
   phase). `--output json` prints the
   `sail-project-result` envelope (a schema-v2 report per target) instead
   of a single schema-v2 report.
3. **What differs, and why it is not a rule 1 difference.** Sail's target is
   its own, and freshness there follows content and arguments:
   - the first run in a project, worktree or new argument set compiles the
     dependencies and verifies vstd in Sail's target, so it prints their
     `Compiling` lines and vstd's summary even when `target/` is current;
   - plain Cargo-Verus treats a crate as fresh when only its Verus arguments
     changed and prints nothing; Sail verifies again;
   - a target with a receipt but whose artifact Cargo found stale is
     verified, as Cargo does. A target Cargo finds up to date without a
     matching receipt (pruned receipts, an interrupted run) is not
     re-verified by `sail verify` without `--output json`: it prints what
     plain Cargo-Verus prints for an up-to-date target, and the next
     `--output json`, job or MCP run re-establishes
     the receipt (those retry once, cleaning that target).

   In each case Sail's output is exactly what plain `cargo verus verify`
   prints for the same verification work on the same target state.
   Verdicts and diagnostics are never reused from a failed or incomplete
   run: such targets get no receipt and are verified again, as Cargo does.
4. **Setups managed verification cannot bind fail safe; nothing plain
   Cargo-Verus accepts is refused (lead decision, 2026-10-15).** Managed
   verification owns the target and Verus's logs and binds every input it
   reuses. `--target-dir`, Verus's `--log` options, `--no-verify`, tests,
   benches and examples, `--sysroot`, a chained `RUSTC_WORKSPACE_WRAPPER`,
   Cargo `--config` and configuration it cannot identify, and a project
   with no `verify = true` target cannot be bound to a receipt. The runner
   reports them as `SAIL3112` before anything runs. `sail verify` then
   prints one `note[SAIL3112]` line naming the reason and verifies exactly
   as `cargo verus verify` does, in the user's own target, with `sail-z3`
   and the solver cache, reusing and recording nothing; with
   `--output json` it prints one schema-v2 report. Refusing them was a
   capability regression for the only `sail verify`. This is not a mode a
   user can choose: it is the fail-safe for invocations the runner cannot
   bind, announced every time. Jobs, `watch` and MCP, which must return a
   project result, still report them as `SAIL3112` errors, as they did
   before this change. The e2e legacy phase compares `--no-verify`,
   `--config`, `--tests` and `--target-dir` runs with plain Cargo-Verus.

   **Process-loader variables fail safe (lead decision, 2026-10-15).**
   `LD_LIBRARY_PATH`, `LD_PRELOAD` and the macOS `DYLD_*` loader variables
   change what the compiler and verifier load, which a receipt cannot bind,
   and `LD_LIBRARY_PATH` is common on Linux (CUDA, conda). Refusing them
   would lock those users out of the only `sail verify`. Such a run is
   verified as plain Cargo-Verus verifies it: Sail prints one
   `note[SAIL3112]` line on stderr, reuses no proof receipt, stages and
   publishes none, records no read check or compiler-family dependency
   records (the run is `unknown`), and does not retry a target Cargo finds
   up to date, which therefore has no report, as plain Cargo-Verus prints
   nothing for it. The loader variables are already part of the context, so
   such runs never share receipts with other runs. The solver cache stays on:
   its keys bind query bytes and solver identity. No check is weakened. The
   e2e managed phase compares such a run's output with plain Cargo-Verus's.
5. **Explicit experiments stay explicit.** `--measure-fragility` and the
   verdict-changing `--portfolio accept-first-proof` (with its single-file
   `--verus-source`) still run Cargo-Verus with `sail-z3` directly and print
   one schema-v2 report; they are measurements, not a verification mode, and
   refuse `--background` and `--engine` (`SAIL0110`).
6. **Conformance covers the default.** `sail-conformance run` compares, on
   its Sail side, Verus run as `sail verify` runs it: Sail's Verus, `sail-z3`
   and the logging flags the managed driver adds
   (`adapter::OBSERVATION_LOG_FLAGS`, one constant for both), with no daemon,
   then with the cache filling and warm (`ci/run.sh conformance`). The bare
   shim stays available as `--sail-arm shim`, test infrastructure for
   telling a logging-flag difference from anything else, like
   `--sail-verus stock` for the fork (ADR 0043). The corpus's fast tier is
   single-file Verus, which `sail verify` does not run, so the CLI itself is
   compared with plain `cargo verus verify` byte for byte by the CI
   `End to end` legacy phase (cold and warm, then `--output json` and MCP).

## Measurements

`scripts/perf-loops.sh`, three rounds, medians, on EC2 c7i.4xlarge
(us-east-1, idle, one timed run at a time), Sail's Verus `73dea021`. "Before"
is `main` at `4cd6fcaa`, whose `sail verify` was the pass-through; "after" is
this change. Each side is measured with its own plain baseline in the same
session. Seconds:

| Project, phase | plain `cargo verus verify` | before: pass-through | before: `--incremental` | after: `sail verify` |
|---|---:|---:|---:|---:|
| demo, edit | 0.78–0.79 | 0.81 | 1.07 | 1.07 |
| demo, failing edit | 0.56 | 0.58 | 0.84 | 0.83 |
| demo, unchanged | 0.11 | 0.11 | 0.32 | 0.30 |
| demo, cold | 32.1 | — | — | 33.2 |
| vest, edit | 39.6 | 25.4 | 28.7 | 28.5 |
| vest, failing edit | 35.3–35.4 | 25.6 | 28.0 | 27.9 |
| vest, unchanged | 0.22–0.24 | 0.21 | 1.34 | 1.11 |
| vest, cold | 71.0 | 72.1 | 74.6 | 74.4 |
| MCP `verify`, unchanged demo / vest | — | — | 0.52 / 1.91 | 0.52 / 1.67 |

Raw logs and tables are kept outside the repository in `l48-linux-perf.tgz`
(sha256 `df44617d7bf69b472b124203a0b4bbb880a79d6bd4de2539eed73a1df016fb33`)
and `l48-mac-perf.tgz` (sha256
`12fff5107e60d1f342342ca5b1ec563b555ffb896982f0c0a9c7e49c564c5466`).
(The `before` run's demo cold figures, 38–40 s for every mode, were
disturbed; the `after` run's are shown.)

**Cuts made here.** Matching each input file to the selection's package
directories scanned every package per file; ancestor lookups in sets take
the read-check record from about 250 ms to about 35 ms per vest run
(`managed/input.rs`, `Scope`). The dirty-receipt scan now parses two fields
instead of whole receipts (about 10 ms). Together: vest unchanged
1.34 → 1.11 s.

**What remains, and why** (phase timings from `SAIL_JOB_EVENTS`, c7i.8xlarge):

- **Small projects are about 0.28 s slower per run** (demo edit 1.07 s
  against 0.79 s, +35%). Of that, about 0.27 s is Sail's work around Verus:
  hashing the reused vstd artifacts that the receipt binds (about 90 ms),
  the targeted clean of the edited package (about 70 ms, a Cargo start),
  the second input snapshot (about 40 ms), the first snapshot and runtime
  identity (about 55 ms). Each is a check a receipt depends on.
- **Unchanged re-runs are slower than Cargo's no-op**: 0.30 s against
  0.11 s (demo) and 1.11 s against 0.22 s (vest). Every run hashes every
  input twice (before and after, about 0.3 s each on vest, ADR 0041) and
  every reused artifact, then returns the complete report. Cargo checks
  timestamps and prints nothing.
- **Solver-heavy edits are faster than plain Verus but about 3 s slower
  than the removed pass-through** (vest 28.5 s against 39.6 s plain and
  25.4 s pass-through). The managed driver asks Verus for AIR and SMT
  transcript logs for attribution (355 MB on vest, about 1 s to write;
  measured by adding the same flags to the pass-through), reads them (about
  0.3 s), and the runner's input and receipt checks take about 1.2 s.
- **Cold runs** are 1–3 s slower (demo 33.2 s against 32.1 s, vest 74.4 s
  against 71.0 s), for the same logs and checks; and Sail's target is its
  own, so a project whose `target/` is already built still pays one cold run
  (L47's pre-verified vstd addresses most of it).
- **A new set of Verus arguments is a new context**: its first run compiles
  the dependencies and verifies vstd again. Plain Cargo-Verus treats a crate
  as fresh when only its Verus arguments change, and prints nothing.

On this Mac (Apple silicon, shared with other lanes, load average 12–23),
the demo shows the same shape: edit 1.21 s against 0.60 s plain, unchanged
0.64 s against 0.38 s; the load makes these indicative only.

## Consequences

- **One product path.** Every user and agent gets receipts, complete
  reports, durable jobs and the cache; there is no slower mode to fall into.
- **Drop-in, with limits.** Projects that verify tests, benches or examples,
  or use `--target-dir`, Verus log options or Cargo `--config`, must run
  `cargo verus verify` without Sail until managed verification supports
  them. Charter appendix entry 6 records this against success criterion 3.
- **Disk.** Each project and argument set has its own target under the Sail
  cache (several GB for a mid-sized project); `sail cache projects` prunes
  them ([disk use](../workspace-hygiene.md)).
- **Output format.** `sail verify --output json` consumers get the
  project-result envelope; the requested targets' reports are the old
  documents. MCP and jobs already used it.
- **Remaining costs** are listed under Measurements, with numbers; L47's
  pre-verified vstd addresses the first-run cost.
