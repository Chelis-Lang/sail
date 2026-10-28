# Exact input ownership and acknowledged solver delivery

Status: candidate extracted from the existing acceptance work, based on the
command/persistence stack at `2ac97a50`, preserving the original extraction
from `2baa1336`. Whole-root discovery, controls and
native qualification remain pending; see the
[qualification record](../validation/checked-input-prefix-delivery.md).

`sail-key::PrefixHasher` executes the shared `key_prefix` framing through the
private SHA adapter. The actual typed factory initializes the solver fields,
mode and original v2 domains. `SessionPrefix` owns both the original transcript
and its incremental key, so a detached feed cannot silently replace that
session's command bytes or interval. The current lexer supplies the original
command head; the existing parser grammar defines complete framing and trivia.

The cached shim transfers its actual owned child-input descriptor into
`InputDelivery`. Successful full writes feed an independent prefix. Failed or
partial writes permanently withhold eligibility. Sealing compares that stream's
silence key with the parser owner's key, freezes further writes and produces
one private `DeliveredInput`. Terminal metadata and completion precede closing
the descriptor, preserving the output thread's existing completion order.

The complete `prefix` root includes the full source-shared operations, typed
factory, actual lexer and all constructive witnesses. Its primitive contracts
are explicit: SHA append/copy/finalization, and successful owned-pipe writes.
The native adapters are reviewed and hash-bound rather than Stock-compiled.
Matching digests require the existing cryptographic assumption; neither a
digest nor a delivered-input token supplies solved-result origin.

The extraction preserves the current held-response storage, statistics order,
response splitter, complete `output_side` and retained-reply comparisons. The
historical implementation predates these connections and cannot replace the
current shim wholesale. In particular, the current `ServedReply` and
`ReplayCheck` behavior remains required. These are extraction review hazards,
not newly established main defects.

The next origin boundary still needs exact stdout/stderr/termination and actual
image identity tied to every writer and protected filesystem transition. The
compiler closure, complete request inventory and whole-service composition
remain in the owning acceptance design; this slice adds no broader theorem.
