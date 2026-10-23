# Divergences

Registry of opt-in modes that can produce a verdict plain Verus would not
(charter rule 8). Each entry names the flag, what can differ, how output labels
it, and how its cache keys are kept apart from default-mode keys.

The following portfolio mode requires explicit selection. Owner-approved
default-mode differences are recorded separately below.

| Mode | Flag | What can differ | Output label | Key separation |
|---|---|---|---|---|
| accept-first-proof | `sail verify --portfolio accept-first-proof` | An independent seeded Verus run may prove a scope that the baseline cannot. A single complete run must prove the entire requested scope. | Human output labels the mode and selected seed; JSON includes every attempt and selected configuration. | `Mode::OptIn("accept-first-proof")` prefix keys, negotiated connection mode, mode-bound disk and memory entries. Shared reads and publishing are disabled for these entries. |

## Accepted default-mode differences

Default mode runs Sail's Verus fork (ADR 0043). One fork fix makes Sail
stricter than stock in stock's per-function JSON status, and the
owner accepted it on 2026-10-20:

| Difference | Mode | What differs | What stays identical | Recognised by |
|---|---|---|---|---|
| [VI-0003](verus-issues/VI-0003-bit-vector-failure-reported-as-success.md): a function whose `by (bit_vector)` assertion failed | default (fork fix `8a5d3655`) | Verus's `--output-json` timing entry: stock `success: true`, Sail `false` (Sail is stricter, never more permissive) | Diagnostics (`error: bitvector assertion not satisfied`), error counts, every other function and the exit code | `sail_conformance::compare`: counted under `accepted`, not as a difference, only when everything else is identical, stock failed, the flip is stock `true` to Sail `false`, and the unique function and assertion location match the recorded Vest reproduction; missing exit status or ambiguous function identities refuse the exception |

The mutation suite found it on Vest (`vest-logic-003`,
[record](validation/mutation-suite.md)). Any other status flip is still a
difference.

## Accepted default-mode progress ordering

Owner decision, 2026-10-21: complete standalone `note: verifying module ...`
blocks may reorder within one uninterrupted run, including notes naming a
function. Their complete bytes and counts must match; other output and all
verdict and exit-code checks remain exact. Diagnostic-message, summary and
other-output boundaries cannot be crossed. This addresses the stock scheduling
defect [VI-0012](verus-issues/VI-0012-filtered-progress-note-order.md); Sail's
fork already emits these notes deterministically. Corpus comparisons record
the accepted ordering separately. See [ADR 0064](decisions/0064-verification-progress-note-order.md).

The explicit `--engine interactive` source engine also adapts supported Cargo
`no-vstd = true` metadata to the launcher's equivalent literal option (VI-0011).
Stock Cargo otherwise chooses the wrong macro mode before decoding that same
requested option and can fail before checking any proof. This adaptation is
labelled in the project result's notes and the target's `launcher-options.json`.
It was checked against the actual launcher with identical Cargo argument
bytes and the same effective `--no-vstd`: full diagnostics, function verdicts
and complete SMT input agreed. Independent full target verification and final compiler checks
remain required. Engine receipts bind the engine manifest, Sail binary and
Cargo configuration; solver entries still bind the exact actual input bytes and
solver configuration. See `docs/verus-issues/VI-0011-cargo-no-vstd-launcher-mode.md`.

The source engine is not a verdict-changing mode: its reports keep
`mode: default` and record `engine` provenance, and `sail-conformance engine`
compares its full phase with plain Verus. It has no opt-in options: it
prepares no lowering for reuse.

Owner decision, 2026-10-21: ignore the six random temporary SMT-log-directory
characters in the exact pinned dispatch query regression's printed rerun
commands. This applies only to its source-hash-bound test scaffolding; query
bytes, diagnostics and outcomes remain checked. See ADR 0042.
