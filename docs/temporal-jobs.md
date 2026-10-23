# Native temporal jobs from Rust

`sail-dive` runs pinned native D.I.V.E. checking in the separate
`sail-runtime-worker` process. It supports finite exhaustive model checking,
the admitted final temporal subset, finite one-worker simulation, and
independent validation of complete traces. It does not execute a D.I.V.E.
command, TLC or Java. The payload is also available through the shared runtime endpoint; see
[shared execution](runtime.md#native-temporal-jobs). Ordinary Verus
verification retains its existing behavior.

Build the worker with `cargo build --locked -p sail-runtime-worker`.
Source builds need read access to the private pinned dependency; see
[private dependencies](private-dependencies.md). Callers select the worker
using an expected SHA-256 from their trusted installation manifest. The parent
reads and verifies the executable, then executes a private exact-byte copy.
Missing or changed bytes are explicit failures, never a backend fallback.

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
binds the exact request SHA, protocol version and D.I.V.E. revision. Stray
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
native checking; genuine temporal properties use final analysis and
independently validated short lassos. A holding verdict requires actual typed complete exploration evidence;
the native report's `check_kind` alone is not completion evidence. A state or
depth limit returns `Stopped`, not Holds. Temporal evaluator/product limits
remain those reported by the pinned backend. There is no hard RAM guarantee.

`Operation::Simulate` requires positive trace/depth counts, up to 100,000
traces and 1,000,000 transitions per trace. It returns sampled evidence with
`model_proved: false`, never an exhaustive proof. The parent deadline still
bounds execution. Every returned trace passes independent semantic replay.

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
