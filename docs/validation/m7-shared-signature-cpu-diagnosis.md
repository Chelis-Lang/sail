# Shared signature verification diagnosis

The integrated `a9dc753` gate passed 435 of 436 tests. The established-transport
paused-initiator regression exceeded its unchanged 250 ms lookup deadline.
Its body completed at 208.783 ms and classification completed at 344.521 ms;
the caller returned after 251.170 ms. That interval does not by itself identify
cryptographic CPU work or descheduling as the cause.

Test builds now record calling-thread user/system CPU ticks around the unchanged
classification path, alongside its wall time. On Linux the sample reads at most
4,097 bytes from `/proc/thread-self/stat` and refuses data over 4,096 bytes,
malformed fields and arithmetic overflow. It locates fields after the command
name's final closing parenthesis. The actual `getconf CLK_TCK` value is recorded
separately during fixture setup, through an owned `timeout 2s getconf CLK_TCK`
utility invocation. Hosts without that optional utility report unavailable
evidence explicitly.
These diagnostics print no credential, payload or environment contents.

The fixture's 75 ms response delay, 250 ms serving deadline, transport/trust
readiness, coalesced request count, strict signature checks, forged-entry refusal
and independently bounded cold-constructor regression remain unchanged. Test
instrumentation adds work and is diagnostic, not a product latency claim. No
crypto optimization is justified without measured CPU evidence; trust reuse is
not authorized by timing evidence. The original failed gate remains preserved.

A fixed direct control verifies 16 authentic entries and refuses 16 entries with
a modified signed publication field as `BadSignature`, using the same
`verify_entry` path and trusted key. It records wall time and calling-thread CPU
ticks with no network or serving deadline in the measurement. Each cryptographic
outcome is asserted; the diagnostic does not replace the real HTTP regressions.

At 100 clock ticks per second, the unoptimized control recorded 30 ticks and
310.332 ms wall time for the authentic batch, then 31 ticks and 319.907 ms for
the forged batch. Optimizing only `curve25519-dalek` at level 3 in dev/test
recorded one tick and 6.808 ms for the authentic batch and one tick and 7.526 ms
for the forged batch. After the diagnostic utility/lint cleanup, the final control
recorded one tick and 7.438 ms for authentic entries and zero ticks and 3.914 ms
for forgeries. Ticks have 10 ms resolution: zero does not mean zero CPU work.
The same release algorithm, features, strict signature validation, debug
assertions and overflow checks remain enabled. Production release already uses
this optimization, so this establishes a development-build improvement only.

All ten HTTP regressions and five signed-format negative tests passed, including
the unchanged real 75 ms response-delay and 250 ms serving checks. Final strict
daemon/cache Clippy passed. The first diagnostic Clippy run failed the new helper's
line count and an empty assertion; that failure is retained and the helpers were
corrected. These targeted results do not qualify a full integration gate or
establish the entire original wall-time failure's cause. Sources, executable/raw
log hashes, control measurements and exact scope are in
`m7-shared-signature-cpu-diagnostic.json`.
