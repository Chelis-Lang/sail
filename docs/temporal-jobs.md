# Native temporal jobs from Rust

`sail-dive` runs the native D.I.V.E. libraries in the separate
`sail-runtime-worker` process. The published Sail 0.3.0 distribution links
`dive-api` and `dive-ir` 0.1.0 from crates.io. The development checkout
pins an upstream revision for edit sessions and reports it as `backend_source`
on edit evidence. The worker supports finite exhaustive model checking,
the admitted final temporal subset, finite one-worker simulation, and
independent validation of complete traces. It does not execute a D.I.V.E.
command, TLC or Java. The payload is also available through the shared runtime endpoint; see
[shared execution](runtime.md#native-temporal-jobs). Ordinary Verus
verification retains its existing behavior.

`sail-dive` is part of Sail's workspace and is published with it. The
`sail-solve` package builds the worker as its `sail-runtime-worker` binary,
beside `sail` ([ADR 0172](decisions/0172-native-temporal-in-main-workspace.md));
`cargo build --locked -p sail-solve --bin sail-runtime-worker` builds it from
a checkout, and every release archive ships it with its installation
manifest. Callers select the worker
using an expected SHA-256 from their trusted installation manifest. The parent
reads and verifies the executable, then executes a private exact-byte copy.
Missing or changed bytes are explicit failures, never a backend fallback.
The verified copy is kept for later jobs in a per-user private directory
(`sail-runtime-images-<uid>` under the temporary directory) and reused only
while the installed worker and the copy both keep their recorded complete
stat stamps; any write, replacement, `chmod` or `utimes` of either verifies
the bytes again ([ADR 0173](decisions/0173-verified-worker-image-reuse.md)).
Each job still gets its own private working and temporary directory.

```rust,ignore
use sail_dive::{Model, Operation, Request, VERSION, client};
use std::{collections::BTreeMap, path::Path, sync::atomic::AtomicBool, time::Duration};

let request = Request {
    version: VERSION,
    model: Model {
        root: "Counter.tla".into(),
        config: "Counter.cfg".into(),
        files: BTreeMap::from([
            ("Counter.tla".into(), source_text),
            ("Counter.cfg".into(), configuration_text),
        ]),
    },
    operation: Operation::Check { max_states: 100_000, max_depth: 1_000 },
};
let result = client::run(Path::new(worker_path), expected_worker_sha256,
    &request, Duration::from_secs(30), &AtomicBool::new(false));
```

Inspect `RunResult` before the backend outcome. `TimedOut`, `Cancelled` and
`Failed` establish no model result. The parent kills and reaps the owned
process group on timeout or cancellation, including during model binding,
and keeps the deadline active while input/output pipes drain. Each response
binds the exact request SHA, protocol version and D.I.V.E. release
(`sail_api::temporal::DIVE_VERSION`, reported as `backend_revision`). Stray
stderr, malformed or trailing output, duplicate JSON members and oversized
documents are protocol failures. The encoded request and response bounds are
16 MiB each; the source closure is at most 8 MiB and 1,024 files.

Inputs are immutable UTF-8 files with relative portable paths, explicit root
and configuration, and unique module file names. Imports resolve inside the
private closure or to pinned modules. Alongside `Naturals`, `Integers`,
`Sequences`, `FiniteSets` and `Bags`, the worker admits the embedded TLC
operators `:>`, `@@` and `Permutations`, and the bundled CommunityModules
`FiniteSetsExt`, `Folds` and `Functions`. A model can use `Max` and configured
symmetry with these imports. The CFG must assign literal values to declared
scalar constants and select source-defined roles. The worker checks source
uses even in an unused operator, and checks action labels before trace replay.
Other TLC operators, CFG redirections in this profile, unqualified imports
and host access return `SAIL4202`.

Model checking uses mathematical integers, exact states, seed zero,
fingerprint zero and one worker. Bound safety-only properties use ordinary
native checking; genuine temporal properties use D.I.V.E.'s ordinary
completed-level temporal checking, and every temporal counterexample is
independently validated. Genuine temporal properties run on the same exact
state store and state/depth limits as safety checks, never a fingerprint-based
or unbounded search. A holding verdict requires actual typed complete
exploration evidence (for temporal properties, a completed graph capture); the
native report's `check_kind` alone is not completion evidence. A state or
depth limit returns `Stopped`, not Holds; for temporal properties D.I.V.E.
reports the stop in its `graph-capture` phase and makes no claim about the
model. Temporal evaluator/product limits
remain those reported by the linked backend. There is no hard RAM guarantee.

`Operation::Simulate` requires positive trace/depth counts, up to 100,000
traces and 1,000,000 transitions per trace. It returns sampled evidence with
`model_proved: false`, never an exhaustive proof. The parent deadline still
bounds execution. Every returned trace passes independent semantic replay.
Sampled behaviors and simulation deadlocks are replayed under D.I.V.E.'s
sampling rules: a simulation deadlock means every successor of the last
state lies outside the model's constraints, and its validated trace makes the
job `Refuted`.

The response, with each returned trace in its native, ITF and validated
forms, must fit the 16 MiB worker bound. D.I.V.E. samples and checks every
requested trace. The response keeps the lowest-numbered traces that fit,
plus the deciding trace of a violation, and replays each one independently.
The native report's `statistics.traces_started` and `traces_kept` give the
two counts. A job of more than 16 traces first samples 16 to size how many
traces it keeps. D.I.V.E.'s traces for a fixed seed do not depend on the
trace count, so these are the job's own first traces. If the kept traces
still exceed the bound, the job runs once more keeping only those that fit.
It is `Rejected` with `SAIL4216` only when not one trace, or the deciding
trace alone, fits. A 1,000-trace, depth-100 job on bosco samples all 1,000
traces and returns 80 of them in a 15 MB response.

`Operation::Validate` accepts `ItfTrace`: complete ITF variable states plus
separate ending, temporal property names, native action labels and optional
length. Integers use canonical decimal `#bigint` values. Sets, sequences,
records and functions retain their distinct ITF forms. Missing states,
unknown tags, `#unserializable` and representations that
do not round-trip exactly are unsupported. Native reports and native trace
documents remain available alongside the ITF rendering. The independent
validator checks Init, every transition and the actual ending claim; decoding
or round-trip equality alone does not establish validity. A finite behavior
trace establishes only that execution.

A counterexample can contain a symbolic model value and an ordinary TLA+
string with the same spelling. When a trace contains any model values, its
`value_encoding` is `sail-model-values-v1`. For example, this variable state
has a model-valued owner and an ordinary string label.

```json
{"owner":{"#sail-model":"w1"},"label":"w1"}
```

This is a Sail extension in the temporal trace wrapper. Generic ITF consumers
need a decoder for this marker before interpreting the extended state's model
values. Use `sail runtime execute` with a native `validate` request to check
the witness against the current specification. Existing traces without atom
tags retain their original encoding and request hashes. A
[pinned Disruptor control](validation/temporal-reuse-s7.md#model-valued-disruptor-witness)
reproduces a real counterexample and tests a same-spelled string mutation.

The [versioned specification](spec/temporal-jobs.md) and
[ADR 0067](decisions/0067-dive-runtime-worker.md) define the boundary.
Tests compare the adapter with direct calls to the exact pinned library,
including real violations, forged witnesses, unsupported inputs, resource
stops and process failures. They add no TLC backend or fallback.

## Rechecking and undo

The shared runtime keeps completed exhaustive checks in its local cache. Run the
same check again, including after undoing an edit, to reuse its evidence. A hit
sets `cache_hit` to true and reports the matched digest in `cache_key`. The
outer result carries this invocation's obligation and requirement IDs. Native
statistics and the validated trace describe the check that established the
result. Direct calls to the low-level worker client always execute the request.

The identity covers every closed model and configuration file, worker and
backend identity, checking policy and requested limits. A different limit or
file starts a fresh check. Simulation, trace validation, incomplete exploration,
errors, cancellation and timeout do not create proof receipts. Keep the local
cache writable only by trusted code. For GitHub-hosted jobs, see
[authenticated temporal receipts](ci-cache.md#native-temporal-receipts).

## Recheck an unchanged program after a project edit

An editor can submit an entire closed project and choose a different module
and CFG for each check. After checking the original request, use the same
cache for a later project revision.

```sh
sail runtime execute --file previous.json --cache project-cache
sail runtime recheck --previous previous.json --file current.json --cache project-cache
```

For `unchanged-loaded-program`, the same authenticated worker resolves both
source graphs. Their module paths, standard origins and bytes must match in
loader order. The root, selected CFG bytes and all limits must match, and the
previous receipt must establish complete ordinary Holds. The output names
the loaded modules, both request identities, the original receipt and the
current caller. Only the establishing run has exploration counts. A later
ordinary execution at the current identity still runs fresh. Changed imports,
shadowed modules, extracted monoliths and changed assumptions fall back to
ordinary checking. An exact receipt for the current request takes precedence.
[Public edit histories](validation/temporal-reuse-s7.md#installed-public-edit-histories) · [SlateDB WAL counts and receipt measurements](validation/temporal-reuse-s7.md#slatedb-wal-maintenance-checks).

## Recheck a previous invariant failure after a configuration comment edit

In development builds after v0.3.0, save the two temporal `check` request envelopes. Run the previous request normally, then ask Sail to recheck the edited request using the same local cache:

```sh
sail runtime execute --file previous.json
sail runtime recheck --previous previous.json --file current.json
```

The command also sees receipts populated by the local daemon or restored by the signed action. Pass `--cache DIR` when the original job used a custom cache. The current input can come from stdin with `--file -`.

If the current job already has an exact receipt, Sail returns it through the ordinary `current-check` route. Otherwise Sail looks up the exact previous job without executing it. The fast route requires the same worker, backend, state/depth and wall limits, module names and bytes, and two clean CFG parses with identical values, declaration order and retained source locations. A trailing CFG comment is one useful example. The previous result must name a single configured invariant violation. A separate worker validates that trace against the current request. The version-1 result names the route `equivalent-cfg-rechecked-refutation` and records the current caller and request digest, previous receipt key and digest, and the validator's actual response. It establishes a refutation. It publishes no new exact receipt and copies no old exploration counters into the current result.

Other edits, missing receipts, simulation evidence and an inconclusive trace check fall back to normal execution of the current check. This route is `current-check` with the ordinary `response` envelope. An exactly repeated request uses normal exact replay. Each fallback retains the submitted job's full limits. The preceding optional lookup and validation have their own window bounded by the submitted wall time. Exit 0 indicates a conclusive check or validated refutation, 1 an incomplete current check and 2 malformed input or a command error. For changed properties, the edit session below uses Dive’s ordered states and complete temporal graphs.

## Recheck changed invariants and temporal properties

Use `sail runtime recheck --previous previous.json --file current.json` after
removing an invariant from a previous successful job. Sail first looks up
the exact current request. On a miss, it can use the authenticated completed
safety receipt for `previous.json`. The same native worker binds both programs
and checks that the current invariants appear in the same order among the
ones that already held. The complete module bytes, transition semantics,
constraints, constants, deadlock policy and limits must qualify. The result
names the current obligations, both request digests, the worker and the
original receipt. On LamportMutex, this covers keeping `TypeOK` and `Mutex`
after checking `TypeOK BoundedNetwork Mutex`.

For a completed check with multiple temporal properties, keeping only its
first property can return `established-first-temporal-claim`. For example,
the InternalMemory job checks `PROPERTY LivenessProperty` and
`PROPERTY Liveness`. Removing the second declaration qualifies when both
bound programs have the same transition system and first liveness check, the
previous run completed entirely in native mode, and the state limit is below
the first periodic liveness boundary. The report names the retained property.
Its `basis_counts` belong to the ordinary check of both properties. An editor
session can return the same route after its previous `fresh` entry.

For a new or changed predicate, pass the successive closed native check
requests to one worker. Given two ordinary runtime envelopes, this command
extracts their native requests and gives the entire batch a 30-second budget.

```sh
jq -s '{version:1, wall_time_ms:30000, checks:map(.input.job.request)}' \
  previous.json current.json | sail runtime edit-session --file -
```

Each entry contains the current request digest, worker microseconds and
evidence. `safety-rows` checks a new invariant against the states in their
original discovery order. A failed original search can supply a prefix for
another currently validated failure. A holding decision requires the complete
reachable set. `temporal-graph` checks the current temporal formulas against
the stored states and labelled edges. Dive checks actual stuttering, fairness,
configured periodic boundaries and the complete set of property names on a
counterexample. `established-invariants` keeps a completed proof and its bound
invariant inventory without retaining the state rows. This route can project
a proof onto previously checked invariants for models with aggregate state.
Sail validates the ITF rendering of each reused counterexample against the
current model.

For an unchanged request or an undo, `exact-request` points to the zero-based
`basis_entry` of an earlier `fresh` Holds or Refuted in this batch. Read that
entry’s full native response for the verdict, counts, property names and validated
word. Comparing every file’s name and bytes, the selected root and CFG, protocol
and limits makes this applicable even when graph or invariant capture declines.
Stopped checks, refusals and changed requests require another decision.

The row and graph APIs currently admit pure models with scalar integer or
Boolean state fields. A different transition relation or an unsupported edit
can require a new capture or a fresh ordinary check. The `reused` flag and
basis digest distinguish these outcomes. `basis_counts` describe the run that
established the artifact. `decision_counts` are the ordinary search position
proved for the current answer. A graph scan generates zero successors.

The isolated CLI takes 1–64 exhaustive requests and a positive whole-worker
deadline of at most five minutes. Input and aggregate output each have a
16 MiB limit. Send EOF when writing to stdin. The caller’s input time is
charged when the batch arrives. Cancellation or expiry returns no partial
evidence. Exit 0 means all checks were conclusive, 1 means a stop, incomplete
check or worker failure, and 2 indicates invalid input or CLI setup. Each call
owns its own artifact. Rust editors with a live process can instead keep
`sail_dive::edits::Session` and call `check` as new edits arrive. That low-level
API runs synchronously in the caller. The CLI enforces the process deadline.

The [measurement record](validation/temporal-reuse-s7.md) gives capture costs,
per-edit samples, native fresh comparisons and examples that used the ordinary
fallback. The [protocol](spec/temporal-jobs.md) describes implementation and
dependency identity.

## Recheck a saved temporal counterexample

```sh
sail runtime witness-recheck --dive /path/to/dive --module Clock.tla \
  --config Clock.cfg --trace saved.json --time-limit 30
```

Pass the trusted Dive executable and a native word naming all false temporal
properties. The worker binds the current TLA+ source and CFG. It validates the
saved word, can bridge broken steps by generating bounded paths through the
current Next relation, and replays the complete replacement independently.
For either successful route Sail returns `route: validated-witness` and exit
13. The exact word the worker validated is in `validation.repair.trace`.
The unchanged and repaired cases are distinguished by `validation.repair.status`.
The witness establishes the violation directly and has no BFS state counts.

An unreadable or oversized donor, declined repair, disproved claim, unrecognized
repair response or older worker triggers an ordinary single-worker current
check with the remaining time. Its route is `current-check`, and `report`
contains Dive's native check report. Sail binds the report to the submitted
paths, semantic profile and requested TLC exit convention. A contradictory
report returns error `SAIL6102`. For an ordinary temporal violation, the human
property names, complete native word and displayed states must agree. For an
ordinary finite violation, Dive’s typed decoder and TLC printer check the
displayed state values. A stopped ordinary Dive check carries exit 5.
A supervisor time limit or cancellation returns `stopped` with exit 4. Both calls use the same original deadline, Dive's
default semantics, seed 0 and fingerprint 0. The existing native temporal
receipt database and pinned worker are independent of this external command.
To carry a witness across a parameter renaming, supply the explicitly mapped
anchors as the trace. A mapping carries no validation authority.
[MultiCarElevator and real-time clock measurements](validation/temporal-reuse-s7.md#repairing-named-temporal-counterexamples).

### Discover donors from an edited job

```sh
sail runtime witness-recheck --dive /path/to/dive --module Actors.tla \
  --config current.cfg --donor-index saved-witnesses.json --property Live --time-limit 10
```

The version-1 index points to existing native words and their original inputs.
Relative paths are resolved beside the index.

```json
{
  "version": 1,
  "donors": [{"module":"old/Actors.tla","config":"old/Actors.cfg","word":"old/word.json"}],
  "layout": {
    "integer_carrier": "N",
    "renames": {"old_actor":"new_actor"},
    "fields": [
      {"path":["x","seq","0","record","actor"],"kind":"model"},
      {"path":["x","seq","0","record","coordinate"],"kind":"integer"}
    ]
  }
}
```

Sail matches the module filename and configured property. It independently
validates each candidate on its own source before trying the unmodified word
on the current source. If needed, it tries ordered embeddings of `1..N`, where
both CFGs bind `N` to an integer literal, and applies explicit model-value
renamings. Field paths follow the native state document's typed values. `*`
selects every item in an array. `integer_or_model` preserves a model-valued
alternative and maps an integer alternative. Selectors cannot overlap, and
renaming cannot merge distinct observed atoms in the selected fields. Any
changed word still needs complete replay on the current source.

The index holds up to eight donors. Current-source attempts are capped at 16
and use up to half the request deadline, with a five-second ceiling for the
whole donor phase. Sail records the selected donor digests, embedding, prior
replays, attempted mappings and timings under `selection`. On a miss,
`fallback_reason` identifies what stopped selection and an ordinary check uses
the remaining time. The [reproduction panel](../scripts/collect-witness-selection.py)
uses an unmodified seven-floor Elevator donor and a typed actor example.
[Native measurements and controls](validation/temporal-reuse-s7.md#donor-selection-and-typed-embeddings).
