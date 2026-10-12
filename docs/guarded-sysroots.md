# Guarded sysroot digest handoff

Managed verification hashes its selected Rust sysroots whenever a guarded
snapshot is unavailable. The [012707a diagnostic](validation/m7.md)
found this work took about 16 seconds even
for a warm command that started no verifier. The optional Linux daemon handoff
in [ADR 0031](decisions/0031-guarded-sysroot-digest-handoff.md) is implemented
to reuse those content digests under uninterrupted kernel guards.

The managed runner now attempts this optional handoff from a running local
`sail serve` daemon. No extra command or cache publication is required. The
daemon uses a private user socket and owns the endpoint and maintenance worker
through shutdown. An older or unavailable daemon keeps the original local
hashing behavior. Integration is staged for the original cold/warm acceptance;
primitive controls alone are not a measured verification speedup or acceptance
of the final M7 source engine.

The corrected [1e8b633 scenario](validation/m7.md)
reaches the automatic fill hint but fails its original first cold 240-second
limit. Warm verification is not reached, so background publication and actual
import remain unestablished in that scenario. A separate [pinned context control](validation/m7.md)
subsequently observed natural background publication and imported all 327 eligible
hashes before the second context's local hash loop. That control invoked no
verifier and cannot replace the original failed proof or performance checks.

The handoff is local to a trusted private Sail daemon. It does not read digest
authority from disk, a shared cache or another host. Each caller acquires its
own original file leases and namespace guards before asking for a snapshot,
compares every actual file/path/canonical/device/inode identity, and imports only
files whose first eligible hash is protected by those guards. All engine,
executable and other runtime bytes continue through their original checks.
Already-hashed executable aliases cannot receive handoff authority.

The endpoint requires a cache socket owned by the current user with mode 0600.
An older daemon with a public socket mode cannot provide this handoff; its
clients retain the original local hashing path. Upgraded normal service startup
must make its owned cache socket private before binding the optional endpoint.

A cold, busy, unavailable, incompatible, overbudget or late endpoint returns to
the original local hashing path. The optional request has one 100 ms aggregate
budget and never waits for content hashing. After a locally validated cold
command, a bounded maintenance hint may ask the daemon to independently acquire
its own guards and hash files. It accepts no caller-submitted hashes. Interrupted
construction cannot publish. Unsupported filesystems, native path encodings or
kernel coverage decline reuse.

The caller and daemon guards overlap during a successful transfer. Writes
through hardlinks, writable mappings, alias replacement, changed installed
files, helper loss, watcher overflow and mount-generation changes invalidate
that generation. A caller that has completed a valid overlapping transfer keeps
its own guards through proof and receipt publication; later daemon eviction or
restart does not remove those caller guards. A restarted daemon begins cold.

The daemon retains at most two bounded owners and constructs one at a time.
Shutdown cancels construction immediately and joins owned workers outside
verification RPCs. Cancellation is cooperative between bounded operations;
these checks do not claim to interrupt kernel reads or scheduler suspension.
The full Rust storage and kernel-resource ledger is in ADR 0031. The private
local daemon remains an explicit trust boundary, as described in
[trust.md](trust.md).

Original proof verdicts, diagnostics, cache keys, unknown-serving policy and
CLI/RPC deadlines do not change. Original managed cold/warm checks and corpus
conformance must pass before this staged integration is accepted or any product
performance claim is made. See [managed-cold-start.md](managed-cold-start.md) and
[ROADMAP.md](ROADMAP.md) for current evidence and remaining qualification.
