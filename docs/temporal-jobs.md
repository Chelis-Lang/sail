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
and configuration, and unique module file names. Imports resolve only inside
the private closure or the admitted embedded `Naturals`, `Integers`,
`Sequences`, `FiniteSets` and `Bags` modules. Other standard modules, including
host-capable `TLC`, `Json` and `Randomization`, refuse before binding.
There are no external search paths or host grants.

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

The response, with every trace in its native, ITF and validated forms, must
fit the 16 MiB worker bound. A job of more than 16 traces first samples 16.
D.I.V.E.'s traces for a fixed seed do not depend on the trace count, so these
are the job's own first traces. If their measured size, scaled to the
requested count, exceeds the bound, the job is `Rejected` with `SAIL4216`
before the full run, naming the largest trace count that fits at that depth.
Because the scaled size is an estimate, the full run is also refused with
`SAIL4216` as soon as its evidence exceeds the bound, before replaying further
traces. A 1,000-trace, depth-100 job on bosco is refused in about 2 s. At that
depth, bosco fits 102 traces.

`Operation::Validate` accepts `ItfTrace`: complete ITF variable states plus
separate ending, temporal property names, native action labels and optional
length. Integers use canonical decimal `#bigint` values. Sets, sequences,
records and functions retain their distinct ITF forms. Missing states,
unknown tags, native model values, `#unserializable` and representations that
do not round-trip exactly are unsupported. Native reports and native trace
documents remain available alongside the ITF rendering. The independent
validator checks Init, every transition and the actual ending claim; decoding
or round-trip equality alone does not establish validity. A finite behavior
trace establishes only that execution.

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

## Recheck finite safety properties with a portable predicate

Development builds provide `sail runtime symbolic-check` for a trusted Dive
installation that supports `dive check --strategy symbolic` and predicate
transfer. Set `DIVE` to its executable. Give each model a dedicated writable
session directory. Each invocation checks the module and configuration on disk.

```sh
sail runtime symbolic-check --dive "$DIVE" --session .sail/kvs \
  --module MCKVS.tla --config MCKVSSafetyMedium.cfg --time-limit 120
sail runtime symbolic-check --dive "$DIVE" --session .sail/kvs \
  --module MCKVS.tla --config EditedSafety.cfg --time-limit 120
```

The first completed search saves `sail-symbolic-predicate-v1.json` in the
session directory. A colleague can copy this one file into another session.
The receiving Dive binds its own modules and CFG and checks initial inclusion,
preservation by every current action, and implication of each configured
safety obligation. A successful row reports `proof: inductive-safety`. This
proof carries no state counts or diameter. It can still succeed after a
transition edit. Use `--type-invariant NAME` to select the layout invariant
when the source uses a different name.

When a candidate fails an obligation or its format is unsupported, Sail uses
the remaining wall budget for a fresh symbolic search. A completed search
atomically replaces the candidate. A violation carries a trace independently
validated by Dive. The previous candidate survives timeouts, interrupted
searches, refusals and violations. Concurrent successful writers may replace
each other's candidates. Every receiver checks whichever one it reads.
Keep the session directory outside the source tree or ignore it in version
control. Use a directory whose contents are managed by trusted local code.

The command emits Dive's JSON report with its original integer precision.
Exit 0 means a proof or completed safe search, 1 a validated counterexample,
2 unsupported input or a Sail usage/protocol error, 3 a checker error, and
4 a stop. Cancelling or expiring a check allows up to 400 ms for Dive to
reap its worker group before Sail sends a hard stop. A cgroup or equivalent
job controller is needed for cleanup if Sail itself receives SIGKILL. Sail's ordinary temporal receipts, daemon, and signed receipt
transport continue to use their own execution and evidence policy.

For repeatable comparisons on three- and five-key KVS, individual timings
and a cardinality change that invalidates the old predicate, see
[portable symbolic predicate measurements](validation/temporal-reuse-s7.md#portable-symbolic-predicates).
