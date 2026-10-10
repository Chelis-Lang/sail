# Optional interactive Verus engine

The patch applies only to the pinned source commit and a separate Sail-managed
source variant. Stock releases and ordinary `sail verify` are unchanged.

A repair runs fresh Rust frontend, type, mode, trait and ghost lifetime checks,
then focused verification and an independent full verification in one active
compiler callback. Each phase creates fresh solver contexts. Full stdout and
its diagnostics remain authoritative only after deferred errors and the late
EXEC compilation pass; focused results and progress remain provisional.

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

The cache retains one serialized buffer, not a cloned compiler/global context.
A single global admission lock covers key construction, encoding, span binding,
decoding and cache replacement across every bucket and context. Stock solver
worker concurrency remains unchanged. The retained buffer cap is 128 MiB;
serialization and disk-read buffers also stop at 128 MiB before growth. Key
writers and each aggregate typed lookup table stop at 32 MiB, including Vec
capacity and table metadata. Source-file provenance, each span-binding map and
each converted binding table stop at 16 MiB. Span keys and clones are charged
with both String/Vec contents and conservative HashMap overhead.

The aggregate allocation audit has separate, non-overlapping preparation
phases. The bounds below deliberately count conversion overlap:

| Phase | Simultaneously live cache-owned buffers and temporary budgets | Upper bound |
|---|---|---|
| Constructor/key | Retained body 128; source inventory 16; all captured diagnostic ownership 16; bucket member keys 32; naming encoding 32; lookup tables 32; completed graph encodings 64; one graph table plus two in-progress map key/value encoders 96; final key/baseline writer 32; directory and fixed metadata 2 MiB | 450 MiB |
| Store | Source inventory 16; captured diagnostics 16; message-reference vector 4; encoder Vec 128; span map and conversion overlap 32; provenance/binding table 16; replacement Arc conversion 128; fixed/directory metadata 2 MiB | 342 MiB |
| Read/restore | Source inventory 16; captured diagnostics 16; read Vec and Arc overlap 256; current and resolver span maps 32; decoded provenance table 16; fixed metadata 2 MiB | 338 MiB |

The source cap checks reject an uncertain/oversized owner before retaining it.
Diagnostic admission counts the entire reachable `MessageX`: note, help and
fancy-note capacities, labels, primary spans, packed span data, display strings,
known raw-span allocations, Arc/vector overhead. Shared allocations are
conservatively counted again. Unknown dynamic message/raw-span types miss.
Focus evidence shares this 16 MiB capture budget; overflow marks focus
unavailable and still runs the independent full proof. No cloned `Ctx`, mutable
GlobalCtx or duplicate diagnostic-owner vector is retained for preparation.

The 512 MiB memory bound covers these cache-owned allocations, not the fresh
compiler's working AST/SST or its decoded active lowering state. The value
codec can duplicate shared IR nodes while decoding; it does not promise DAG
sharing or a bound on total compiler RSS. Interpreter state is never cached,
and pointer-keyed analysis maps are recomputed after the checkpoint. Measured
RSS includes these ordinary working allocations. The reported
`peak_retained_bytes` is charged retained ownership, **not** peak allocation or
process RSS; temporary budget arithmetic above is separate.

Persistent files bind version, exact input key and body digest. A distinct
shared directory limit of 512 MiB includes committed and temporary files;
writers reserve under the directory lock and reject excessive directory entry
counts. This disk allowance is not added to the memory guarantee. The local
store trusts its sole owned engine writer; it is not a shared, adversarial IR
import format. No prepared entry is imported from the signed SMT shared tier.

## Validation

`test-engine.py` and `test-prepared.py` are local-only differential gates using an already built
variant; it never compiles Verus in CI. They compare stock exit status, exact
stderr, verification counters, per-function verdicts and full raw SMT sessions.
Fixtures cover bitvectors, termination, failed focus, failed other functions,
macro diagnostic spans and late EXEC errors. The edit sequence checks real
persistent hits, other-module edits, stale-span misses, corrupt and misfiled
checkpoints, and bounded refusal of oversized files. `test-cancel.py` kills a worker at an
actual blocked solver check, requires no terminal seal or unfinished focus
report, and verifies the restart against stock.
Final performance acceptance uses the frozen repair catalogue and the complete
focused-plus-full loop, including input/runtime validation and late compiler
checks. A source build or cache hit count alone is not performance acceptance.
