# Shared-cache foreground filesystem boundary

This correction addresses [issue 76](https://github.com/Chelis-Lang/sail/issues/76):
the old lookup read signing trust before entering its helper timeout, while setup
and status performed configuration filesystem calls on their callers. Stalled
reads could bypass the documented bound. The correction moves those operations
onto owned helpers and makes optional status snapshots unavailable while busy.
It does not identify the cause of historical HTTP timing failures.

The original 250 ms lookup deadline begins at request entry. Configuration,
trust, client construction, entry validation, coalesced reply copying and final
admission share that clock. Existing signatures, trust reload cadence, solver
binding, backoff and fallback remain unchanged. One presence helper per tier can
wait at most 100 ms for a fresh presence result; old results are ineligible.
Foreground publication/cleanup uses try-locks, completed owners are reaped outside
the table lock, and retirement checks exact owner identity. The existing 64
lookup-helper limit also covers blocked helper publication.
The clock is a fallback/admission deadline; OS scheduling and condition-variable
reacquisition are not interruptible hard real-time guarantees.

Tests ran in a private Linux x86-64 target under Docker Desktop emulation on
macOS arm64, with two build jobs and serial independent cases. Other product,
source and corpus work overlapped. No causal scheduling claim follows from that
observation. The [machine-readable record](m7-shared-trust-foreground.json)
binds each measured source/binary/log scope; it retains all failed drafts.

Three counterfactual controls restore the old foreground trust read,
configuration read and blocking pending publication in a private diagnostic
copy. Each fails its original bounded-return assertion. The configuration case
uses an actual FIFO; cleanup releases the reader before reporting the failure.
These controls establish the foreground blocking defect. They do not establish
a complete HTTP timing or full-workspace acceptance pass.

The final source passes 23 shared unit controls, 26 unchanged signed-store, HTTP,
stats and tier integration controls, and strict all-target Clippy. The preceding
exact-owner source retained
an original eight-reader timeout: one request and seven coalesced followers,
with publication 216.052 ms after first lookup entry. That measurement alone
does not establish the caller's final admission time. Additional diagnostics
retain caller outcomes/elapsed times without changing the Hit assertions or
deadline: all eight returned Hits in 200.656–205.224 ms in that subsequent run.
It did not reproduce or explain the retained failure. Complete integrated gates
and native CI remain required.

Reproduce the affected checks with a private target:

```sh
cargo test --locked -j2 -p sail-daemon --lib shared:: -- --test-threads=1 --nocapture
cargo test --locked -j2 -p sail-daemon --test shared --test http_shared \
  --test stats --test tiers -- --test-threads=1
cargo clippy --locked -j2 -p sail-daemon --all-targets -- -D warnings
```

User behavior and malformed-key inspection are documented in
[the shared-cache guide](../http-shared-cache.md). Startup no longer enumerates
malformed key files; the existing explicit trust-list command retains those
diagnostics. Status absence or an empty key snapshot does not diagnose a trust
error and never authorizes an entry.
