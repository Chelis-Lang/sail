# Live source observation ownership overlay

This [overlay](../observer-budget.patch) follows the unselected
[decode/report overlay](../decode-budget/README.md). It does not select a runtime,
raise the 512 MiB preparation allowance, or change proof acceptance. Its exact
source identities and limited validation are in [identity.json](identity.json).
Combined source tests, a normal runtime build, the exact oracles, and a genuine
previous-revision cache hit remain required.

## Discovered gap

`rust_verify/src/sail_engine.rs::diagnostic` converted a borrowed `MessageX` into
an owned JSON `Value` before taking the source event mutex. `emit` then allocated
another unbounded output buffer. Diagnostic expansion can run on proof workers
while preparation workers are active; the focused-document phase ordering does
not exclude this overlap. Function/module/bucket formatting had the same
pre-mutex risk. Progress IDs also retained an unbounded process-wide ordinal
map and uncharged strings across source worker calls.

These are Sail-owned observer allocations, included in the preparation gate.
They are not relabeled as ordinary compiler working IR.

## Bounded observation

Every source `emit` call now uses a borrowed-field macro. The emitter acquires
its process mutex before evaluating field expressions, converting names, or
serializing data. Fields serialize directly into a 1 MiB admitted writer.
Growth, including old/new allocation overlap, is bounded by 2 MiB. There is no
JSON `Value` tree or final String copy. Fields are sorted by their fixed keys
without allocating a sorting container; all arrays keep their original order.

Nested diagnostic objects follow their derived serializer's member order,
where the earlier `Value` representation sorted object keys. The exact JSON
graph, diagnostic text, spans and emission levels are preserved; byte-identical
source event serialization is not claimed. Authoritative Verus stdout/stderr
and the exact SMT oracle remain unchanged.

Overflow discards the partial record and emits a small `observer-unavailable`
record. It never changes a proof or turns missing progress into a compiler seal.
The mutex owns the writer through publishing and physical drop, including open,
write and serialization failures. A poisoned mutex refuses the observation.

## Names, progress and retained ordinals

An independent 2 MiB process quota admits fixed 24 KiB RAII owners **before**
uniqueness scans or progress formatting. One owner covers bounded friendly names
and their helper temporaries, a bounded lookup key, and the returned identity.
The friendly-name helper checks a 4 KiB limit before cloning/conversion. Current
and candidate uniqueness strings coexist at most with their bounded crate-name
conversion. Module observation uses a separately bounded 4 KiB join. The
returned `Progress` retains its owner through the completed-query event; Drop
physically destroys the identity before releasing quota. Failure paths drop
local strings before the local owner. Quota exhaustion makes observation
unavailable, while the original proof continues.

The ordinal table has a separate 2 MiB conservative allowance. New keys admit
eight times their string lengths and inline tuple size plus 512 bytes before
reservation/insertion. This includes key owners, HashMap occupancy/control
bytes and old/new growth overlap. Lookup-key clones are covered by the in-flight
owner. Failed admission does not insert a key. Clearing the table drops its
physical key/table allocations under its mutex; there is no reusable credit
released while another thread can still retain the old table. Existing-key
counters use checked addition and refuse overflow.

Bucket Debug and serialization error Display use the pinned serde_json 1.0.151
`collect_str` override, which streams escaped formatter output through the
bounded writer. The actual sites are the derived `BucketId`/Fun/Path debug
formatters and the pinned codec/I/O error Display implementations. They stream
through the formatter rather than allocate an intermediate complete String.
This is a closed source-site audit, not an allocation guarantee for arbitrary
custom `Debug`, `Display` or `Serialize` implementations. New sites need review.

## Complete observer allowance and phase totals

The observer allowance is 8 MiB: retained ordinal table 2; all in-flight owned
progress/name work 2; one serialized writer with growth overlap 2; bounded
name/formatter/path/fixed scratch 2. The last allowance includes one bounded
friendly/module conversion at a time under the emitter and native source-event
path/opening scratch. Controlled counter/hash/role payloads are fixed-size or
borrowed. The normal source path is supplied by the Sail adapter; original
proof diagnostic owners are borrowed and never copied into another tree.

Adding that allowance to every conservative decode/report phase gives:

| Phase | MiB |
|---|---:|
| Constructor/key | 488 |
| Store | 380 |
| Disk body load | 328 |
| Decode/validate | 492 |
| Focus output | 468 |

All listed owners and scratch remain included through handoff/drop. Prepared
SST and effects are still admitted through decoding, binding and graph
validation; only their move into the current active compiler lowering crosses
the working-IR boundary. Returned diagnostic replay owners remain charged.
The decoded ledger and its bindings drop under the preparation mutex. Focus
serialization still runs only after every focused worker has joined, before
full workers start. The isolated oracle's independent compiler context is a
separate diagnostic compiler arm, not permission for cache-owned allocations
to escape this table. Its explicit source comparison/serialization buffers
remain governed by the existing preparation mutex and allowances.

These are conservative requested-capacity/owned-allocation calculations,
including growth and metadata, not a process RSS or allocator-arena claim.
Retained-byte telemetry alone does not prove the physical gate.

## Checks and remaining qualification

Six isolated tests using the exact observer helpers pass at Rust 1.98.1. They
check canonical top-level/control-field bytes, complete borrowed payload shape,
zero extra Arc owners, duplicate-key partial record refusal, escaping overflow with only an unavailable record,
writer refusal before growth, serialization under the single owner, quota
exhaustion/release, retained ordinal refusal, write failure and streaming Debug
refusal. Strict helper Clippy also passes. A macro call before its declaration also compiles. An actual
`MessageX`/span graph fixture is included in the source overlay and awaits the
combined RustVerify build. The engine track reports 26 combined VIR tests and
strict clippy passing before this observer addition.

No ordinary runtime or real proof/performance qualification is claimed by these
isolated tests. In particular, the same-input exact oracle and the genuine
previous-revision HIT must still pass after combining this overlay with the
private span boundary and epoch lineage changes.
