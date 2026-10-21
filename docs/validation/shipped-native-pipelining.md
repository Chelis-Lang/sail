# Shipped-proof native pipelining

**Status:** fixes the controlled CapybaraKV cold regression (#307). Final
native conformance and composed release qualification remain in progress.
This is a bounded Linux result, not acceptance of the complete release matrix.

The shipped-proof observer now applies accepted ADR 0061's ordering: capture
present compiler inputs privately, run the compiler, then require complete
producer certificates, unchanged bytes and the compiler's read document before
publishing a certificate. A run-bound owned producer mark permits waiting only
while an admitted observation can still publish. Missing, failed or older-run
producers immediately miss; marks never authorize reuse. Verified-vstd lookup
retains its full admission checks.

Native Linux, c7i.8xlarge, us-east-1; runtime `f8589ed4`, upstream `7d829946`,
Rust 1.99.1 with rust-src. Three rotated stock/before/after rounds use the same
CapybaraKV source path, fresh build and Sail cache per sample, and fetch locked
dependencies outside the timer. Before CLI SHA-256 is
`cbe30b785e686490e4c0577dd51087a7d0bfa81d6f005166efa6dba8d4cab6d5`;
after source `828780e5`, CLI
`82e27f56977c6414874f38568f6eb680aec80fe01144dea74f0ff39d30af03a9`.

| Arm | Round 1 | Round 2 | Round 3 | Median |
|---|---:|---:|---:|---:|
| Stock | 54.12 s | 54.11 s | 54.23 s | 54.12 s |
| Before | 67.07 s | 67.23 s | 67.25 s | 67.23 s |
| After | 52.88 s | 53.22 s | 53.19 s | 53.19 s |

All nine runs exit zero with identical stdout (2060 vstd and 732 project
obligations verified). Separate instrumented before/after controls each record
469 solver sessions and the identical multiset of complete solver-input hashes.
The CapybaraKV dependency configuration still misses the shipped proof; this
result preserves ordinary verification rather than disabling proof observation.

A fresh template with the after artifact passes the release first-use witness:
a release-origin vstd proof and exactly one verifier invocation. Its unchanged
call is Current with zero verifier invocations and two reused targets.
The producer witness covers metadata preceding its certificate, exact output
bytes, absent producers, old-run marks and producers exiting without a record.
The composed Mac host gate passes 905 tests with 37 skipped and Clippy clean;
clean Mac performance remains pending.
