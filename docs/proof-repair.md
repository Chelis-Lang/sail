# Proof events and repair validation

The optional `interactive` engine is a Sail-managed build of the pinned Verus
source. It is selected explicitly. Stock verification retains its existing
behavior. An unavailable or invalid engine returns a setup error; it never
silently claims a batched run.

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

For a binary, replace `--lib` with `--bin NAME`. Function names are exact,
crate-qualified identifiers. This command first checks that selected function,
then independently checks the full requested target in the engine's compiler
batch. It does not change what existing `verify --function` means. Additional
scope filters are rejected because they would defeat full validation.

The version-one repair bundle retains separate `batch.focus` and `batch.full`
project results, engine manifest identity and actual compiler-invocation count.
`batched` describes actual engine support. `accepted` is recomputed from the full
report: successful, complete requested targets, whole-target verification and
Current input evidence after compiler completion. A passing focus phase remains
provisional. A failing full phase, late compiler failure, edited inputs or
incomplete input coverage cannot authorize a repair.
A missing or ambiguous exact function records `batch.selection_error` with a
stable code and prevents acceptance while retaining the independent full report.
The private worker exits with the full phase's status so a completed proof is
not mistaken for an infrastructure failure; the public repair command exits
nonzero when the repair is not accepted.

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
sail jobs events JOB --stream JOB --after 12 --follow --output jsonl
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
`verification_events`, `explain_failure` and `compare_verification`. Existing
`start_verification` accepts an explicit `engine` choice. See
[`mcp.md`](mcp.md). Machine-readable contracts are in
`crates/sail-api/schema/proof-event-v1.json`, `proof-page-v1.json` and
`repair-result-v1.json`.
