# 0046: Managed verification is the only `sail verify`

- **Status:** accepted (owner decision 2026-10-15); amended by
  [ADR 0047](0047-attribution-logs-on-request.md) (plain `sail verify` asks
  Verus for no attribution logs; the logging flags of item 6 are those of
  `--output json`, jobs and MCP)
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
   project result, still reported them as `SAIL3112` errors at first.
   *Amended 2026-10-16 (#165):* they now verify them the same way; the
   job's project result holds one target (`cargo verus verify`) with the
   run's schema-v2 report, a `no_reuse` object and a `SAIL3112` note, and is
   `unknown`, never `current`, binding no context or input revision. Only
   the opt-in interactive engine still reports `SAIL3112` as an error. The
   e2e legacy phase compares `--no-verify`,
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

## Amendment (2026-10-16): caller target-directory selection

`CARGO_TARGET_DIR` does not partition managed contexts (#166). Every managed compilation and
cleanup command explicitly selects the owned target with `--target-dir`,
which takes precedence over that environment variable. Managed-run metadata
probes select an owned metadata-only target under the Sail cache, overriding
both environment and Cargo configuration to keep source exclusions stable. Public snapshots for ordinary-Cargo fallback
retain the caller's target selection. No compiler artifacts are taken from the
caller's directory.

The environment variable still reaches child processes. Its presence and
exact bytes remain receipt inputs whenever rustc declares an `env!` read or
a build script declares `rerun-if-env-changed=CARGO_TARGET_DIR`. Such reads
still invalidate receipts and force Cargo re-verification. This exception
only removes redundant target selection from the context key; it does not
remove declared input validation or change unsupported-setup fallback.

Managed metadata probes, including Cargo-Verus's private metadata subprocess,
select `cargo-metadata/` under the Sail cache with a highest-priority
`build.target-dir` override. Neither caller environment nor Cargo configuration
can exclude source directories for these probes. Cargo metadata can otherwise create
`.rustc_info.json` in a source directory named by that override. Compilation
and cleanup keep the caller's environment value; an internal marker is removed
before compilation so a build script's own nested Cargo commands retain their
ordinary environment. Public fallback snapshots retain caller target selection.

Managed reuse additionally requires the physical Sail cache to be disjoint
from every resolved package input directory (#189). The check resolves
aliases and existing ancestors of future cache paths and runs before any
receipt read, dependency seeding or proof publication. Overlap in either
ancestor direction, including a project checked out under the Sail cache,
returns SAIL3112 for ordinary Cargo-Verus fallback without managed receipts.
This is a conservative unsupported-configuration boundary, not an expansion
of the input exclusions; the default external cache is unaffected.

## Amendment (2026-10-20): build-script contracts after a Historical run

A run whose workspace inputs change while it runs is Historical and records
no receipt. A log redirected into the project (`sail verify > verify.log`)
is enough. Until now such a run also withheld the compiler family's
build-script export (ADR 0047's script contracts). The next run then found no
export, treated every registry build script as unobserved, and ran
`cargo clean -p` on all of them: proc-macro2, quote, verus_prettyplease and
vstd. Cargo rebuilt nine crates, vstd was verified again and printed its
results, where plain Cargo-Verus was a no-op. With the log written on every
run this never ended (#259).

A Historical run with exit code 0 and no error now publishes the export, but
only when its changes left every script contract intact. That holds when every
path whose content differs between the run's before and after snapshots:
- lies inside the workspace;
- lies outside every package that has a build script;
- is not a path any recorded script declared.

The records must all be observed (covered). Declared variables and declared
paths are rechecked against each record when the export is published and
again when the next run reads it, as before. A run with a changed module
candidate (absence), an unknown currentness, or any other change keeps
today's behaviour.

The run itself stays Historical and records no receipt. The next unchanged
run is Cargo's own no-op with the same output as plain Cargo-Verus. The
output phase's `loop` checks this (`ci/test-output.sh`).

## Amendment (2026-10-26): target revisions and discovered inputs

Three full-path behaviours made `sail verify` rebuild or re-verify where plain
Cargo-Verus is a no-op. Verdicts were unaffected.

- **A receipt's input revision now covers what can change its target.** It
  hashed every file of every closure package, so an unreferenced new file made
  the crate's receipt changed, its package was cleaned and re-verified (#398).
  Adding a `src/bin` file did the same to the library. Sail now records each
  unit's dep-info declarations (`source-inputs.json`, per target). In a path
  package without a build script whose relevant units all have records, a
  target's revision counts only: the declarations of the target and, for a
  non-library target, of the package's library units; the declarations of
  every other closure package; manifests, lockfiles and toolchain files;
  anything under a `.cargo` directory; and the other spelling of a declared
  module file (`a/b.rs` and `a/b/mod.rs`). Every other package, every file
  outside package directories, and every package without complete records
  still counts in full. Rustc's dep-info lists every file it reads, including
  `include_*!` files, so a file outside this set cannot change the target's
  compilation; Cargo keeps the target fresh then too. Receipt reuse still
  checks the artifact and extern bytes, declared variables and the context,
  and every run still compares its whole snapshot for Currentness.
- **A vanished declaration leaves the record.** A declared input that no
  longer exists (a removed `src/bin` file) stayed in the record, so every later
  snapshot was incomplete, every run was forced, and the missing export made
  every build script count as unobserved: nine crates and vstd were rebuilt on
  every call (#401). A path that is definitely absent is now dropped when the
  record is read; its removal still changes the revision, and a file created
  again under that name enters the next snapshot by its content. A receipt of
  a target that Cargo's metadata no longer lists no longer marks its package
  changed.
- **A discovered compiler input does not withhold the export.** A run that
  first observes an `include_str!` file outside the workspace is Historical,
  since its before-run snapshot could not contain the file. The export rule
  above now also accepts a path that is new in the after-run snapshot and is a
  compiler-declared input, under the same script-package and declared-path
  exclusions (#399). The run itself still records no receipt.

The e2e `noop` phase compares each case with plain Cargo-Verus.

## Amendment (2026-10-26): ghost-only environment reads

Receipts bound only the variables the final dep-info declares, and Verus's
erased compilation rewrites that file without the verification phase's reads.
A variable read only in ghost code therefore bound nothing, and a proof
established with it set was reused as Current after it was removed (#386,
[VI-0036](../verus-issues/VI-0036-ghost-env-reads-missing-from-dep-info.md)).
Each verified target now requests the pinned fork's report of rustc's tracked
environment in both phases, appends the missing reads to the unit's dep-info
(variables the proof context binds excepted), and records them in its
invocation. A receipt (version 6) requires that record; replays of the shipped
vstd build keep their own recorded environment. See
[freshness](../freshness.md#which-environment-variables-are-proof-inputs).

The same holds for files (#439,
[VI-0037](../verus-issues/VI-0037-ghost-file-reads-missing-from-dep-info.md)):
a file only ghost code includes was not a declared input, so the per-target
revision above did not count it. The verification phase's local source-map
and tracked files from the same report are added to the unit's dep-info, with
rustc's checksum of the bytes it read under checksum freshness. Receipt
version 8 requires it.

## Amendment (2026-10-28): loader variables take the plain fallback

A run with a process-loader variable set (item 4) was verified inside managed
verification, in a target family keyed by the loader variable's value. It
reused and recorded nothing, so the family served no proof, yet every new
value started a cold one: the first loader run, and each change of the value,
rebuilt and re-verified every crate, vstd included, where plain Cargo-Verus,
which does not fingerprint loader variables, finds the project up to date and
prints nothing (#577). The output then differed from stock's for that call.

Outside the opt-in interactive engine, the runner now reports a loader
variable as `SAIL3112` right after its configuration check, before Cargo
metadata or any managed target is touched, and the run takes item 4's
fallback: verified exactly as `cargo verus verify` verifies it, in the
invocation's own Cargo target, with `sail-z3` and the solver cache, reusing
and recording nothing. It is the same plain run a loader run was always meant
to be, and it adds no new trust: no receipt, read check, dependency record,
replay record or dependency-store entry is written, and the loader variable
reaches Cargo-Verus unchanged.

The result shapes stay as they were for loader runs, as for an unqualified
Cargo (`SAIL3139`, ADR 0084): human output is Cargo-Verus's, with one
`note[SAIL3112]` line only under `SAIL_EXPLAIN=1`; `--output json`, jobs,
`watch` and MCP return one project result, `unknown`, with a `SAIL3112` note
and `no_reuse` (`verified_as` now `plain-cargo-verus`), whose one target holds
the run's schema-v2 report. The other `SAIL3112` setups keep their bare
schema-v2 report under `--output json`. With `--engine interactive`, which
reports the other `SAIL3112` setups as errors, a loader run is still verified
inside managed verification without reuse (`verified_as` `managed`).

The cost moves to the project's own target: the first loader run in a
project verified only with `sail verify` builds that target as a cold
`cargo verus verify` does. After that, unchanged loader runs and runs with a
different loader value cost what stock's do. Directional timings (wall seconds, one round,
native Linux x86-64 shared with other agents at load 30-37, host build with
no shipped vstd proof; "before" is `main` at `e1365eb2`), after a plain warm
`sail verify` in each copy (`cargo verus verify` for stock):

| Run (`LD_LIBRARY_PATH`) | demo stock | before | after | IronKV stock | before | after |
|---|---:|---:|---:|---:|---:|---:|
| first value | 0.40 | 91.2 | 95.4 | 0.30 | 50.0 | 72.2 |
| same value again | 0.50 | 0.83 | 0.30 | 0.27 | 0.47 | 0.32 |
| another value | 0.49 | 101.8 | 0.24 | 0.35 | 66.0 | 0.28 |
| failing edit | 2.84 | 1.66 | 1.10 | 21.7 | 24.5 | 13.9 |

The first-value row is the one-time build of the project's own target in a
copy that had never run `cargo verus verify`. Every "after" transcript but
that first one equals stock's (Cargo's `Blocking waiting for file lock`
lines and Verus's order of two failing functions' errors in IronKV aside);
"before" also differed for each new value. The e2e `loader` phase checks
the JSON result's new `verified_as`, its empty context and that no receipt is
recorded, and still compares human output with plain Cargo-Verus's.
`crates/sail-cli/tests/no_reuse.rs` covers every loader variable for human,
JSON and job runs.
