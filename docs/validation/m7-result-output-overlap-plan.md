# Retained output overlap candidate

Status: source-only candidate on `perf/m7-result-delivery`, based on
`db4c65fe8221884983326c8259a7c2f065df7693`. Compilation, affected controls,
complete-operation comparison and product selection are pending. No proof,
daemon, reader census or acceptance retry has run for this change.

## Evidence and supported change

The [original descriptor reader](m7-descriptor-reader-terminal.md) retained all
200 valid samples and verifier-continuity checks. Status passed at 120.526 ms
p95; complete Result failed at 343.142 ms. Its 4,313,743-byte complete response
and full Python decoding remain required. Rust and retained CLI/MCP were
unattempted. The 42 retained Current Result timing rows all hit preencoded
frames; output sealing, freshness and socket write p95 values were 15.537,
51.489 and 194.522 ms respectively. These tail rows are not full 200-sample
attribution, and independent quantiles cannot be added.

That workflow predates installed guarded output ownership. No existing timing
row proves an overlapping read took the guarded path: `frame_hit` establishes
only immutable frame reuse. The separately recorded full-graph component
measurements do not support another graph-encoding memo or socket-buffer change.

Current `output_service::Queue::validate` holds the frame's output-owner mutex
across the independent input/runtime freshness operation. A second reader
cannot acquire this output mutex and takes the complete read/hash fallback,
even when the first reader is doing no output work. This is a concrete redundant
operation in the source; its frequency and cost in a genuine live census have
not been measured.

The candidate retains the exact charged output owner in an `Arc<Mutex<_>>`.
It acquires the original namespace barrier with a nonblocking owner try-lock,
releases both output locks during independent input freshness, then rechecks
that the same owner is still published and performs the original complete
final output barrier. Both guards and the prospective input result belong to
the same request interval. No Currentness value is cached.

Missing, busy, cancelled, dirty or substituted coverage returns no prospective
result. `store::result_frame` then performs its unchanged complete stdout
read/SHA and independent freshness checks. Report graphs, response variants,
framing, diagnostics and all existing deadlines remain unchanged. A continuously
guarded owner retained after index eviction can still validate; eviction alone
does not change Current to Unknown.

## Ownership ledger

The same 128 MiB family ceiling remains in force. One background construction
retains its existing 16 MiB owner reservation and 64 MiB scratch reservation;
foreground inspection reserves its existing 16 MiB scratch before barriers.
No helper, thread, graph or constructor is added per request. An `Arc` clone
adds a stack reference, not another owner allocation or refundable allowance.

The owner reservation now follows the actual last reference instead of the
frame cell alone. Replacement, eviction, a held request and shutdown cannot
release it while its helper or observer remains alive. Field drop order disposes
the guard before its charge. The permanent admission margin includes each
bounded endpoint's retiring owner mutex/Arc allocation shell and the sole
constructor's shell, as well as the existing stack/control/Frames margin.
This is requested owned storage accounting, not allocator overhead or RSS.
Refused constructor publication drops its candidate outside the frame-cell lock.

## Controls prepared, not yet executed

Six Linux controls use actual independently constructed output guards:

1. Pause the first independent input operation and require a second reader to
   validate before releasing it. The original lock-across-input implementation
   must fail this sensitivity control.
2. Overwrite and restore output bytes/mtime during overlapping input operations;
   both final barriers and a later attempt must refuse the dirty owner.
3. Remove or substitute a different independently clean owner between barriers;
   the earlier prospective result must be discarded.
4. Evict the frame family during input validation; the same clean retained owner
   may finish, and its charge survives until the final owner reference drops.
5. Shut down during input validation; final coverage refuses while the retained
   owner remains charged through disposal.
6. Pause the actual owner charge refund before the outer Arc allocation is
   freed; permanent admission headroom must still cover that physical shell.

The original store controls continue to require complete frame equality,
busy/quota fallback, final-barrier restored-write fallback, exact SHA mismatch
refusal and oversized output refusal. Existing primitive replacement, rename,
hardlink, mmap, mount-generation and helper-loss controls remain required.
These component closures supply prospective values to test bracketing; they do
not establish a verified project's Current authority.

## Diagnostic attribution and next validation

The optional fixed recorder adds `output_guarded`: `true` only after completed
same-owner validation, `false` before original full read/hash fallback, and
`null` for operations where neither path was attempted. It remains default-off,
with the same latest-256 ring, nonblocking storage and shutdown-only export.
The maximum-width row/export control and exact-schema retention fixtures are
updated. Old frozen launchers and historical records remain unchanged; a future
candidate diagnostic needs the updated launcher identity.

Before selection, run affected native controls and strict checks in a private
target, then compare the same complete validation operation under four readers.
Include both output barriers, independent real input freshness and any fallback,
rather than claiming savings from the paused mutex control. The original live
four-reader/200-sample strict 200 ms p95 and 5 s RPC oracle is unchanged. Neither
this candidate nor a component pass erases previous failed reader or HTTP gates.
