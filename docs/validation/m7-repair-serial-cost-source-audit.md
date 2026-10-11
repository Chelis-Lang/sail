# Original repair serial cost boundaries

This read-only audit examines root source `a627f3f67a34a1163e1580021ae4443734d47cd1`
while its separate gate/solver work proceeds. No fixture, proof actor, deadline,
compiler target or production code was changed. The original fixture remains
SHA-256 `9880f200f99007e048727dbe8348eb93f9e53937df601eb16f2d9fdc9ceaebd7`.
The instrumented-product diagnostic and its measurements remain pending.

The complete path of `ci/test-repair.py` includes six serial validations: success, background
missing selector, foreground missing selector, full regression, MCP repair and
late EXEC. Its two existing establishment controls can add up to two validations.
All share the original 120-second proof/tool budget. Dependency fetching and
engine installation precede that budget; full plan checks, negative full verdicts
and genuinely fresh verified-target invocation remain mandatory. Progress polling
overlaps verification: adding all polling RPC durations to child execution time
would double-count elapsed work. The fixed quarter-second poll interval and CLI
process creation remain original fixture behavior.

The runner's `execution.wall_ms` is sampled when constructing `ProjectResult`,
before result-file publication, guarded receipt commit, compiler-generation
publication and disposal of its native owners. The worker's `child_wait_us`
also spans those phases, worker-process startup and process exit. Only then does
it record document parsing, synchronous independent freshness establishment and
terminal status persistence. Historical worker-minus-runner gaps therefore
cannot be assigned solely to freshness establishment. Retained setup publication
phases and the new worker timing fields are the appropriate evidence boundaries.
The 256-row ring may have lost early records; missing or overwritten stages
remain unavailable.

There are concrete seams to assess against those measurements:

- The worker already retains the process-validated FULL document. The separately
  reviewed `9ae6056` candidate borrows its exact workspace/context fields instead
  of parsing stdout again for optional freshness setup. It preserves synchronous
  publication and every completed RPC seal. This removes a duplicate parse; its
  runtime benefit is unmeasured.
- `runner::context` atomically replaces the full-context runtime `.link` on every
  run. `source_inputs::update`, build-input publication and `input-state.json`
  publication also replace metadata tracked by `FreshnessIndex::observation_plan`.
  Even equal final contents do not preserve the previous continuous owner after
  those replacements. A future exact-byte no-op publication could avoid some
  self-invalidation, without substituting equality for input or runtime authority.
  Changing only `.link` would not establish a hot owner while other tracked files
  still change. No such correction is selected here.
- Cold `FreshnessIndex::fill` constructs a namespace observer, acquires runtime
  and input leases, freshly hashes runtime/input bytes, and checks the final plan
  and barriers. Hot `establish` already attempts exact-request owner reuse first.
  Carrying a snapshot across requests or moving completion ahead of fill would
  change authority or terminal behavior; the existing asynchronous fallback can
  return Unknown. These are not redundant checks to delete for timing.

`store::result` also reads/parses a repair report for its generic FULL result and
again for the repair envelope; the cached frame path already avoids that work on
eligible reads. This is a smaller, distinct code path to investigate only if
original tool timings identify it. Its current seal checks must remain meaningful
under concurrent output mutation.

The actionable next step is attribution of the original diagnostic's actual
retained stages. A large `freshness_establish_us` identifies the optional cold-fill
interval; it does not identify which inner hash, lease acquisition or namespace
operation dominates. Large setup receipt/compiler-publication phases identify a
different post-`wall_ms` interval. No scheduling, emulation, historical timeout
cause or acceptance improvement is claimed from this source audit.

Audited source SHA-256 values:

| File | SHA-256 |
| --- | --- |
| `crates/sail-verus/src/managed/runner.rs` | `0b2fe6a764695d4b60502c948699205cf2f337ce889de7a66d715eba32c285fb` |
| `crates/sail-verus/src/managed/freshness.rs` | `953de4875b6a30cca59d189542cbab62677a849c36ca5c620d6ce146ac4d33f1` |
| `crates/sail-verus/src/managed/files.rs` | `f71a9b399f708c819978e85576b3cf76f1a43c36d3c9c4e1c67f8086732407ee` |
| `crates/sail-daemon/src/jobs/worker.rs` | `129c41ef7e9239ee9b65effadf41a4969f2737529dea06376060ac5828a82932` |
| `crates/sail-daemon/src/jobs/store.rs` | `4dae151920bab6b220674e29444fa55990ec0f8ca70b643112507f0c5ef2f7e9` |
