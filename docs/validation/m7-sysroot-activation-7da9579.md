# Original managed scenario after sysroot handoff activation

The original standalone managed scenario failed its first 240-second cold
check on source `7da9579f4aeebafdecf977aeb97a8daf8023740f`. It used the sealed
Linux Sail/shim and pinned binary Verus in a fresh native fixture and cache,
without preparation, manually submitted fill hints, or copied receipts.
The unchanged harness, timeout helper, product/runtime identities, raw artifact
hashes, phase rows and cleanup observations are in the
[portable record](m7-sysroot-activation-7da9579.json).

The controller finished with exit 1 after 245.121 seconds, including the
bounded timeout observation and original cleanup. The first managed operation
timed out at its original 240-second limit and produced no proof report. Warm
reuse, public CLI/MCP and restored-mtime mutation checks were not reached.
The captured process snapshot was explicitly partial; it does not establish
the timeout cause. The last progress record started `vstd`, and stderr ended
while compiling `verus_builtin_macros`. No completed verification bucket was
recorded. This run used Docker Desktop emulation on a shared development host;
concurrent work does not by itself explain the timeout.

Cold runtime context took 21.628 seconds, including 15.673 seconds of original
sysroot hashing and 2.893 seconds acquiring the original guards. The optional
handoff phase lasted 0.749 milliseconds, and no fill-hint phase was emitted.
Independent reproduction found a deterministic activation defect: serializing
the byte-keyed environment as a JSON map fails for every nonempty environment.
Scope construction therefore returned no scope, so this product could perform
neither Lookup nor Prepare. The missing hint is not evidence that the original
caller guards failed. A separate fix serializes sorted byte pairs through a
bounded streaming digest; this failed run is preserved unchanged.

The harness and managed process were absent after cleanup, the private scratch
directory was removed, and the two recorded helper/Cargo descendants were
non-running zombies. Product and runtime hashes matched their original grant
after the scenario. These observations cover recorded actors, not every host
process.

No sysroot reuse, warm speedup, live-reader latency acceptance or final source
engine qualification is claimed. The original limits and assertions remain
unchanged. See [guarded-sysroots.md](../guarded-sysroots.md),
[ADR 0031](../decisions/0031-guarded-sysroot-digest-handoff.md) and
[managed-cold-start.md](../managed-cold-start.md) for the contract and earlier
standalone results.

An independent private-target check of the selector correction passed both
exact-byte and oversized-selection regressions in 0.13 seconds, plus strict
all-target Clippy in 18.22 seconds. Its
[frozen source and control record](m7-sysroot-selection-controls.json) includes
the initial validation-snapshot compile failure and the exact sealed timer
dependency used to correct that snapshot. These controls do not replace the
failed original scenario or establish an actual warm handoff.
