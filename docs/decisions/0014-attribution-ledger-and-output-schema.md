# 0014: Function attribution, the ledger, and `--output json` schema v1

- **Status:** accepted; amended by [ADR 0044](0044-output-schema-v2.md)
  (schema version 2 renames `tier` to `establishment` and adds
  `evidence_tier` and ITF values);
  amended by [ADR 0045](0045-managed-verification-by-default.md)
  (`sail verify --output json` prints the project-result envelope of these
  reports)
- **Date:** 2026-10-08

## Context

The charter asks for three things:
- every query attributed to the Verus function and obligation that produced
  it;
- obligation identities that are stable, with exactly what survives
  documented;
- `--output json`: per-function verdicts with tier and provenance, timings,
  cache sources and counterexamples.

Function names never reach the solver pipe (ADR 0003). The owner decided
that attribution uses `--log smt-transcript` plus structure, never
`--log smt` in default mode, and that any flag changing Z3's input must also
be given to the conformance baseline (ADR 0010 item 3).

**Evidence** (pinned Verus `0.2026.10.04.426d8b0`, macOS arm64, 2026-10-08):
- **Pipe neutrality.** Through a recording solver wrapper, the per-process
  input digests were identical with no flags, with `--output-json --time`,
  with `--log smt-transcript`, with `--log air`, and with both logs. The
  inputs were `corpus/fixtures/toy/toy.rs` (1 process) and a 5-process
  crate with modules, a nonlinear spinoff and two bit-vector spinoffs. Only
  `--log smt` changed them. Stderr and exit codes were identical in every
  case. The source agrees:
  - transcripts are written beside the pipe (`air/src/smt_process.rs:169`);
  - AIR logs use their own emitters with no pipe
    (`air/src/context.rs:159-180`);
  - only the SMT log shares the pipe's emitter, and it adds indentation.

  `crates/sail-verus/tests/attribution_verus.rs` keeps checking this.
- **Structure.** Every AIR `check-valid` is preceded by `;; <Kind>
  <function>` and `;; <span>` comments. It becomes exactly one solver op:
  - a `(push)` … `(pop)` block holding one or more `check-sat`s, the first
    deciding validity and later ones locating further failures
    (`air/src/smt_verify.rs:497`, `context.rs:487-556`);
  - or, in a single-check spinoff, one `check-sat` with no push.

  An AIR log and a transcript of the same process share a file stem. So the
  k-th `check-valid` of `X.air` is the k-th op of `X.smt_transcript`. On the
  test crate, per-op rlimit sums match Verus's per-function `rlimit`
  exactly.
- **Transcripts reconstruct the pipe exactly.** Each batch, plus a newline,
  plus `(echo "<<DONE>>")`, gives the input byte for byte. Its SHA-256
  matched the shim's recorded input for all 5 processes.
- **Verus's own verdicts** (`--output-json --time`, `function-breakdown`) are
  per function. They are incomplete: `success` stays `true` when a function's
  only failure is a bit-vector query (VI-0003, reproduced).
- **Log directories are shared.** Verus deletes every file in its log
  directory on first use. `cargo verus` forwards the same `--log-dir` to
  every package and target it verifies (VI-0004). Changing forwarded
  arguments does not re-verify a crate Cargo considers fresh (VI-0005).

## Decision

**Sources.**
- Function verdicts, rlimit and solver time come from Verus's
  `--output-json --time`.
- Query-to-function mapping comes from `--log air` (names, op kinds, spans)
  zipped with `--log smt-transcript` (answers, effort, models). Both are
  pipe-neutral. `--log smt` is never used.
- A file whose AIR and solver op counts differ is not attributed, and is
  reported as such. sail never guesses.
- Cache provenance: with `SAIL_PROVENANCE_DIR` set, each cached shim session
  writes a record when it ends:

  ```json
  {"version": 1, "input_sha256": "…", "replies_served": N, "solver_started": true}
  ```

  `replies_served` counts the replies the cache served before a solver
  started, in the same units as the transcript's reply index (every
  output-producing command, including each `<<DONE>>` echo). The record is
  written only after every reply has been forwarded and the solver has
  exited, and failures are ignored, so it cannot affect the stream. Records
  join transcripts by input SHA-256.

**Obligation identity.**
- The identity is `<function>#<kind>/<ordinal>` for an op in the function's
  module process, and `<function>#<kind>/spinoff-<n>` for a spun-off op.
  `n` is Verus's per-op spinoff counter, taken from the log file name.
- It contains no spans, bucket order or other functions' data. So it
  survives edits to other functions, moves within a file, and body edits that
  keep the function's op structure.
- It does not survive renaming or moving the function, or changing its
  loops, nonlinear or bit-vector blocks.
- Spans are reported beside it, for tools that want to match across
  refactors.
- `docs/output-schema.md` documents this, and the Verus test checks both
  directions.

**The ledger (`sail-ledger`).**
- It holds per-function records: Verus's verdict, obligations, checks, effort,
  sources, the tier, and the mode.
- The reported verdict (`verified`, `failed`, `unknown` or `provisional`) is
  derived, never stored.
- A provisional record always reports `provisional`, so nothing provisional
  is ever reported as verified.
- A failed verdict-bearing obligation (`Function-Def`, `Spec-Termination` or
  `Check-Api-Safety`) fails the function. This closes the bit-vector gap.
- Recommends, expand-errors and profile-rerun ops are advisory.
- A give-up from exhaustion, or above 50% of the limit, is `unknown`, never a
  settled failure. This is the same threshold the shim applies to cached
  `unknown`s (ADR 0009).
- `sail verify` produces only confirmed results: every answer is the pinned
  Z3's, fresh or cached.

**Schema v1 (`sail-api::output`).**
- `docs/output-schema.md` documents it. The JSON Schema is
  `crates/sail-api/schema/verify-output-v1.json`, checked by a small
  validator in `sail-api::schema`.
- Fragility is reserved and always `null`.
- Within version 1, fields are only added, and the types ignore unknown
  fields.

**CLI.**
- `--output json` is sail's flag, accepted anywhere before `--`.
- sail appends `--output-json --time` and, when safe, the two log flags with
  a scratch `--log-dir`.
- stdout carries only sail's document. stderr is relayed byte for byte as it
  arrives. Any non-JSON text from Verus on stdout goes to stderr. The exit
  code is `cargo verus`'s.
- Logs are requested only when exactly one Verus process will write them: one
  workspace member, at most one lib or bin target, and no log flags of the
  user's own (checked with `cargo metadata --no-deps`). Otherwise
  attribution is `unavailable`, with the reason.
- Without `--output json`, nothing changes.

**Conformance baseline.**
- The flags change nothing Z3 receives, so the baseline needs no change.
- The harness already runs both sides with `--output-json --time`.
- The e2e step checks that `--output json`'s stderr and exit code are
  identical to plain `cargo verus verify`.

**Layering.** No new edges. These edges, already permitted, are now used:
- `sail-verus → sail-smtlib` and `sail-verus → sail-ledger`;
- `sail-api → sail-ledger`;
- `sail-cli → sail-api` and `sail-cli → sail-verus` for the report.

## Consequences

- **Agents get per-function and per-obligation results** with stable
  identities, provenance, effort and counterexample models, without parsing
  Verus's text.
- **Attribution is unavailable for multi-package workspaces and lib+bin
  packages** until Verus can give each process its own log directory. A
  per-package re-run, or a Verus option, would lift this (VI-0004).
- **A fresh crate produces no results** (Cargo skips Verus). The document
  says so. Forcing re-verification is left to the user (VI-0005).
- **Spinoff log collisions** can make attribution `partial`. Two ops of one
  function that both spin off overwrite one log (survey §6). Session records
  that match no log are counted.
- **Diagnostics are carried verbatim and parsed for spans**, but not yet
  linked to obligations.
- **The shim gains a best-effort side file.** Its forwarded stream is
  unchanged, and the cached-session test checks the record.

## Partial verification (2026-10-09)

When the caller explicitly selects a module, root or function, `sail verify`
uses `cargo verus focus`, as `sail fragility` already does. The pinned
`cargo verus verify` rejects partial selectors; `focus` is its supported
entry point. Compare such requests against plain `cargo verus focus` with
the same selectors and flags. Whole-project requests still use `verify`.
This also lets MCP function/module requests and their optional measurements
execute the requested scope. No scope is inferred or widened.

## Failing crates with incomplete attribution (2026-10-09)

VI-0003's real fixture shows that a failed bit-vector query can retain a true
function-success field. With incomplete attribution, the failing crate cannot
establish which successful timing entries are proofs. Preserve the raw fields
and diagnostics, but expose those uncertain entries as `unknown`, with a note.
Complete attribution keeps its precise obligation-based verdicts, and successful
crates keep their normal Verus results. The ordinary verifier stream and exit
code are unchanged. The old fallback test that left this gap open is replaced
with assertions that retain every function and refuse false proof claims.

## Ambiguous friendly function names (2026-10-10)

VI-0010 records that distinct trait implementations can share a friendly
function name, even inside one module. If the Verus timing breakdown contains
more than one entry for that name, Sail retains each original record but does
not join obligations to any occurrence. Attribution is explicitly partial and
raw logs remain available. Log order, ordinal renumbering and source spans do
not establish the missing implementation identity. Unique names retain their
existing obligation IDs. The duplicate-ID invariant remains unchanged;
successful whole-target results retain their existing authority, while failing
crates with partial attribution remain conservative.
