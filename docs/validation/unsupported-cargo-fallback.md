# Explicit Cargo without the qualified script protocol

An explicitly selected Cargo outside the qualified 1.99.1 source commit
cannot establish build-script proof receipts. Previously it attempted managed
rebuilding on every call, including unchanged passing Anvil builds. Sail now
uses plain Cargo freshness with its solver cache for selected dependency
closures containing build scripts. Missing or malformed observations still
fail closed; unrelated workspace scripts do not trigger the fallback.

JSON remains Unknown with `no_reuse`, no complete or reused target and no proof
receipt. The note explains how to select the pinned toolchain. This does not
remove compiler-byte checks or change receipt admission.

## Native comparison

EC2 c7i.8xlarge, us-east-1, 2026-10-24. Sail `e51e4660` versus that source
plus this fix; both independently staged archives use runtime `7d9d5410`.
Anvil uses its real Cargo 1.98.0 toolchain selection. Three rotated unchanged
pairs follow the retained warm-up and normal real-project harness setup.

| Median unchanged call | Before | After |
|---|---:|---:|
| Wall time | 60.973 s | 0.895 s |
| Actual target verifications | 3 | 0 |

Every call succeeds. Fallback reports contain no crate checks, and stderr has
no `Checking` or `Compiling` lines. The documented NoReuse execution counter
counts one aggregate Cargo invocation, rather than individual target compiles.
No fallback result is reported Current or usable as a proof receipt.

Raw samples, JSON, stderr and setup logs:
`s3://example-evidence-bucket/experiments/2026-10-24/cargo-fallback-paired-anvil-final.tgz`,
SHA-256 `bd09eccfa54a39031f666157b088a31edc500c6f0144aa143493a45e6d291e8d`.

## Gates

On the native Linux host, the workspace gate passes 1,047 tests with Clippy,
formatting, docs, layering and dependency checks clean; solver tests pass all
42. Full E2E, including every phase and extra, passes. The new script fixture
compares failing and passing output to stock, checks unchanged Cargo no-ops,
and requires explicit no-reuse JSON. All PR arms pass 33 Same, and all full
FAST arms pass 147 Same. STANDARD passes all four arms (159 Same and
13,166 functions each), plus all 21 bounded mutation/control comparisons.
The cumulative integration on main `d24297db` passes 1,056 workspace tests,
42 solver tests, full E2E including extra and all four PR conformance arms
(33 Same / 311 functions each). The Mac host gate passes all 1,045 tests
with Clippy clean. No checks are weakened.

Native gate and conformance logs:
`experiments/2026-10-24/cargo-fallback-native-gates-final.tgz` in the same
S3 bucket, SHA-256
`621308b3464cc055d8fcd4effa21595ae13bccb7c18a462e7bed72aac1b19d4d`.

Indexed attempt: initial paired setup selected Rust 1.98 globally, preventing
the harness helper from meeting its MSRV. The rerun keeps the harness on its
pinned Rust and lets Anvil's own toolchain file select Cargo 1.98.0.

Current-main cumulative logs: `experiments/2026-10-24/sprint-cumulative-native-gates-final.tgz`,
SHA-256 `4ceb90e620e303f9712878c88363a0b64a1aed5e8f84f11b328ca24611756c72`.
Native Mac log: `experiments/2026-10-24/sprint-integration-macos-host.log`,
SHA-256 `a70de7b536345919c5ee91035dcb2dcd3dbfbaa241b39632e5296262e3e53479`.

Initial cumulative source copies omitted documentation and were refused by
the gate; the complete Git merge passes. The first Mac run inherited a shared
`CARGO_TARGET_DIR` and failed three fixtures; ordinary Cargo target selection
passes all 1,045, with the failed log retained separately alongside it.
