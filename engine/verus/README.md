# Optional interactive Verus engine

The patch applies only to the pinned source commit and a separate Sail-managed
source variant. Stock releases and ordinary `sail verify` are unchanged.

A repair runs fresh Rust frontend, type, mode, trait and ghost lifetime checks,
then focused verification and an independent full verification in one active
compiler callback. Each phase creates fresh solver contexts. Full stdout and
its diagnostics remain authoritative only after deferred errors and the late
EXEC compilation pass; focused results and progress remain provisional.

The interactive driver catches only the pinned Rust compiler's expected
`FatalErrorMarker` unwind. Unexpected internal panics resume unwinding and cannot
reach a compiler seal; ordinary default verification retains its existing
catch-all behavior. A terminal `compiler-sealed` frame includes
`payload.normal_termination: true` only on a normal driver path, including
reported proof, VIR and late EXEC errors. This flag describes completion, not
success: the full JSON, exit status, selected scope and coherent input checks
still determine authority. Missing or malformed completion, internal panic,
signal, or malformed full output requires the stock fallback. The source patch
and installed runtime identity change. The selected patch's portable
checkpoint format is version 3 (`PortableView`/`Portable` in `sail_engine.rs`);
a restore rejects every other version.

## Prepared lowering checkpoint

The checkpoint sits after fresh global construction, pruning and `Ctx::new`,
before `ast_to_sst_krate` and `poly_krate_for_module`. A hit skips those two
lowering operations; it does not skip frontend checks, AIR generation or proof.
An uncertain input, unsupported interpreter state, logging side effect, span
binding or checkpoint format is a miss.

Keys use typed binary serialization, not Rust debug rendering. In particular,
stock debug rendering omits stable crate ids and some span identity fields.
Pruned VIR includes every field in its serde representation, with original
function/datatype/trait/module order. Bucket membership binds full `Fun`
identities. Lookup maps encode complete typed entries in key order; ordered
vectors, graph nodes/edges and SCC state preserve their actual order. No query
bytes are normalized.

The constructor/effect audit is:

| State | Key input or replay rule |
|---|---|
| Global datatypes, function bounds/attributes, warning configuration, extension maps | Complete typed lookup entries |
| Function/datatype graphs, SCCs and diagnostic span indexes | Complete graph state, original node/edge/vector order |
| Global synthesized span, architecture, crate id, solver, limits, API safety, axiom and bitvector options | Explicit typed values; solver enum spelling is complete |
| Pruning mono/spec-function types, reached dynamic traits, builtins, function definitions and resolved types | Complete typed values; original vector order |
| Module, visibility/invariant lookup sets, function/datatype/trait/impl/opaque/reveal maps, naming lookup and ensure-predicate state | Complete typed constructor state |
| Existing chosen triggers, qid/quantifier maps, names and string/byte-string collision tables | Complete baseline binding |
| Function SST map and polymorphic datatype map | Replay complete lowered maps |
| Names, chosen triggers, qids, quantifiers, ensure-predicate and string collision maps | Replay complete post-lowering values |
| Per-function temporary context | Must be empty at checkpoint; otherwise miss |
| Interpreter and graph log files | Their lowering side effects disable reuse |
| Diagnostics emitted during lowering | Replay exact structured messages, current spans, supplied level and original `report`/`report_as`/`report_now`/`report_as_now` method |

The map-order audit applies to the pinned source, not arbitrary future Verus
versions. `ast_to_sst`, `poly`, `sst_to_air`, `sst_to_air_func`, `expand_errors`
and `heuristics` consume the restored function/datatype/ensure/string maps by
keyed lookup or entry update. Quantifier counts are keyed by function; qids are
inserted by generated id and later retrieved by the profiler. `NameCtxtImpl`
uses keyed `entry` decisions for stable crate ids and duplicate-name counts.
These tables do not drive declaration, diagnostic or qid traversal order.
Their serialized insertion/iteration order therefore carries no semantics.
Chosen triggers, pruned functions/datatypes, graph nodes/edges, SCCs, extension
member vectors and diagnostic messages retain their original sequence. Naming
decisions made during constructor iteration are already present in the complete
baseline name tables. Any future iterator consumer invalidates this audit and
must preserve that order or disable reuse; the patch and lock identity prohibit
using these checkpoints with another source version.

Serialized spans retain id, packed crate/byte positions and display text. A hit
resolves every AST and diagnostic span to an exactly matching span collected
from this callback's pruned VIR and constructor span seeds. It also validates
its current SourceMap owner: ordered file index, name, starting offset, length,
content hash and raw SpanData hygiene/parent ownership. Imported spans bind the
current foreign-file table. Missing, ambiguous or stale bindings miss; a saved
raw rustc span is never reused across compiler callbacks.

Provenance hashing keeps a memo owned by this callback's immutable ordered
source-owner inventory. Imported entries bind both packed span words. Local
entries bind all owned raw SpanData fields: lo, hi, syntax-context identity, and
optional parent definition index. The pinned public SyntaxContext API does not
expose its numeric field; its audited Debug implementation writes only `#<u32>`.
We bound and validate that token before encoding it. Neither SpanData Debug nor
`.span()` runs on a verification worker: both can access compiler thread-local
interners for long or complex spans. The minimal reproduction is in
`corpus/repros/engine-span-data-worker/`.

Local provenance has a `local/raw-fields/v2` domain, the complete constructor
key uses the `sail-prepared/graph-ledger/current-literal-missing-original-range/v10`
domain, and portable checkpoints require version 3. Older entries cannot be
restored. Every miss runs the same exact field/owner hashing as a memo
hit, and every current span binding is still resolved and checked. No raw Span,
TyCtxt, or SourceMap is retained across callbacks. The memo never survives
`finish_cache` or a compiler callback. Its fixed table never
resizes after the initial reservation; HashMap capacity, owned key/result String
capacities, metadata, and a 4 KiB allowance for bounded transient work all count
against 8 MiB. A full table merely recomputes hashes. Unknown or oversized span
encodings refuse preparation, leaving ordinary lowering intact.

The cache retains one serialized buffer, not a cloned compiler/global context.
A single global admission lock covers key construction, encoding, span binding,
decoding and cache replacement across every bucket and context. Stock solver
worker concurrency remains unchanged. The retained buffer cap is 128 MiB;
serialization and disk-read buffers also stop at 128 MiB before growth. Key
writers start at 64 bytes and each aggregate typed lookup table stops at
32 MiB, including Vec capacity and table metadata. Starting every tiny map key
and value at 4 KiB previously exhausted this budget for real-project constructor
graphs despite their small encoded entries; the correction changes allocation
only, preserving every typed byte and resulting key hash. Source-file
provenance, each span-binding map and
each converted binding table stop at 16 MiB. Span keys and clones are charged
with both String/Vec contents and conservative HashMap overhead.

The aggregate allocation audit budgets separate, non-overlapping preparation
phases. The arithmetic below deliberately counts conversion overlap; the
baseline reporter-release limitation described below prevents treating it as
a guaranteed bound on physically live allocations:

| Phase | Simultaneously live cache-owned buffers and temporary budgets | Budget arithmetic |
|---|---|---|
| Constructor/key | Retained body 128; source inventory 16; all captured diagnostic ownership 16; bucket member keys 32; naming encoding 32; lookup tables 32; completed graph encodings 64; one graph table plus two in-progress map key/value encoders 96; final key/baseline writer 32; directory and fixed metadata 2; callback provenance memo including transient work 8 MiB | 458 MiB |
| Store | Source inventory 16; captured diagnostics 16; message-reference vector 4; encoder Vec 128; span map and conversion overlap 32; provenance/binding table 16; replacement Arc conversion 128; fixed/directory metadata 2; callback provenance memo including transient work 8 MiB | 350 MiB |
| Read/restore | Source inventory 16; captured diagnostics 16; read Vec and Arc overlap 256; current and resolver span maps 32; decoded provenance table 16; fixed metadata 2; callback provenance memo including transient work 8 MiB | 346 MiB |

The source cap checks reject an uncertain/oversized owner before retaining it.
Diagnostic admission counts the entire reachable `MessageX`: note, help and
fancy-note capacities, labels, primary spans, packed span data, display strings,
known raw-span allocations, Arc/vector overhead. Shared allocations are
conservatively counted again. Unknown dynamic message/raw-span types miss.
Focus evidence shares this 16 MiB capture budget; overflow marks focus
unavailable and still runs the independent full proof. No cloned `Ctx`, mutable
GlobalCtx or duplicate diagnostic-owner vector is retained for preparation.

The intended 512 MiB memory bound covers cache-owned allocations and validation
scratch, including decoded preparations until their validated handoff to the
active compiler. Fresh compiler working AST/SST after that handoff is reported
separately. The
selected runtime's `RecordingReporter` and `FocusReporter` release diagnostic
quota in their `Drop` implementations before Rust drops their message vectors
and nested cache-owned Arc references, so a concurrent reporter can admit new
ownership while old allocations are still live. Its accounting therefore does
not establish the aggregate physical bound. Unselected corrective overlays
(reporter ownership, bounded decoding, observer budgets; a conservative 492 MiB
phase argument) were built and partly qualified but never selected; they are
summarized in [the M7 validation record](../../docs/validation/m7.md) and kept
in the `archive/m7-contracts-f7103bf` tag.

The value codec can duplicate shared IR nodes while decoding; it does not promise DAG
sharing or a bound on total compiler RSS. Interpreter state is never cached,
and pointer-keyed analysis maps are recomputed after the checkpoint. Measured
RSS includes these ordinary working allocations. The reported
`peak_retained_bytes` is charged retained ownership, **not** peak allocation or
process RSS; temporary budget arithmetic above is separate. `batch_hits` counts
RAM-entry restores, and `persistent_hits` counts validated file restores. Parallel
workers can restore files written earlier in the same batch, so the persistent
count alone does not prove reuse across edits.

Sail passes `SAIL_ENGINE_PREPARED_DIR` to every engine driver. By default it
names a directory private to that one compiler invocation, removed when the
driver exits: the focused phase can serve the same invocation's full phase, but
no entry reaches another invocation or source revision. Persistent cross-edit
reuse under `<cache>/engine-prepared` requires the explicit
`SAIL_ENGINE_PERSISTENT_PREPARATION=1` opt-in, because its key binds Verus's
pruned crate and a selected argument tuple and a real hit is not compared with
fresh lowering at runtime (charter rule 6). The opt-in is part of the proof
context and is reported as `engine.persistent_preparation`. An engine fault
removes the persistent directory's entries. The runtime is unchanged by this
policy; it only reads the directory Sail names.

Persistent files bind version, exact input key and body digest. A distinct
shared directory limit of 512 MiB includes committed and temporary files;
writers reserve under the directory lock and reject excessive directory entry
counts. This disk allowance is separate from the intended memory budget. The local
store trusts its sole owned engine writer; it is not a shared, adversarial IR
import format. No prepared entry is imported from the signed SMT shared tier.

## Validation

The current source acceptance contract is the owner-amended
[ADR 0027](../../docs/decisions/0027-interactive-proof-engine-and-repair-workflow.md):
compare cached lowering with fresh lowering from the same compiler input using
independent mutable owners and actual ordered solver writes. Keep independent
stock verdict/diagnostic checks and report stock transcript instability. A
design review is not qualification. See
[product acceptance](../../docs/bench/m7-repair-acceptance.md).

`test-engine.py` and `test-prepared.py` are local-only differential gates using an already built
variant; it never compiles Verus in CI. They compare stock exit status, exact
stderr, verification counters, per-function verdicts and full raw SMT sessions.
Fixtures cover bitvectors, termination, failed focus, failed other functions,
macro diagnostic spans, missing/ambiguous exact selectors and late EXEC errors. The edit sequence checks real
persistent hits, other-module edits, stale-span misses, corrupt and misfiled
checkpoints, and bounded refusal of oversized files. `test-cancel.py` kills a worker at an
actual blocked solver check, requires no terminal seal or unfinished focus
report, and verifies the restart against stock.
Final performance acceptance uses the frozen repair catalogue and the complete
focused-plus-full loop, including input/runtime validation and late compiler
checks. A source build or cache hit count alone is not performance acceptance.

The selected Linux source runtime has passed the original thirteen-stage
qualification, recorded in [qualification-linux.json](qualification-linux.json).
It includes genuine previous-revision focused reuse, three complete HumanEval
revisions, cached-versus-fresh lowering from the same compiler input, exact stock
verdicts and diagnostics, cancellation/restart, and all 152 library controls.
The preceding 150 source controls and both strict checks also pass. Removing the
actual missing-range memo bypass makes its unchanged regression fail.
Independent stock transcript instability remains separately recorded; it grants
no cached-versus-fresh normalization.

Prepared ownership remains conservatively admitted within the 512 MiB ceiling;
unsupported imported metadata falls back to fresh lowering. The source gate and
its memory controls do not establish process RSS, live-reader latency or repair
speedup.

### Running the source qualification

`qualify/qualify-same-input.py` is the thirteen-stage pipeline that qualified
the selected runtime. It needs a sealed source build (its `sail-build.json`
manifest), the stock Verus and pinned Z3, and a human-eval-verus checkout:

```sh
python3 engine/verus/qualify/qualify-same-input.py --build BUILD \
  --stock STOCK_VERUS --z3 Z3 --he HUMAN_EVAL --out OUT --execution LABEL \
  --patch engine/verus/interactive.patch --private-span-boundary --old-engine OLD_VERUS
```

The stages call `test-engine.py`, `test-prepared.py` and `test-cancel.py` in
this directory and the helpers in `qualify/`. The relocated copies differ from
the frozen inventory recorded in `qualification-linux.json` only in their
directory and default patch path; a new patch needs its own complete run.
Earlier qualifications and unselected candidates are in the
`archive/m7-contracts-f7103bf` tag.

## Pinned Linux runtime for CI

`artifact-linux.json` binds the private release asset, archive digest, source
commit, patch, lockfile, Rust toolchain and solver. `ci/engine-artifact.py`
checks those pins and every runtime file before installation. The Docker image
uses a BuildKit secret for the private download; it does not retain that
credential or build Verus from source. The optional runtime is exercised by
the real repair CLI/MCP end-to-end phase alongside the stock checks.

The committed patch and runtime pin select the missing-original-range and
broker-retirement correction. Its ordinary build verified all 2,060 vstd
obligations, and local packaging plus a fresh install matched all thirteen
runtime files byte for byte and mode for mode (delivery record summarized in
[the M7 validation record](../../docs/validation/m7.md)). CI downloads these
exact prebuilt bytes; it never compiles the engine.

This selection is on the in-progress M7 branch. Rebuilt Sail CLI/MCP repair
checks, complete current-product corpus conformance, established reader latency
and the frozen HumanEval/Vest repair benchmarks remain required. The private
prerelease artifact is a runtime prerequisite, not M7 product acceptance.
