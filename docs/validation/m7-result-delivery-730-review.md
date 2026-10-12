# Complete Result delivery on product 730

This file-only review binds source `730a3210151ca51b7e011121c4795553b0cfa3e5`.
The [source inventory](m7-result-delivery-730-source.json) records exact bytes.
No Rust change, Docker call, target read, compilation, daemon, proof or collector
ran. The independent runtime-owner candidate remains test-only and unselected.
The [recorded complete original reader workflow](m7-reader-730a321-terminal.md)
now passes both unchanged collectors and retained CLI/MCP. The optional designs
below are deferred, unnecessary for the current original bound, unselected and
unmeasured. This review adds no measurement or production change.

## Current pass and evidence boundary

Product 730 passes four readers and 200 samples per unchanged collector, with
all Current, revision and live full-verifier predicates intact. Python Result
p95 is 94.197 ms and Status 59.896 ms; Rust Result p95 is 67.483 ms and Status
50.273 ms. All are strictly below the original 200 ms bound. Python receives
and parses the complete 4,311,179-byte Current graph; Rust decodes the complete
typed graph. The retained CLI/MCP workflow passes its original 120/30/5 bounds.
The [terminal record](m7-reader-730a321-terminal.json) retains immutable seals,
normal service exit and empty cleanup/archive/finalization error arrays.

This pass is a different, unpaired scope from the historical failure below.
It does not isolate a single optimization, establish a paired speedup, erase
any failed scope or complete M7's separate real-project performance gates.
No snapshot, Status or extra runtime-owner candidate supplied this pass.

The [older descriptor product46 census](m7-descriptor-reader-terminal.md) failed
Result p95 at 343.142 ms with all 200 Python samples admitted and the same real
verifier continuously active. Each Result body contained 4,313,743 bytes. Only
42 Current Result rows survived in its recorder tail: every row hit an encoded
frame and had no encoding interval. Socket-output p95 was 194.522 ms, freshness
51.489 ms and stdout read/hash 15.537 ms. These independent quantiles cannot be
added, and those tail rows cannot be joined to particular client samples.

The [same-call transport audit](m7-cached-result-transport-review.md) finds
pre-output p95 63.794 ms by subtracting each row's own write duration from its
own total. The [separate transfer control](m7-result-transfer-profile.md)
demonstrates that concurrent full Python parsing can prolong sender wall time.
Larger socket buffers did not materially improve its complete-client p95. That
control uses reconstructed key order, not captured wire or fresh authority.
Neither result identifies the historical live wait or justifies a buffer change.

Product 730 includes selected guarded-output overlap and exact unchanged metadata
preservation. Their [output component](m7-result-output-overlap-native.md) and
[metadata component](m7-preserve-freshness-metadata-replay.md) remain narrower
than the live reader criterion. The current full-client pass does not isolate the
guarded/fallback ratio or a causal contribution from either component.

## Current hot path and allocation map

| Stage | Actual work and ownership | Remaining cost boundary |
| --- | --- | --- |
| Accept and request | `jobs.rs::endpoint` polls a nonblocking listener, sleeps 10 ms on WouldBlock, admits at most 32 endpoint workers, then starts one thread per accepted connection. Request read is length/version bounded; socket timeouts are five seconds. | Connection, scheduling and request decode occur before any complete response. Recorder dispatch starts at accept and cannot measure waiting before accept. |
| Job lookup | `jobs.rs::get` clones the Stored record under the jobs mutex. The record contains submission/status/projection metadata, not the complete result graph. | Metadata Strings/maps are copied; size depends on the actual submission. No graph or socket output is protected by that mutex. |
| Frame key | `store.rs::result_frame` serializes Stored metadata and hashes it. `ResultCache::get` locks its bounded map long enough to clone an Arc. | One metadata Vec/String per key calculation. No full project JSON is encoded at this stage. |
| Guarded output | `output_service.rs::Queue::validate` reserves 16 MiB scratch, retains the SAME charged owner, checks namespace, releases output locks during independent input freshness, then checks pointer identity and the final complete guard. | Scratch admission and each try-lock can refuse. Final refusal discards its prospective currentness and repeats original complete output and input checks. No per-reader owner or output graph is allocated. |
| Independent freshness | FreshnessIndex runs all original guards, but deep-copies InputSnapshot path inventories for the validating caller, coherent publication and overlapping followers. Daemon classification consumes only revision and complete. | Inventory copies allocate Vec/BTree/path storage; a coherent follower copies under the runtime mutex. Their cost has not been isolated. Every native/helper/kernel/mount, checkpoint and namespace check remains mandatory. |
| Original output fallback | `store.rs::project_bytes` reads the complete retained stdout into a bounded Vec and compares SHA with the exact projection seal. Freshness follows it. | Allocates actual stdout bytes per fallback. Unsupported, busy, missing, dirty or quota-refused guards do not bypass this work. |
| Encoded hit | `Frames::select` chooses a complete Current/Historical/Unknown variant and clones byte/family Arcs. `WireFrame` retains the physical family charge through eviction and output. | No complete graph clone, parse, serde rebuild or complete byte copy. Cache locks are released before socket output. |
| Encoded miss | The same sealed stdout is fully parsed. Repair retains BOTH the result and full repair envelope; all three currentness variants are fully encoded. | Typed graphs, clones, Values and temporary serialization buffers exist on this path. Uncached encoding is outside retained-cache quota. Historical hit evidence does not justify a new miss optimization. |
| Stream output | The endpoint calls write_all on the existing complete header/body slice, then flushes. Each client has its own stream; no global output/cache mutex covers this send. | Kernel copies, receiver progress, backpressure and scheduling can hold a writer. Wall duration is not sender CPU. |
| Original Python client | Every call opens a fresh socket, sends the same request, receives the complete bounded frame using recv/bytearray/bytes, and executes full json.loads inside the timed call. Admission and verifier continuity checks follow timing. | Python graph allocations/copies and concurrent parsing remain inside the original oracle. Rust decoded caches and MCP copy changes do not remove this Python work. |

The original census uses four executor threads and 200 alternating Status/Result
samples: 100 timed calls of each operation. It also checks the running active
job before and after each sample. Those active Status reads take the noncompleted
fast path, whereas timed completed Status reads always validate complete stdout.
The complete graphs, original Python source and this timing boundary remain fixed.

## Deferred design: immutable input snapshot delivery

`freshness.rs::refresh_outcome` copies validation.inputs before checking the
original owner, then copies it again into slot.coherent. `await_validation`
deep-copies a qualifying coherent snapshot under the runtime mutex. These
InputSnapshots include roots/exclusions and two sets of exact path inventories.
`store.rs::classify` only needs revision and complete, preserving its existing
revision-mismatch, Current, Historical and Unknown branch order. This allocation
seam exists on both Status and Result; no measured share of 51.489 ms is claimed.

If later scaling measurements show this copying is material, the design is an
immutable snapshot inside the existing RuntimeValidation/coherent state, plus a
lightweight daemon
inspection result. The original validation still runs before return: same
request, full owner, entry checkpoint, 100 ms owner wait, final pointer/generation
and outer namespace checks. Native loss still invalidates and runs the original
refill/fallback. A payload Arc or old coherent timestamp supplies no authority.
The existing full inspect API must keep returning independently owned full
InputSnapshots; only daemon revision/complete consumers avoid deep inventory
copies. Do not shorten any guard, remove source/runtime paths, retain a new lease
owner, or change the original classification order.

Internal snapshot Arc shells and in-flight lifetime must be accounted, with no
external mutable sharing or new unbounded retention. Existing source-owner
storage remains separate from the 128 MiB frame quota; this design does not
invent a global heap ceiling. Exact full/minimal classification equivalence,
independent mutation of full caller snapshots, body/config/sidecar/hardlink/mmap
changes, helper loss, old checkpoints, context mismatch, generation/owner
replacement and cancellation remain necessary controls. A copy-count sensitivity
control should identify the removed inventory copies; whole-operation comparison
is still required. This design is unimplemented, unselected and unmeasured;
the current bound already passes without it.

## Deferred design: completed Status output checks

`store.rs::status_traced` still always reads and hashes complete stdout even when
the exact same completed job already has a continuously guarded cached output.
Unlike Result, it never uses `Queue::validate`. This is concrete repeated server
work competing with Result readers; it is not an established cause of Result
p95 and no saving is claimed. If future workload measurements support this
cost, the narrow design is to share the existing output-only owner for completed
Status:

1. Derive the same exact Stored key and retain its existing charged frame family.
   Do not create a new guard, cache Currentness or fill a frame in a Status RPC.
2. Use the unchanged SAME-owner namespace/input/final barrier operation, with
   its existing 16 MiB scratch admission and try-lock refusal. Fresh Status
   currentness still comes from the original independent inspection.
3. Return only the normal freshly cloned Status after success. On any miss,
   contention, substitution, mutation, cancellation or quota refusal, use the
   original complete stdout read/SHA plus independent freshness unchanged.
   Noncompleted and old projection-free records retain their existing paths.
4. Reuse the existing bounded observational output_guarded field only after
   actual successful Status coverage; update its documented operation scope and
   exact parser controls together. Do not infer authority from that field.

No extra owner, helper, queue or retained graph is needed. The existing 128 MiB
family charge survives in-flight eviction. Additional simultaneous scratch
admissions could nevertheless make guarded Result refuse more often. Selection
therefore needs a mixed four-reader complete-operation comparison, not a Status
microbenchmark. This design remains unimplemented, unselected and unmeasured;
it is unnecessary for the current passing original census.

Required controls include exact Status equality for Current/Historical/Unknown,
no graph copy or owner construction, missing/old records and quota fallback;
output mutation/restored bytes and mtime; owner replacement between independent
input and final checks; live charged eviction; concurrent Status/Result overlap;
and shutdown/helper loss. A restored-output or substituted-owner sensitivity
control must fail the prospective fast path and exercise original fallback.
Both full Result graphs, sizes, wire order and original output errors must stay
exact. The original four-reader/200-sample strict p95 below 200 ms and five-second
RPC limits remain mandatory for any future implementation.

## Admission and delivery limits that require evidence first

The output constructor is one-shot per encoded generation. `Queue::offer` marks
frames.offered before scratch/retained admission or active-slot acquisition; a
transient busy/quota refusal leaves that generation without another offer. Hits
do not call offer. This can explain a permanently absent optional owner in code,
but no current row proves that event occurred. output_guarded=false also covers
lock contention, construction, unavailable guards and final refusal. Changing
this policy would alter an explicit one-shot control and needs a concrete refused
generation plus a separately reviewed bounded admission policy. No retry is
proposed or introduced by this review.

A frame miss can incur full encoding and output construction; a hit cannot be
made cheaper by another serialization memo. Guarded Current hits with small
pre-output cost and large write cost alone support no copy/serde change. No
additional transport change is selected. Existing write_all already sends the exact complete slice. Larger
socket buffers, graph trimming, compression/new protocol, altered Python parsing
and changed reader concurrency or deadlines are outside this review.

A transport design should be considered only after concrete attribution of
accept/dispatch or send CPU versus receiver backpressure. The current bounded
ring has no per-call identifier joining all client samples and can overwrite
early rows. Missing coverage must remain explicit; stage quantiles cannot
manufacture a complete-client explanation. The current original census has
passed; no additional candidate is needed for that observed scope. These designs
are deferred for potential future scaling, not remedies needed to clear its bound.
