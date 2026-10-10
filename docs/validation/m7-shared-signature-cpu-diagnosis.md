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
separately during fixture setup. Unavailable CPU evidence remains explicit.
These diagnostics print no credential, payload or environment contents.

The fixture's 75 ms response delay, 250 ms serving deadline, transport/trust
readiness, coalesced request count, strict signature checks, forged-entry refusal
and independently bounded cold-constructor regression remain unchanged. Test
instrumentation adds work and is diagnostic, not a product latency claim. No
crypto optimization or trust reuse is justified until the measured CPU interval
establishes a cause. The original failed gate remains preserved.

A fixed direct control verifies 16 authentic entries and refuses 16 entries with
a modified signed publication field as `BadSignature`, using the same
`verify_entry` path and trusted key. It records wall time and calling-thread CPU
ticks with no network or serving deadline in the measurement. Each cryptographic
outcome is asserted; the diagnostic does not replace the real HTTP regressions.
