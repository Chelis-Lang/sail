# 0063: A seeded mutation suite as soundness probe and edit-loop benchmark

- **Status:** accepted (implemented; first results and remaining blockers in the
  [validation record](../validation/mutation-suite.md))
- **Date:** 2026-10-20
- **Credits:** the design is PR #205's proposal (its ADR, then numbered
  0054, "A seeded mutation corpus for Verus programs") and its FPBench pilot.
  This decision adopts that design and records what changed.

## Context

Conformance (ADR 0006, ADR 0009) compares Sail with plain Verus on pinned
projects that mostly verify, so the failure path is exercised far less. The
one outcome Sail must never produce is a function reported verified that
plain Verus rejects, and the likeliest way to produce it is a cache answer
served for a query that differs only slightly from one already seen.
Default-mode keys bind the exact solver input bytes (ADR 0008), so such a hit
should be impossible, but nothing tested it at scale.

The release matrix's edit-loop rows (`scripts/perf-matrix.py`) each use one
scripted edit per project. That is too narrow to stand behind an edit-loop
claim (#284). PR #205's pilot showed why: on a one-module-per-crate layout an
edit ended cache reuse for the rest of its solver session (ADR 0008, ADR
0011), giving about 1.0×, where a single scripted edit had suggested a gain.

Seeded program mutation tests both. PR #205 proposed the method: textual
rewrites of one function in nine named classes, ordered by a seeded hash so
a catalogue regenerates identically, with plain Verus as the only oracle. A
*rare-region* mutation changes behaviour for one input value, so its solver
queries differ very little from the original's, which is the case that
stresses keys. An equivalent mutant still verifies on both sides and is still
a valid comparison.

The pilot itself had flaws that this decision fixes:

| PR #205 pilot | This decision |
|---|---|
| Generator outside the repository, unpublished | `cargo xtask gen-mutants` in the repository, deterministic from a seed |
| 5 of 9 classes ran | All nine classes |
| One benchmark family (FPBench translations) | Real corpus projects: Vest, IronKV, CapybaraKV and human-eval-verus |
| `sail-bench edit` refilled the cache per mutant, so the cross-mutant check was never run | One daemon across every mutant, then the original is restored and must get its own answers |
| Medians and ranges only | Rotated rounds, medians and 95% bootstrap confidence intervals |
| Numbers from runtime `73dea021` | Measured on current main's release build and runtime |

## Decision

### Catalogues

- `cargo xtask gen-mutants --project NAME [--unit UNIT] --seed N
  --per-class K` reads only the fetched source of one corpus unit. It lexes
  the files the crate root reaches through `mod` declarations, finds every
  candidate site inside `verus! { ... }`, orders the candidates of each class
  by SHA-256 of the seed, project, file, class, offset and replacement, and
  takes up to `K` per class, at most one per function first. It adds no
  dependency (the lexer is in `xtask/src/mutants/`) and no crate-graph edge.
- A mutant is one exact replacement in the `corpus/edits.toml` format
  (`find` is the smallest window of whole lines around the change that occurs
  exactly once in the file), so `sail-bench edit --edits` and the mutation
  runner read the same file. Catalogues live in `corpus/mutants/<name>.toml`
  with a sidecar `<name>.meta.json` recording the generator version, seed,
  base commit, the sha256 of the generated catalogue, the candidate-site
  counts and, per mutant, its class, function, mode and location.
- The same seed, generator version (`gen-mutants 2`) and base commit give a
  byte-identical generated catalogue; its digest is in the sidecar.
- Generated mutants carry `expected = "pending"`. The workloads refuse
  pending mutants.

### Mutation classes

Each class is one rewrite inside one function body or one
`requires`/`ensures` clause; signatures and other functions are unchanged.
Functions marked `external`/`external_body` get no body mutants.

| Class | Rewrite |
|---|---|
| `arith` | binary `+`↔`-`, `*`↔`/`, `%`→`/` |
| `compare` | `<`↔`<=`, `>`↔`>=`, `==`↔`!=` (`<`/`>` only with spaces on both sides, so generics are never touched) |
| `constant` | a decimal literal `k` to `k + 1`, `k - 1` or `2k`, keeping its suffix |
| `logic` | binary `&&`↔`\|\|`; drop a unary `!` |
| `statement` | delete one `;`-terminated statement of an exec or proof body (never a `let`, `return`, `break` or `continue`) |
| `invariant` | delete one loop `invariant` item |
| `spec-weaken` | delete one `requires` item |
| `spec-strengthen` | conjoin `false` with one `ensures` item, or tighten one of its `<=`/`>=` to `<`/`>` |
| `rare-region` | in an exec function with `ensures`, an integer parameter and an integer, `bool` or unit result: `if p == k { return w; }` first in the body |

### Oracle

`sail-bench mutants oracle --catalogue FILE [--runs 3] [--jobs 4]` runs
stock `cargo verus verify` (the stock release, never the fork) on each
mutant in a persistent working copy, rewriting the file before each run so
Cargo rebuilds the crate. It is run on the reference platform (native Linux
x86-64). A mutant is dropped and listed with the reason when:

- any run does not finish within the limit: a run that does not finish is
  never an outcome;
- any run never reaches verification (it does not compile, Verus rejects it
  before verification, or Cargo does not rebuild it because the file is not
  compiled): such a mutant probes nothing;
- the runs disagree on the exit code, the outcome or the set of failing
  functions (ADR 0009).

The others get `expected` (`verified` or `errors`); the sidecar keeps every
run's outcome, exit code, failing functions and those that are new against
the original. Workers may share the machine because Z3 stops at an rlimit,
not a deadline; oracle timings are never performance results.

### Runner

`sail-bench mutants run --catalogue FILE [--rounds 3] [--per-class N]`
runs, per round:

1. a fresh working copy and an empty Sail cache, so `sail verify` starts one
   daemon that serves the whole round (its process id is recorded at every
   step);
2. `cold`: stock `cargo verus verify` and `sail verify` on the original;
3. for each mutant, in an order rotated by round: apply, run both, restore
   the file without verifying (the next mutant starts from the original, as
   an agent's next attempt does);
4. `revert`: both on the restored original;
5. `revert-forced`: the same with `SAIL_MANAGED_FORCE=1` (and the stock
   copy's crate rebuilt), so Verus runs and every check goes through the
   solver cache rather than a restored proof.

Both sides pass `--fwd-verus-args-to roots -- --output-json --time
--no-report-long-running`: only requested roots receive diagnostic logging
flags, so vstd still follows the installed product's shipped-proof path. The
quiet flag suppresses wall-time notices in Verus itself on both sides;
**no Verus diagnostics are stripped by the reader**. The intentional CLI
bundle-miss notice (SAIL3138, introduced by #275) is recorded separately from
Verus diagnostics; malformed, unknown or repeated notices remain failures.
Complete raw stderr remains in the logs. The quiet flag remains in
retained identity keys, like the existing JSON/time flags; it changes no proof
scope and cannot authorize a skipped check. Ordinary dependency output before
root JSON remains part of the comparison.

Both arms use the same source with separate Cargo targets and alternate order.
Every Sail run is judged against the same step's stock run under ADR 0009 and
ADR 0042, plus the owner-approved default differences. False verified is
counted per function entry, including repeated names, and on an erroneous
successful exit. Reverts also compare their function results with Sail's cold
original. The oracle and workload carry protocol `roots-json-strict-2`; older
qualification does not authorize this workload. The earlier candidate's
all-crate logging and progress-note stripping remain historical limitations.

Every sample goes to `samples.jsonl` as it finishes; none is dropped. The
report gives, per class and overall: median per-mutant wall time of both
arms, the ratio of summed per-mutant medians and the geometric mean of
per-mutant ratios, each with a 95% percentile bootstrap interval over
mutants (2,000 deterministic resamples); the share of solver replies served
by the cache; and the share of Sail runs with a newly published retained-proof receipt
(ADR 0050); no solver sessions alone does not establish instant revert. Per-round totals show the rotation. The command
fails on any difference, false verified, failed revert check or step that
could not run, and keeps those steps' logs under `failing/`.

### Tiers

The suite is local-only (ADR 0013): `ci/run.sh conformance standard` runs
two mutants per class of the human-eval catalogue for one round as a
soundness probe, and `ci/run.sh conformance deep` runs every committed
catalogue for one round. Timings are taken only on a native host (ADR 0038):
`ci/run.sh bench mutants NAME ROUNDS` there, three rounds for a release.

## Consequences

- The failure path and near-identical queries get systematic coverage in
  default mode, with stock Verus as the only oracle, and the cross-mutant
  check runs on every round.
- The release matrix's edit-loop rows rest on a breadth of edits, most of
  them failing, instead of one scripted edit; per-class served shares show
  which edits defeat reuse.
- Catalogues are generated, checked in and reviewable; regenerating the
  oracle needs a native Linux host (three stock runs per mutant).
- Mutants that change a signature, add code beyond the rare-region guard or
  need type inference are out of scope; the classes can grow by amending
  this decision, which bumps the generator version.
- Results on human-eval-verus are not evidence of generalisation; Sail has
  been measured on it. FPBench (PR #205's translations) can be added later
  as another base with its own generator.

The oracle and runner also accept `--vstd-lock FILE`: scratch copies pin Verus
crates to the release lock's registry versions. Omitting it retains source preparation for explicit diagnostic controls. The run records the setup and source adjustments; oracle
results must be regenerated for the selected setup. Every round requires the
same observed daemon PID. Retained-proof share requires a new published receipt,
not merely zero solver sessions. Interrupted commands keep their partial logs
and have their owned process-group descendants stopped.

Generator version 2 corrects the strengthening class: deleting an `ensures`
item weakens a postcondition and cannot stand in for strengthening it. Version
2 instead tightens a bound or adds a false conjunct to one postcondition.
Version 1's pilot catalogues and measurements remain historical evidence;
release coverage requires regenerated catalogues and a fresh stock oracle.
