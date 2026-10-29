# HTTP shared-cache validation record

Validation evidence for the optional HTTP shared cache, moved out of the
[user guide](../http-shared-cache.md). The guide describes behavior; this
record holds the tests, the developer serving controls and the integration
evidence, with the source commits they apply to. The related M7 gates are in
[M7 validation](m7.md).

## Validation

Fast tests cover writer/reader roles, untrusted signing keys, tampered and misfiled
entries, unknown refusal, interrupted and oversized uploads, path traversal,
concurrent immutable publication, chunked responses, redirects, content encoding,
stalled responses, CA trust, hostname mismatch and same-key coalescing.

The real-Z3 test fills a writer cache, publishes replies and the session certificate,
then uses an empty reader cache. Replies match direct Z3 byte for byte and the
reader starts no additional solver process. It runs in the existing solver gate:

```sh
SAIL_TEST_Z3=<pinned-z3> cargo nextest run -p sail-shim \
    --run-ignored only -E 'test(fresh_reader_replays_whole_session_without_starting_solver)'
```

Long corpus validation remains local-only. Test CA and private-key fixtures under
`crates/sail-daemon/tests/fixtures/` are exclusively for loopback tests; renew their
leaf certificate before October 2027. They must never be used in a deployment.

## Developer serving tests

The concurrent-reader, paused-initiator, status-refresh and disconnect regressions
establish their real HTTP transport and signing trust on an owned setup helper
before testing serving. Setup has its own two-second bound, records its actual
duration, and must send zero requests. Serving still uses the original 250 ms
lookup deadline, the server's 75 ms response delay, and the original exact
Hit/Rejected, one-request, signature and initiator-state assertions. Credential
rotation and invalid-credential rejection still run against the real transport.

Cold construction is tested independently by pausing the actual constructor.
Followers and status must remain bounded, no request may start while it is paused,
and a constructor completing after published fallback must skip that obsolete
request. A timeout is a safe miss; it never authorizes an unverified entry.

This separation follows the retained integrated run at source `d12d4af`, where
five HTTP fixtures failed before or during their first cold lookup. For example,
client construction consumed 152 ms before a paused-initiator test's network
request, and the validated result was published after 398 ms. The 250 ms serving
assertion failed as recorded. That run remains failed; establishing future
fixtures does not reinterpret it as a pass or change a production deadline.

Dev/test builds compile `curve25519-dalek` arithmetic with the production release
optimization level, while retaining debug assertions and overflow checks. The
same strict signature verification and all negative trust/binding checks run on
every entry. This removes measured unoptimized arithmetic work from the serving
budget; the 250 ms deadline and 75 ms delayed-server assertions remain unchanged.
CPU and wall-time evidence, including the original failed established-transport
test, is retained in
[M7 validation](m7.md) (full record in the `archive/m7-contracts-f7103bf`
tag). It is a development-build
improvement, not a production-release speedup or an attribution of every earlier
wall-time delay to cryptography.

The stalled-response integration test measures cold transport construction, the
original 50 ms request against a 300 ms server delay, and destruction of the
owned blocking client inside its unchanged 250 ms total bound. Its phase output
separates those costs without moving initialization or the runtime join outside
the assertion. HTTP lookup phase diagnostics retain the original signed-entry
checks and 75 ms response fixture; isolated CPU and TCP controls are diagnostic
evidence, not substitutes for the complete concurrent lookup checks.

[The retained HTTP follow-up](m7.md) records a
negative small-response TCP control and a fixed strict-signature CPU control.
Neither establishes the cause of the preserved full-gate failures.

### Setup-owner fixture (#102)

The constructor/reload tests now configure their loopback endpoint explicitly.
Their old automatic-discovery fixture could correctly return no setup owner
when presence discovery exceeded its 100 ms bound; `wait_finished` then
unwrapped `None`. This explains the setup-owner flake, independently of the
unresolved HTTP serving timings below. Production discovery, constructor and
serving deadlines are unchanged.

A regression holds the real presence-read hook while preparing the fixture.
The preceding fixture fails the owner assertion; the explicit fixture starts
its constructor without entering discovery. All five constructor tests pass
on macOS with the correction, preserving reload counts, zero requests,
cancellation and the original bounds. The full macOS host check also passes
(613 tests, 35 solver tests skipped); log SHA-256
`e927dc092faeb696fb9e9520060f359bc4a6f3ebb22012f6432e3ea276cba829`.
The combined native Linux workspace gate also passes at `f9e33238`, joined
with `aabf3a15`: 633 tests, 35 skipped. Its complete source manifest and raw
archive are indexed in the [MCP fixture record](mcp-fixture-retirement.md).
There is no verifier, solver, cache-key or corpus behavior change.

Retained attempt: `1bc27f87` plus the regression, 2026-10-16, failed the
owner assertion with discovery paused; raw log SHA-256
`fca2764d7d2283d119a16d0da7ebe820d92886d6f731ff9c7cb0825946f38b18`.
Corrected five-test log SHA-256:
`0407208c222650b06281045c9124a91a2b813817de585270943c09db9cb0816c`.

## Current integration evidence

Local HTTP controls and signature regressions retain the same 250 ms lookup
bound and every trust check. The `f9e33238` native workspace gate passes,
including these controls. The earlier `749255e` workspace attempt failed
two HTTP elapsed/serving cases; those failures remain in [the phase record](m7.md)
with their unresolved cause. A later passing gate does not establish a stable
performance gain or explain those earlier delays. An unavailable or slow
shared store still falls back to local verification; no timeout, unsigned reply
or diagnostic timing authorizes a cached proof.

### Rejected transport adapter experiment

Production retains the pinned reqwest blocking transport. An owned async-client
candidate passed 39 affected security and lifecycle controls, including the
unchanged eight-reader, one-GET fixture. Its once-only complete-operation
comparison was slower: combined helper and I/O CPU rose from 66.264 ms to
84.160 ms, and lookup-to-publication wall time rose from 185.680 ms to
222.422 ms. The smaller body-adapter cost did not justify selection.

The unselected patch and exact limits are retained in
[ADR 0037](../decisions/0037-owned-async-shared-http-transport.md) and the
[comparison record](m7.md). Both measured
fixtures passed the original 250 ms deadline and 75 ms response delay. Those
separate passes do not erase the failed full gate or qualify live readers.
No trust, signature, TLS, payload, timeout or fallback contract changed.

The selected source implementation reads an exact small HTTP body within the
existing 8 KiB application buffer, avoiding the extra read used only to observe
EOF. Larger and unknown bodies keep capped streaming. All 38 affected controls
and strict checks passed, along with the read-count counterfactual/restored
control and both original eight-reader comparison arms at the unchanged 250 ms
deadline. The current workspace gate passes as recorded above; the isolated
comparison does not qualify installed-product performance.
The single observed pair supplies no stable speedup or historical failure cause;
see the [actual controls](m7.md).
