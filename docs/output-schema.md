# `sail verify --output json`: schema version 1

`sail verify --output json [cargo verus verify arguments…]` verifies exactly
as `sail verify` does and then prints one JSON document describing every
verified function. This page is the reference for that document. The decisions
behind it are in ADR 0014.

- JSON Schema: `crates/sail-api/schema/verify-output-v1.json` (draft 2020-12).
- Rust types: `sail_api::output` (`Report` and the types it contains).
- An example document: `crates/sail-cli/tests/fixtures/attr/report.golden.json`
  (from `crates/sail-cli/tests/fixtures/attr/input.rs`).

## Invocation and streams

`--output json` (or `--output=json`) belongs to sail. It may appear anywhere
before a `--`. Everything else is passed to `cargo verus verify` unchanged and
in order. After a `--`, every argument belongs to Verus.

| Stream | Contents |
|---|---|
| stdout | Exactly one JSON document, followed by a newline. Nothing else. |
| stderr | Everything `cargo verus verify` writes to stderr, byte for byte, as it arrives (Cargo's status lines and Verus's diagnostics), plus any non-JSON text Verus printed on stdout. |
| exit code | `cargo verus verify`'s exit code, or 2 with a `SAIL01xx` error if sail could not set up. |

Under `--output json`, `cargo verus` writes stderr into a pipe rather than a
terminal, so colours are off unless you force them (`CARGO_TERM_COLOR=always`).
The bytes are otherwise the same as plain `cargo verus verify`, and the e2e CI
step checks this.

Without `--output json`, `sail verify` adds nothing: its output is
byte-identical to `cargo verus verify`.

### What `--output json` adds to the Verus command

sail appends these Verus arguments, after a `--`:
- `--output-json --time`, unless already present. Verus then prints its own
  per-function results (stdout, captured by sail).
- `--log smt-transcript --log air --log-dir <scratch>`, when attribution is
  possible (see [Attribution coverage](#attribution-coverage)).

None of these flags changes a byte of what Z3 receives. This is tested
against the pinned Verus (`crates/sail-verus/tests/attribution_verus.rs`).
`--log smt` does change Z3's input, and sail never adds it.

When Cargo considers every package up to date, Verus does not run.
`crates` is then empty and `attribution.notes` says why. To re-verify, change
a source file or run `cargo clean -p <package>`. Cargo does not re-verify a
fresh crate because Verus's arguments changed (`docs/verus-issues/VI-0005`).

## The document

All field names are `snake_case`. Optional values are present as `null`;
fields are never omitted.

| Field | Type | Meaning |
|---|---|---|
| `schema_version` | `1` | This schema. |
| `sail_version` | string | The `sail` that wrote it. |
| `mode` | string | `default`. A future opt-in, verdict-changing mode writes its own name here (charter rule 8). |
| `command` | string[] | The arguments passed to `cargo verus verify`, without sail's own flags. |
| `exit_code` | int \| null | `cargo verus verify`'s exit code. |
| `solver` | object \| null | `name` (`z3`), `version` (as Z3 reported it), `path`, `sha256` of the binary. |
| `cache.daemon` | bool | Whether solver sessions could be served from the cache daemon. |
| `attribution` | object | `status` (`complete`, `partial` or `unavailable`), `notes` (plain-language reasons), `unmatched_sessions`. |
| `crates` | array | One entry per Verus run (crate), in the order Verus finished. |
| `diagnostics` | array | Verus's diagnostics as data (see [Diagnostics](#diagnostics)). |
| `summary` | object | Counts over all functions: `functions`, `verified`, `failed`, `unknown`, `provisional`, `unstable`. |
| `error` | object \| null | `code` and `message` when sail could not run Verus at all. |

A crate has a `name` (from its functions' paths), `verus` (Verus's own
`verified`, `errors`, `success` and `encountered_vir_error`), and `functions`,
sorted by name.

### Functions

| Field | Meaning |
|---|---|
| `function` | The function's crate-qualified path as Verus names it: `demo::max`, `demo::arith::mul_pos`, `demo::Counter::incr`. |
| `module` | Its module as Verus reports it (`""` for the crate root). |
| `fn_mode` | `exec`, `proof` or `spec`. |
| `verdict` | `verified`, `failed`, `unknown` or `provisional`. See [Verdicts](#verdicts-and-tiers). |
| `tier` | `confirmed` or `provisional`. |
| `unstable` | Whether any of its answers could plausibly differ between plain Verus runs: a proof that used more than half its rlimit, or a give-up from resource exhaustion or near the limit (ADR 0009). |
| `verus_success` | Verus's own `success` for it, verbatim. Incomplete for bit-vector queries (`docs/verus-issues/VI-0003`). |
| `provenance` | `mode`; `source` (`fresh`, `cache`, `mixed` or `unrecorded`); `checks_solved`, `checks_served` and `checks_unrecorded`. |
| `timing` | `smt_micros` and `rlimit`, as Verus reports them. |
| `fragility` | Reserved. Always `null` until fragility is measured (seed portfolio). It will then be `{score, runs, proved}`. |
| `obligations` | Its obligations, or `null` if attribution was unavailable for it. |

### Obligations

One obligation is one solver query op that Verus ran for the function: an AIR
`check-valid`, sent to Z3 as one `(push)` … `(pop)` block, or as its own
process for a spinoff.

| Field | Meaning |
|---|---|
| `id` | The stable identity (next section). |
| `kind` | Verus's op kind: `Function-Def` (the body or a loop), `Spec-Termination`, `Check-Api-Safety`, `Function-Recommends`, `Function-Expand-Errors`; any of them may carry a `-Profile-Rerun` suffix. |
| `ordinal` | Its position among the function's in-module ops of this kind (0 for spinoffs). |
| `spinoff` | `null`, or `{index, reason}` for an op Verus ran in its own process (`reason`: `nonlinear`, `bitvector`, `profile_rerun` or `spinoff_all`). |
| `advisory` | `true` for recommends, expand-errors and profile reruns. Their failures are notes or warnings in Verus, not errors, and never fail the function. |
| `span` | Verus's span for the op (`text` verbatim, plus `file`, `line`, `column`, `end_line` and `end_column`). It is informational and not part of the identity. |
| `outcome` | `proved` (the first check answered `unsat`), `failed`, or `unknown`. |
| `unstable` | As for functions, for this obligation's checks. |
| `checks` | Every `check-sat` in the op, in order: `answer`, `rlimit_limit`, `rlimit_used`, `reason_unknown`, and `source` (`solved`, `cache`, or `null` if not recorded). The first check decides validity. Later checks are Verus locating further failing assertions. |
| `counterexample` | The first model Z3 gave for the op, if any: `check` (an index into `checks`), `model` (the `get-model` reply verbatim), and `assignments` (each zero-arity `define-fun` as `{name, sort, value}`). |

Verus encodes names in models. Parameter `x` is `x!`. Successive versions of
local `i` are `i@0`, `i@1`, and so on. Assertion labels are
`%%location_label%%N`, where `false` marks an assertion that failed in this
model.

## Obligation identity

```
<function>#<kind>/<ordinal>          an op run in the function's module process
<function>#<kind>/spinoff-<index>    an op spun off into its own process
```

For example: `demo::sum_to#Function-Def/0` (the loop),
`demo::sum_to#Function-Def/1` (the body after it), and
`demo::arith::mul_pos#Function-Def/spinoff-1`.

**What survives.** An identity contains no source positions, no bucket or
process order, and nothing derived from other functions. So it is unchanged
by:
- edits to other functions, including adding, removing or reordering them;
- edits anywhere that move the function within its file (comments, blank
  lines, new items above it);
- edits to the function's body that do not change its op structure (for
  example changing an expression).

`crates/sail-verus/tests/attribution_verus.rs` checks this against the pinned
Verus. It inserts a function and comments above everything, swaps two
functions, and edits a third function's body. Every identity is then
unchanged, although every span moved.

**What does not survive.**
- *Renaming or moving the function* (to another module or impl, or under a
  renamed crate): the path is part of the identity. The test checks this too.
- *Changing the function's op structure*: adding or removing a loop, or a
  `by (nonlinear_arith)` or `by (bit_vector)` block, renumbers that
  function's ordinals or spinoff indices from the change onwards.
- *Changing a function's verdict* can add or remove ops. For example, Verus
  runs a `Function-Recommends` op only after a `Function-Def` failure, so
  that obligation exists only while the function fails.

**Spans** are reported with every obligation so that tools can match across a
rename or move themselves. Matching by function path and kind, with the span
as a tie-breaker, recovers most refactors.

## Verdicts and tiers

- `verified`: confirmed, and proved. Verus said `success`, and no attributed
  verdict-bearing obligation failed.
- `failed`: confirmed, and not proved, and the failure is settled: Z3 found a
  model, or gave up far from its limit for its own reasons (for example
  `(incomplete quantifiers)`).
- `unknown`: confirmed, and not proved, because Z3 gave up at or near its
  limit, or no answer was recorded. Plain Verus reports such a function as
  failed (for example `rlimit exceeded`). sail never presents it as a settled
  failure (ADR 0009).
- `provisional`: not yet established by the pinned solver on the exact query.
  **A provisional result is never `verified`, whatever its evidence says**
  (charter rule 7). `sail verify` itself never produces provisional results.
  Every answer it reports came from the pinned Z3, fresh or from the cache,
  so its tier is `confirmed`. Speculative results (`sail watch`, MCP) will
  use this tier.

**Agreement with Verus.** `verified` holds exactly when Verus's own `success`
is true, with one deliberate exception. Verus's `success` stays `true` for a
function whose only failure is a bit-vector query (VI-0003), although Verus
reports the error and exits non-zero. When attribution shows the failed
bit-vector obligation, sail reports `failed`. This agrees with Verus's
diagnostics and exit code. The e2e CI step checks agreement with Verus's
`--output-json`.

## Provenance

Each check records how its answer was established (charter rule 7):
- `solved`: a fresh run of the pinned Z3;
- `cache`: a cache hit, which is always a reply the pinned Z3 produced for
  the same key (ADR 0008);
- `null`: no record. For example, the daemon could not be reached and the
  shim fell back to pass-through.

A function's `provenance.source` summarises its checks:
- `fresh`: all solved;
- `cache`: all served;
- `mixed`: some of each;
- `unrecorded`: at least one check has no record.

## Diagnostics

Each top-level `error`, `warning`, `note` or `help` block on stderr becomes
`{level, code, message, spans, rendered}`:
- `rendered` is the block verbatim (ANSI colours removed);
- `spans` lists its `-->` (primary) and `:::` locations.

Nothing is reworded. Diagnostics are not yet linked to obligations.

## Attribution coverage

Attribution needs Verus's logs. Verus deletes every file in its log directory
when it first writes there, and `cargo verus` gives every package and target
in a run the same log directory (`docs/verus-issues/VI-0004`). sail therefore
asks for logs only when one Verus process will write them: a single workspace
member with at most one lib or bin target, and no `--log` arguments of the
user's own.

Otherwise `attribution.status` is `unavailable`, its notes say why, every
function's `obligations` is `null`, and verdicts come from Verus's JSON alone.

`partial` means some processes could not be matched. The notes say which.
Verus can overwrite a spinoff's log when two ops of one function both spin
off (survey §6). `unmatched_sessions` counts cached solver sessions that
matched no log.

## Compatibility

Within schema version 1, fields are only ever added. A field is never
removed, renamed, or given a different meaning; any such change is version 2.
Consumers must ignore fields they do not know. The published JSON Schema is
strict (`additionalProperties: false`), so that sail's own tests catch drift.
Validate against the schema of the version you were built for.

With `--portfolio accept-first-proof`, optional `portfolio` evidence records the
ordered seeds, each independent attempt (report, exit, timing, infrastructure
error and verbatim stderr), and `winner_seed`. Every selected function's
provenance includes `configuration.seed`. Baseline seed 0 denotes unchanged
solver defaults. Ordinary reports omit these optional fields.

`--measure-fragility` explicitly measures solver replay configurations and writes
JSON (`--fragility-seeds N`, default 8). Each function's existing `fragility`
field is set only when its exact transcript input hash and ordered AIR checks
cover every primary obligation. `proved` counts configurations proving every
primary obligation together; `score = 1 - proved/runs`. It differs from the
census's per-check effort-pressure/disagreement metric. Missing mappings,
ambiguous sessions and incomplete replays leave the field null with notes.
Verdicts remain those of the baseline run. A replay score is not a complete-run
seeded proof; only the portfolio mode can select such a proof.

The MCP verify tool accepts `measure_fragility: true`. The existing fragility
tool also includes an optional `verification` report with the same function
scores. Normal verification performs no replay measurement.

When a crate fails and attribution is incomplete, successful timing entries
are reported `unknown` with a note (VI-0003). Their raw `verus_success` fields
are retained. Verus can leave that field true for failed bit-vector queries,
so it cannot establish a confirmed function proof in this case.
