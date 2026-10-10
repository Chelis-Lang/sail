# Preparation allocation audit and proposed overlay

This [overlay](../decode-budget.patch) is unselected and unqualified. It does not
change the installed engine pin, the 512 MiB allowance, or the proof acceptance
contract. File identities and the limited checks completed so far are in
[identity.json](identity.json). It applies narrowly to the graph-ledger candidate;
its `Capture` lineage changes must be merged separately before qualification.

The audit found three preparation ownership gaps in that candidate:

- `source/rust_verify/src/sail_engine.rs::take_bucket` derived-deserializes
  `Portable.bindings`, diagnostics, SST and effects with a codec byte-read limit.
  A byte-read limit does not admit container capacities before allocation.
  Its returned diagnostic `Vec` also outlives the preparation mutex without
  belonging to the process-wide diagnostic quota.
- `source/vir/src/sail_graph.rs::validate` similarly decodes a derived `Query`.
  Replaying graph queries allocates vectors and, for shortest-cycle queries,
  copied BFS paths. That scratch is preparation validation ownership.
- `sail_engine.rs::sail_focus` expands captured diagnostics into owned JSON
  `Value` trees and then an output `String`. Neither expansion had an admission
  limit. Captured diagnostic quota alone cannot bound that expansion.

The reporter-drop ordering defect remains documented in the
[baseline guide](../../README.md). This overlay builds on the candidate's
physical-drop-first reporters; it does not qualify the selected baseline.

## Decode ownership

`source/vir/src/sail_decode.rs` wraps the borrowed-slice serde decoder. The two
production entry points are the statically fixed `Portable` and graph `Query`
schemas. The reader cannot choose a Rust visitor. Adding a schema type or a
custom deserializer requires a fresh allocation audit; this is not a general
sandbox for arbitrary user-written serde visitors.

The guard hides all container size hints from derived and standard visitors.
Consequently `Vec`, `HashMap`, `HashSet` and the pinned big-integer limb visitor
start without hint-driven preallocation. Before decoding each element/key/value,
it admits eight times its actual inline `DeserializeSeed::Value` size plus
128 bytes. Before entering a structural visitor, it makes the same charge for
that visitor's result. This includes the actual size of large inline enums;
it does not assume every element is pointer-sized. The allowance conservatively
covers minimum/geometric vector allocation, old/new growth overlap, hash-table
occupancy/control bytes, Arc/Box headers, and duplicate decoded DAG owners.
These charges accumulate without release during the decode transaction.

Owned strings and byte buffers are redirected to the identical borrowed wire
encoding. Their bytes are admitted at eight times length plus 128 before the
owned visitor runs. An unexpectedly owned string/buffer, dynamic or ignored
schema, arithmetic overflow, or nesting beyond 128 rejects the entry. A finite
wire sequence count is checked against early end; malformed length hints never
reach a reservation. Oversized or malformed entries miss and lower freshly.

The fixed schema has derived AST/SST/air structs, standard Arc/Box/Vec/hash
containers, and audited custom span and big-integer visitors. The span visitor
clones a decoded key for current-callback binding; its contents and overlap are
covered by the eightfold admission. Big-integer serde starts from the suppressed
hint and consumes admitted scalar limbs. The only bypass is the private
`SailBoundedFrames/v1` newtype tag: its existing visitor admits frame capacities
before reservation, with a 1 MiB frame and a 16 MiB complete-ledger cap. The tag
is transparent under the unchanged bincode wire encoding. That separately owned
16 MiB is included below. No unreviewed custom visitor is exempted.

The complete prepared decode has a conservative 128 MiB subbudget. Before
`take_bucket` returns, provenance bindings and graph frames are dropped while
the global preparation mutex remains held. Only after exact binding, graph,
version/key and complete-byte-consumption validation do decoded effects move
into the current compiler `Ctx` and decoded SST become that invocation's active
lowering state. They are still charged during validation; their later working
IR lifetime is the same boundary used for fresh lowering, not a retroactive
exemption during cache decoding.

`ReplayMessages` first reserves the smaller Arc-tuple conversion under the
already charged decoded-message ownership. It then admits the actual message
capacities and replay-vector capacity into the existing shared 16 MiB
`REPORT_BYTES` allowance. There are no fallible allocation steps after that
admission. The owner survives oracle comparison and every replay call. Its Drop
physically destroys the vector and nested owners before releasing quota. A
reporter that captures a replayed diagnostic must independently admit its own
reference, as the existing focus/recording reporters do.

## Graph validation

Query decoding has a separate 8 MiB conservative guard, one query at a time.
The closed supported inventory is `SccRep`, `IsCycle`, `SccNodes`,
`ShortestCycle` and `FunctionOrder`. `CanReach` is not replayed; its
edge-sized worklist would require a separate edge-count bound.
Before any existing graph query runs, `Graph::sail_validation_cost` bounds its
scratch by vertex count and owned `SpanInfo` text. The linear allowance is
`N * (8 * size_of::<Node>() + 1024) + 8 * sum(text lengths) + 4096`.
Shortest-cycle replay additionally admits `16 * N * N * size_of::<usize>()`.
The 8 MiB scratch cap is checked before replay.

These formulas follow the unchanged algorithms: SCC/node/order output clones
are linear; reached-set and current/next vector metadata are linear. The
shortest-cycle algorithm admits each reached vertex once and copies at most
N-index paths, including old/new vector overlap. Edges are traversed without
allocation; duplicate edges do not create a second reached path. `Node` clones
share Arc-backed names, except `SpanInfo.text`, counted separately above. The
compiler-produced SCC order is a unique SCC representative inventory; no new
algorithm or graph ordering is substituted. An uncertain or excessive graph
misses rather than invoking an unbounded validation traversal.

## Focus reporting

The focused document is serialized from typed borrowed diagnostics and
`FuncDetails`, directly into a 128 MiB bounded writer. It retains the entire
existing JSON graph, including the existing `mode:` key. There is no diagnostic
`Value` tree and no final `String` copy. Focus name/stat bookkeeping admits an
8 MiB aggregate allowance before insertion/growth; individual names are limited
to 1 MiB before cloning. Friendly-name aliases and the pinned crate spelling
are preserved by a checked helper. Duplicate friendly report keys or overflow
make focus unavailable; the independent full proof still runs.

Selection counts exact matches without constructing an unbounded reference
vector. Output overflow makes focused evidence unavailable. It never changes a
full proof verdict or accepts provisional focused evidence.

Source-file debug names stream into an admitted writer rather than an
unbounded `format!` String. The writer checks required capacity before growth;
its temporary growth is confined to source-inventory initialization, before
focus workers start. Retained source inventory is still capped at 16 MiB and
provenance memo at 8 MiB. Native file-name debug formatting must remain a
streaming formatter in the pinned compiler; a newly allocating formatter
requires an audit before this boundary can be retained.

Cache-directory housekeeping admits a 4 MiB path inventory before clone/join
and pre-reserves its vector once. It rejects oversized roots/basenames,
non-basename entries, more than 4096 entries, excess path capacities and
iterator failures. One native directory entry is live at a time. This preserves
symlink metadata and deletion behavior; admission failure refuses the cache
write. Directory inventory is dropped before a second inventory is built.

## Aggregate phase accounting

MiB quantities below are conservative requested-capacity/owned-allocation
budgets, including explicit fixed and per-owner overhead, not process RSS or an
allocator-arena bound. Fresh compiler working IR and the opt-in independent
oracle's compiler state remain separately identified. Retained cache owners and
all cache validation/reporting scratch remain included.

| Phase | Simultaneous allowances | MiB |
|---|---|---:|
| Constructor/key | Existing documented 458, graph capture/lineage owners 16, fixed/directory margin increases from 2 to 8 | 480 |
| Store | Existing documented 350, graph capture/lineage owners 16, fixed/directory margin increases from 2 to 8 | 372 |
| Disk body load | Read Vec/Arc conversion overlap 256; source inventory/provenance 24; captured/replayed diagnostic ownership 16; graph capture/lineage 16; fixed/directory/encoder metadata 8 | 320 |
| Decode/validate | Serialized input 128; current/resolver maps 32; source/provenance 24; diagnostics 16; graph capture/lineage 16; guarded decode 128; decoded frames 16; one decoded query 8; validation scratch 8; fixed/directory/encoder metadata 8 | 484 |
| Focus output | Serialized cache body 128; writer old/new growth overlap 256; source/provenance 24; diagnostics 16; graph capture/lineage 16; report metadata 8; bounded name/escape temporaries 4 (including the next name before aggregate refusal); fixed/directory/encoder metadata 8 | 460 |

The 8 MiB fixed margin includes the admitted 4 MiB path inventory, one
1 MiB graph encoding buffer with growth overlap, one native directory-entry
scratch/path and fixed codec/owner metadata. Capture-to-ledger conversion
metadata is admitted into the original 16 MiB graph quota **before** vector
reservation; its RAII owner physically drops any failed conversion buffer
before quota is released. Completed ledger ownership transfers that charge,
so a racing capture or guard cannot consume the same admission.

Source inventory replacement is an earlier initialization phase: old/new
16 MiB inventories, an 8 MiB memo, admitted name-buffer growth and cache/report/
graph owners remain below the larger constructor bound. The source table
contains at most 4096 owners; its per-owner 256-byte allowance also covers vector
growth and native file-name formatting metadata.

Disk Vec-to-Arc conversion finishes before current span collection or decoding;
there is no 256 MiB disk conversion simultaneously with the 128 MiB decode
allowance. Graph queries drop before the next query. The global preparation
mutex prevents another decode/store/key construction from overlapping. Focus
output occurs after `verify_crate_inner` joins every remaining scoped worker
(`verifier.rs` normal parallel return path) and before `sail_focus` returns to
`after_verus`, which starts the full phase. A worker panic resumes unwinding
before document construction. The report writer therefore cannot overlap a
normal constructor worker; this call order is part of the bound. The reporter and graph quotas cover concurrent retained owners in all
phases, including lineage nodes. Returned replay ownership stays charged after
the preparation mutex unlocks.

The calculations remain review evidence. A combined source build, strict source
checks, all existing exact oracles, and a genuine previous-revision hit are
required before asserting this candidate meets the physical preparation gate.
The telemetry counter is retained-owner charging, not peak physical allocation.

## Checks

The standalone decoder tests pass 5/5. A private copy of the pinned source at
Rust 1.98.1 passes 21/21 selected VIR tests, including the actual span/binding/
message/SST/big-integer schema, alias/Unicode spellings, malformed lengths,
preallocation refusal, recursive boxes/large inline values, duplicated Arcs,
existing graph positives and graph scratch refusal. These checks are not an
ordinary Verus runtime build or a proof/performance qualification. The same
private VIR copy passes strict clippy at Rust 1.98.1. The latest conversion
preallocation change is covered by that compiled source; its final strict clippy recheck also passes. Two exact extracted inventory-helper
tests also pass at Rust 1.98.1 with warnings denied, including a genuine directory
with 4097 entries, lock retention and symlink metadata; these are not a full
`rust_verify` compilation.

The overlay also extends the existing blocked nested-owner release test to
replay owners and adds exact focused JSON equality/borrowed-owner checks. Those
`rust_verify` tests await the combined source build; their pending status is
recorded rather than presented as passing.
