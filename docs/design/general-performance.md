# General performance: algorithm and implementation review

This review covers the pinned Verus fork `47fb3be6`, stock `7d829946`, and the
managed implementation based on Sail `715c0f5e`. The initial data-representation/repeated-work bundle is unselected after
broader whole-invocation measurements. The current runtime retains the
`715c0f5e` implementation of those paths and adds only the separately controlled
ADR 0092 prototype. No proof key, query byte or diagnostic policy changes.
Native qualification is recorded [separately](../validation/general-performance.md).

## Where work is avoided

The largest established gains remove complete stages: shipped vstd avoids
dependency verification, complete receipts avoid a target's verifier,
unchanged-call replay avoids Cargo, and exact query hits avoid solving.
Those are distinct boundaries. A query-cache hit still pays the compiler and
lowering pipeline; an unchanged managed call can start no verifier and still
pay substantial validation work.

Cargo-Verus enables Cargo checksum freshness and configures the verifier as a
compiler wrapper. Sail cannot adopt Cargo's freshness result as proof
authority: its complete source, ghost inputs, native compiler provenance,
receipt and artifact checks cover additional conditions. Cargo itself
[documents the limits of its fingerprinting](https://doc.rust-lang.org/stable/nightly-rustc/cargo/compiler/fingerprint/index.html).

Verus's [driver](https://github.com/Chelis-Lang/verus/blob/47fb3be6fc73dd9eec77ffec31866b3157f430df/source/rust_verify/src/driver.rs)
has ghost-retaining and ghost-erasing compiler paths with distinct type,
mode, ownership and executable-compilation responsibilities. Treating either
as duplicate work without accounting for those checks would be incorrect.
The [bucket pipeline](https://github.com/Chelis-Lang/verus/blob/47fb3be6fc73dd9eec77ffec31866b3157f430df/source/rust_verify/src/verifier.rs)
prunes the crate, constructs context, lowers to SST, applies polymorphism
lowering and verifies the resulting bucket. A solver hit occurs downstream
of that work.

## Cost model and unselected experiment

Let `F` be observed files, `V/E` resolved dependency nodes/edges, `M` requested
target projections and `B` required filesystem observation boundaries.

| Boundary | Required work and authority | Reviewed experiment and remaining limit |
|---|---|---|
| Runtime identity | Complete executable/tool inputs and physical bindings | Replace quadratic duplicate selection with an ordered index; retain first lexical descriptors, input order and alias confirmation |
| Source inventory | Fresh discovery, absent names, bindings, complete byte digests or explicitly admitted settled observations | Share borrowed source descriptors and their native order proposal within a pure projection batch; every acceptance-core order/coverage check and closing mutation readback still runs |
| Dependency selection | Traverse the complete currently resolved closure; selected files remain content-bound | Index node ids once per target batch, preserving first-match behavior for duplicate ids. Remove the full node-array scan per reached vertex; preserve sorted closure bytes, cycles and missing-node behavior |
| Receipt/target validation | Exact receipt bytes, selected source/context, complete target namespace and original full stamps | Freshly reread a bounded sidecar on every use, sharing only decoding/checksum work for byte-identical records. Each phase independently checks context, original settlement and closing complete tree readback |
| Key/checksum encoding | Existing exact JSON encoding and error behavior | Stream that same encoding through an 8 KiB hash buffer; no intermediate graph-sized JSON buffer, format change or normalization |
| Cargo invocation/output | Actual Cargo execution unless ADR 0082 admits an already recorded unchanged transcript | No synthesized first-unchanged output; no verifying-run transcript record or background recorder |
| Compiler/lowering | Fresh compiler checks and the complete dependencies of each lowering operation | Separate bounded shadow experiment under proposed ADR 0052; no persistent frontend reuse selected |
| Query/solver | Exact input-stream identity, pinned solver, deterministic resource limits and recorded provenance | Preserve existing cache/pool behavior. Profile cache misses and dispatch critical paths before proposing another scheduler or solver change |

The indexed graph replaces a traversal's repeated linear node lookup with
index construction plus indexed node/edge visits. Shared source ordering
removes repeated native sorting, not the core's complete linear certificate
checks. Fresh namespace/stamp work still grows with `B × F`; selected closure
encoding and validation can still grow with the total selected material over
`M` projections. These changes do not justify a claim that every invocation
is linear in the number of unique files.

The unselected run-scoped JSON observation is not a freshness cache. Its cache key is
the complete bytes freshly read from a bounded regular sidecar. A replacement,
read failure, malformed payload, over-limit record or redirected file cannot
reuse an earlier decode. It retains one record for one managed run. Source
digest reuse stays opt-in; target digest platform defaults remain unchanged.

Native diagnostic profiles of `b9466ece` support the distinction between
these costs. On c7i.8xlarge / us-east-1, an unchanged full-path call makes
107,610 `statx` calls for multilog, 350,932 for Anvil and 205,357 for the
semi-persistent containers package. Separate CPU-clock captures attribute
roughly 43–53%, 19–22% and 31–37% of sampled userspace CPU respectively to
SHA-256; path-component comparisons and input-order checks also recur.
All traced calls exit zero, with the same locale used for their cold setup
and subsequent calls. Tracing distorts execution: these are work/call-stack
diagnostics, not wall-time acceptance or a prediction that removing that
fraction of CPU will remove the same fraction of elapsed time. The remaining
repeated namespace/stamp and byte-observation boundaries need complete
authority arguments before work can be shared across them.

## What generalizes, and what does not

[Rustc's red-green algorithm](https://rustc-dev-guide.rust-lang.org/queries/incremental-compilation.html)
shows how tracked operation dependencies can avoid recomputation and stop
change propagation when a recomputed result stays equal. The useful lesson
for Sail is to define complete operations and share immutable work at those
boundaries. A dependency graph alone never authorizes a verified answer.

[Fine-grained verification caching in Boogie](https://www.microsoft.com/en-us/research/wp-content/uploads/2016/12/krml245.pdf)
reports large gains from dependency-sensitive reuse. Its finer transformations
also alter verification conditions and can change timeout behavior. Sail must
not import those transformations into default mode: default keys bind exact
solver input, and the pinned solver remains the oracle.

The previous raw whole-VIR cache experiment did not establish stable complete
keys even when SMT stayed unchanged. Spans and identifiers participate in
resolution, erasure, symbol generation and diagnostics. The next compiler
experiment remains measurement-only AST-to-SST under [ADR 0052](../decisions/0052-warm-frontend-reuse.md):
inventory every semantic/global input, lower freshly on every attempt and
compare complete output plus current diagnostic locations. Restoration needs
complete coverage and the ADR's existing wall-time gate including key and
restore costs. A serialized Debug value or matching hash is not that evidence.

The [operation audit](frontend-lowering-boundary.md) identifies concrete
context and diagnostic effects omitted by the previous capture. AST-to-SST
mutates the function map and can emit warnings or interpreter diagnostics;
returning the same SST alone would not restore the fresh operation. The
current serde/Debug span representations are also incomplete. This blocks
restoration before another performance experiment, not after it.

The bounded finer per-artifact digest proposal in
[ADR 0092](../decisions/0092-per-file-owned-target-digests.md) amends
[ADR 0091](../decisions/0091-settled-source-and-target-digests.md) and has owner approval for the bounded prototype. Implementation and native
qualification remain separate from platform selection. It must preserve fresh
complete namespace/binding discovery, original full stamps, the original
two-second rule, Current provenance, changed/unsettled fresh reads and dirty
cleanup. The bounded implementation is opt-in and off by default everywhere.
Independent native and platform qualification remain open.

## Qualification

The [broader evaluation](../performance-evaluation.md) combines real project
structures, independently pinned new candidates, held-out workloads and
generated scaling cases. Test cold and unchanged paths separately from body,
specification, dependency and failing edits. Whole-invocation measurements must
include recording/publication costs: moving work into the preceding verify
call is not a free improvement.

Selection requires unchanged conformance and demonstrated native gains across
that broader coverage. Do not select a component improvement solely because
its asymptotic bound or synthetic timing is better. Per-workload regressions,
incompatible projects, failed controls and incomplete coverage remain explicit.

## Closing barrier experiment

The unselected `32f7bba6` bundle collected pure revision proposals before a
batch without its earlier redundant source sweep, retaining the closing whole
source readback. Its mutation regression and native functional gates pass,
but the bundle earns no material whole-call gain across the expanded cases.
The current runtime retains `715c0f5e`'s original early and closing guards;
the per-file ledger never supplies revision proposals.
