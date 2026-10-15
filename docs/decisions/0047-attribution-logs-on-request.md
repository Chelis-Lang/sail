# 0047: Attribution logs on request, and cheaper managed checks

- **Status:** accepted (lead decision for the owner's "certainly we don't
  want a perf regression", 2026-10-15)
- **Date:** 2026-10-15
- **Amends:** [ADR 0014](0014-attribution-ledger-and-output-schema.md)
  (when Sail asks Verus for `--log air` and `--log smt-transcript`) and
  [ADR 0046](0046-managed-verification-by-default.md) (the managed driver's
  flags, item 6)

## Context

- **The regression.** With managed verification as the only `sail verify`
  (ADR 0046), a vest edit took 28.5 s against 25.4 s for the removed
  pass-through (plain stock Verus: 39.6 s). ADR 0046 attributed about 1 s to
  writing 355 MB of AIR and SMT transcript logs on every run, 0.3 s to
  reading them, and 1.2 s to input checks. The owner asked whether
  per-function attribution logs are needed on every run, or could be
  produced later.
- **What needs the logs.** Verus's own `--output-json --time`, which the
  driver asks for on every run anyway (to rebuild Verus's stdout and to bind
  receipts), already gives every function's verdict, rlimit and solver time.
  The logs add per-obligation records: op kinds, spans, per-check answers,
  rlimit use, cache provenance (the shim's session records join
  transcripts) and counterexample models. Consumers:

  | Consumer | Needs from the logs |
  |---|---|
  | `sail verify` without `--output json` (stdout, stderr, exit code) | nothing: Verus's own output |
  | Proof receipts and Currentness | nothing: a receipt needs a passing target, whose functions are all verified by Verus's JSON |
  | `sail verify --output json`, jobs, `--background`, `watch`, MCP `verify`, `verification_result`, `counterexample`, `explain_failure`, `sail explain` | obligations, failing spans, counterexamples, `unknown` for resource give-ups, per-check provenance |
  | Repairs (interactive engine) | the focused phase's obligations (`logs/focus`) |
  | `--measure-fragility`, `--portfolio` | their own logs (they never ran the managed driver) |

  Every reader of obligation detail is a structured caller: the daemon's
  job worker runs `sail verify --output json`, and MCP, `sail explain` and
  `watch` read jobs. Plain `sail verify` prints only what Verus prints; its
  report is used for receipts and is otherwise never shown.
- **Where the rest of the time went** (vest `-p vest_lib`, EC2 c7i.4xlarge,
  phase timings from `SAIL_JOB_EVENTS` and experiments on the pass-through
  build, interleaved, three rounds each):
  - the logs cost 0.6–0.8 s per edit, not 1 s; `--output-json --time`
    costs nothing measurable;
  - Sail's targeted `cargo clean -p` of the edited package deletes rustc's
    incremental state, which plain Cargo keeps across edits: the
    pass-through with that clean added took 24.8 s against 23.7 s, and
    with the incremental directory kept, 23.9 s;
  - the "double hashing" was mostly not hashing: one input snapshot of
    11,600 files took about 250 ms, of which reading and hashing every file
    was 65 ms, resolving every path with `realpath` 130 ms, and three
    `lstat` passes the rest.
- **The fork alternative.** A Verus-fork option writing a compact
  attribution sidecar (function, kind, span, answer, effort, model per op)
  would make structured runs cheap too. Its gain is bounded by what the
  logs cost there: the measured 0.6–0.8 s per vest edit for `--output json`,
  jobs and MCP, and nothing for plain `sail verify` after this change.

## Decision

1. **Logs only when the report is read.** The managed driver asks Verus for
   `--log air --log smt-transcript --log-dir <sidecar>` only when the run's
   caller reads its report: `sail verify --output json` (and therefore
   jobs, `--background`, `watch` and MCP), repairs, and every
   interactive-engine run (`managed::Config::attribution_logs`, carried to
   the driver in the run's runtime record). Plain `sail verify` asks for
   neither log, and sets no `SAIL_PROVENANCE_DIR`. No new flag or
   environment variable selects this: the output format decides.
2. **Reports without logs say so.** Without logs, a target's report is
   built from Verus's JSON alone: `attribution.status` is `unavailable`,
   its first note says the obligation detail was not recorded by the run
   that established the report, every `obligations` is `null` and every
   provenance `source` is `unrecorded`. Verdicts follow the existing ledger
   rules, so a passing target's functions are `verified` exactly as with
   logs (a target with no Verus error has no failing verdict-bearing
   obligation), and a failing target keeps the incomplete-attribution rule
   (VI-0003: successful timing entries in a failing crate are `unknown`).
   Such a failing report is never shown and gets no receipt. A passing
   target's receipt carries the report, so a later `--output json` or MCP
   run that reuses it shows the note rather than obligations, until the
   target is verified again with logs; it never shows invented detail.
3. **No lagging re-run.** A background re-run of failing targets with logs
   is not built. Failing targets are the only ones whose detail matters,
   their readers already run with logs, and a re-run would repeat Verus's
   frontend (about 20 s on vest) and need report amendment in receipts and
   jobs for detail nobody requested.
4. **Cleans keep rustc's incremental state.** Sail's targeted cleans move
   each `<profile>/incremental` directory of its target aside around
   `cargo clean -p` and put it back (`managed/runner.rs`, `clean`). The
   state is a cache that rustc checks against the source contents and the
   compiler options before using; plain Cargo keeps it across edits. A
   directory that cannot be restored is deleted, which only loses the
   cache. What the clean forces (Cargo compiles and Verus verifies the
   package again) is unchanged.
5. **The input walk resolves only symbolic links, and stamps overlap their
   `lstat` calls.** A directory entry that is not a link is already its own
   canonical path. The snapshot's files, digests, roots, stamps and
   revisions are identical (a unit test compares the walk with the old one
   on a tree with file and directory links, a cycle and a dangling link);
   ADR 0041's rules are unchanged: every input is read in full before and
   after the run, and the stamps cover the same paths.
6. **Conformance.** `sail-conformance`'s Sail arm keeps the logging flags
   (`adapter::OBSERVATION_LOG_FLAGS`): it covers `--output json`, jobs and
   MCP, and the flags are pipe-neutral (ADR 0014). Plain `sail verify` now
   runs Verus exactly as the bare-shim arm does (`--sail-arm shim`), and the
   e2e legacy phase compares it with plain `cargo verus verify` byte for
   byte; the e2e managed phase checks that it writes no logs and that a
   later `--output json` run reuses its proof with the note.
7. **The fork sidecar is proposed, not built.** It would remove the
   0.6–0.8 s from structured runs on vest. The owner's rule for fork changes
   is a measured gain first; this records the bound, and a fork change
   needs its own measurement on the fork build.

## Measurements

MEASUREMENTS

## Consequences

- **Plain `sail verify` writes no logs**, so it is no longer slower than the
  removed pass-through on solver-heavy edits by the logs' cost, and its
  sidecars are a few files instead of hundreds of megabytes.
- **Structured callers are unchanged**: `--output json`, jobs and MCP pay the
  logs as before and get full obligation detail for every target they
  verify. A target reused from a plain run's receipt reports Verus's
  verdicts, rlimit and time, with a note instead of obligations.
- **Rustc incremental state persists in Sail's target**, as in the user's
  own target; `sail cache projects prune` removes it with the rest.
- **What remains** is listed under Measurements: the receipt and
  Currentness checks (ADR 0041) and, for structured runs, the logs.
