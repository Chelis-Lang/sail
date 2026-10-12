# Proof events and repair validation

**Experimental (M7):** the selected source engine passes its preparation-memory
and lowering qualification. Current `387ba80` passes the
[complete gate, original solver, product installations and original CLI/MCP
repair fixture](validation/m7.md).
The `57a319d` named-baseline feature smoke and
[b895 full reader/retained workflow](validation/m7.md)
remain separate historical passes. Current readers, real-project performance
and complete standard/cache corpus coverage remain unfinished.
See [known issues](known-issues.md) and the
[acceptance guide](bench/m7-repair-acceptance.md) before relying on performance
or cross-edit preparation reuse. Stock verification remains the default.

For setup through a focus/full regression, resumable events, cancellation and
CLI/MCP evidence, follow the [repair walkthrough](proof-repair-walkthrough.md).
It distinguishes submission from accepted proof and engine identity checks from
unfinished M7 acceptance.

The optional `interactive` engine is a Sail-managed build of the pinned Verus
source. It is selected explicitly. Stock verification retains its existing
behavior. An unavailable or invalid engine returns a setup error (`SAIL3120`).
An internal runtime failure retains its failed evidence and falls back to stock
full verification; missing focused evidence cannot be fabricated or labelled a
successful batch. A target whose engine driver crashed, produced malformed
output, or sealed without normal termination or inconsistently with its exit
status and output reports the exact stock result with
`engine.established_by: stock-fallback`, publishes no proof receipt and cannot
accept the repair. A fault before any target could run falls back to an
independent stock full verification and records `batch.engine_fault`
(`SAIL3123`); the repair is not accepted. Either fault removes persistent
preparation entries when that opt-in is enabled. Inspect `batch.batched`,
focus, selection errors and `engine_fault` alongside the full report, whose
reports and functions name their `engine` provenance
([output schema](output-schema.md#engine-provenance)).

Inspect or register an already built validated engine:

```sh
sail engine status --output json
sail engine install --from PATH --output json
```

`PATH` is an engine-specific Sail source variant with a valid build manifest;
installing a stock or modified unrecognized build is rejected. The install
records provenance and leaves stock pins and installs unchanged.

For a library target:

```sh
sail repair-check --package demo --lib --function demo::arith::lemma \
  --engine interactive --background --output json
sail jobs events JOB --follow --output jsonl
sail jobs result JOB --output json
```

Keep redirected submission, event and result files outside package roots and
explicitly declared inputs while verification runs. Those files are operator
records, and writing them inside a watched package can invalidate input
freshness. The walkthrough uses a separate sibling capture directory.

For a binary, replace `--lib` with `--bin NAME`. Function names are exact,
crate-qualified identifiers. This command first checks that selected function,
then independently checks the full requested target in the engine's compiler
batch. It does not change what existing `verify --function` means. Additional
scope filters are rejected because they would defeat full validation.

The repair-artifact isolation candidate gives each exact selector its own
dependency build generation. Initial setup can compile dependencies again and
uses additional cache space, but another selector cannot overwrite the artifact
bytes tied to that selector's proof receipts. Stock and full-engine calls retain
their existing shared compiler-family target. This source change is not yet
native-qualified; see the [candidate and validation requirements](validation/m7.md).

The version-one repair bundle retains separate `batch.focus` and `batch.full`
project results, engine manifest identity and actual compiler-invocation count.
`batched` describes actual engine support. `accepted` is recomputed from the full
report: successful, complete requested targets, whole-target verification and
Current input evidence after compiler completion. A passing focus phase remains
provisional. A failing full phase, late compiler failure, edited inputs or
incomplete input coverage cannot authorize a repair.
A missing or ambiguous exact function records `batch.selection_error` with a
stable code and prevents acceptance while retaining the independent full report.

Optional ordinary-build recovery passes its local controls; real-project
benchmark recovery remains unqualified. It independently guards script inputs
during initial
regeneration and can retain only dependency contracts. The proof still needs
its own complete Current full result; an Unknown run remains unaccepted.
Unsupported paths, configuration, environment or coverage retain original
cleaning. See [build recovery and limits](managed-cold-start.md).

The private worker exits with the full phase's status so a completed proof is
not mistaken for an infrastructure failure; the public repair command exits
nonzero when the repair is not accepted.

Prepared-lowering reuse stays inside one compiler invocation by default: each
engine driver gets a private preparation directory, removed when it exits, so
the focused phase can serve the same driver's full phase but nothing is reused
across invocations or edits. Persistent cross-edit reuse under
`<cache>/engine-prepared` is an explicit opt-in:

```sh
SAIL_ENGINE_PERSISTENT_PREPARATION=1 sail repair-check ... --engine interactive
```

Its key binds Verus's pruned crate and a selected argument tuple, and a real
hit is not compared with fresh lowering at runtime, so correctness would depend
on that key's dependency coverage (charter rule 6). The opt-in is part of the
proof context (its results and receipts never mix with default ones) and every
result reports `engine.persistent_preparation: true`.

Optional `batch.preparation` records retain source-reported counters separately
for each observed Verus driver. `batch_hits` counts restores from the in-memory
entry; `persistent_hits` counts restores from validated files. A file may have
been written earlier in the same batch, so this is not an across-edit-only count.
Across-edit reuse is established by the retained sequence of source revisions. Misses, retained
byte peaks and configured budgets are source observations, not proof evidence or
process RSS. Peaks and budgets from independent drivers are not summed. Target
and phase identities appear only when independently bound. An engine without
this telemetry reports it unavailable, rather than supplying invented zeroes.

## Engine conformance

`sail-conformance engine` is the engine's corpus arm. For each unit it runs
plain Verus and then the engine with a focused function (the first proved
function of the plain run) and an invocation-local preparation directory, and
compares the full phase with the same rules as the other arms, including the
ADR 0009 unstable classification. It also requires a normal, consistent
compiler seal, and runs the same-input cached-versus-fresh byte oracle on every
restored preparation hit. An uncovered hit fails the unit. An oracle mismatch
is confirmed first (ADR 0038, amendment of 2026-10-12): in the same compiler
callback, the oracle lowers the bucket again from independent owners of the
same input (`--oracle-fresh-arms N` confirming arms, default 1, at most 8) and
compares each with the first fresh lowering. If one differs, stock lowering is
nondeterministic there (VI-0015): the unit is `unstable`, provided an engine
run without the oracle, with the same focus, matches plain Verus. If all
agree, or the runtime records no confirming arm, the mismatch is a hard
failure. Fresh lowerings from separate processes are never used as evidence:
stock declaration order can vary between processes but not within one
(VI-0001), which would excuse a real defect. The summary line reports oracle
failures as hard and unstable counts.
`ci/run.sh conformance` runs it after the cache rounds on every tier, using
`SAIL_ENGINE_BUILD`:

```sh
ci/local.sh conformance fast pr        # includes the engine arm
sail-conformance engine --tier standard --project vest --engine-build PATH
sail-conformance engine --tier fast --filter /guide/ --oracle-fault effects # negative control
```

`--no-oracle` and `--no-focus` narrow a diagnostic run. `--oracle-fault`
injects an `order`, `effects` or `body` fault into the oracle's restored arm and
passes only if every unit with a restored hit reports the mismatch and at
least one mismatch is confirmed hard (confirming fresh lowerings agree). Use
`effects`: it changes every restored arm. `order` swaps the first two lowered
functions, which leaves the solver bytes of many buckets unchanged, and `body`
needs a constant spec-function body, so neither is a complete control. The
oracle mismatches on `guide/traits.rs` in most runs because stock trigger
choice varies within one process (VI-0015); those mismatches are classified
unstable when a confirming fresh lowering differs, and hard otherwise.
Confirming arms need a runtime whose oracle records `second_fresh`; with an
older runtime every mismatch stays hard.

## Resumable observations

`jobs events` reads a separate proof journal, independent of the existing target
lifecycle log. Each version-one event includes stream ID, monotonically assigned
sequence, package ID, target name and kind, run ID, configuration and input
revision, phase, and optional exact function/obligation identifiers. The daemon
binds producer headers to the originating worker's context and target inventory.
Query progress carries bounded descriptions and completion flags with exact
function/obligation identities. It supplies no solver answer, model or verdict;
completion is not verification. Live function observations are provisional; final reports independently establish
proofs. Solver outcomes in obligation observations do not confirm a whole
function or target.

Resume using the last cursor record:

```sh
sail jobs events JOB --stream STREAM_ID --after 12 --follow --output jsonl
```

Identical producer event IDs are delivered once; conflicting duplicates expose a
gap. Wrong-stream or future cursors fail explicitly. Malformed, truncated,
replaced, or oversized streams remain incomplete, with an explicit gap rather
than invented evidence. Each page has at most 64 events. A journal retains at
most 4,096 events or 16 MiB; reaching either bound exposes a gap. Pruning a job
also prunes its journal. A terminal page means both the producer and available
observations have drained. JSONL includes cursor/control records even when no
new observation is available. `--output json` reads one page for programmatic
clients; `--follow` requires JSONL.

## Evidence and comparisons

```sh
sail explain JOB --package-id PACKAGE_ID --target demo --kind lib \
  --function demo::arith::lemma --output json
sail baseline save before-repair --job JOB
sail baseline list
sail baseline get before-repair --output json
sail compare LATER_JOB --baseline before-repair --output json
sail compare BEFORE_JOB AFTER_JOB --output json
```

`explain` preserves the original obligation reports, counterexample models,
provenance, and raw diagnostics. Add `--obligation EXACT_ID` for one attributed
obligation. Producer-linked diagnostics use exact identities; other diagnostics
remain explicitly unattributed. Source locations come from verifier evidence,
not guessed line windows. Same-name library and binary targets remain distinct.

Named baselines are immutable snapshots retained independently of the 20-job
history limit. Their establishing run and revision remain unchanged. They are
historical reference evidence, not a new proof of today's inputs. Comparisons
show function and exact obligation-ID transitions. Different runtime contexts,
modes or requested scopes are marked incomparable. Partial scopes cannot prove
function removal; removal describes absence from a successful sealed whole-target
verification inventory, not inferred deletion from source text.
Obligation IDs are matched exactly, with no inferred semantic correspondence
between changed or reordered obligations. Repeated Verus friendly function names
are also incomparable: Sail cannot identify one implementation from that name
and never lets timing-entry order select a regression.

The corresponding MCP tools are `start_repair_validation`,
`verification_events`, `explain_failure` and `compare_verification`.
`save_baseline`, `list_baselines` and `get_baseline` expose the same immutable
service records to agents. Save still requires a completed job; read validates
the record's version/name identity and Historical currentness before returning
the complete report. Invalid, corrupt or oversized records fail explicitly.
These operations do not create proof receipts, change target completeness or
promote provisional verdicts. `start_verification` accepts an explicit `engine` choice. See
[`mcp.md`](mcp.md). Machine-readable contracts are in
`crates/sail-api/schema/proof-event-v1.json`, `proof-page-v1.json` and
`repair-result-v1.json`.
