# Warm frontend qualification (#162)

**Status:** profile recovered; first implementation independently reviewed,
unit-checked and built as two native research runtimes. The bounded native
Vest/IronKV comparison passes its research checks; overall edit gain remains
inconclusive. No frontend performance change is selected or shipped.

## Candidate and checks

- Fork source `2f9669d2`, based on `a9ce7c13` / runtime `73dea021`: reuse
  existing immutable AST nodes during read-only reachability traversal.
- Local macOS arm64, Rust 1.98.1, pinned `engine/verus/Cargo.lock`.
- Both `vir` library unit tests pass. The new regression compares identity
  rewriting with read-only traversal: exact expression/type/place callback
  order, binding scope and root reconstruction. Clippy of all `vir` targets
  with warnings denied passes.
- Benchmark command regression: all four tests pass. They verify focused
  versus full Cargo-Verus commands, the fork solver/shim/owned socket binding,
  and refusal of unknown modes. No compiler or solver runs in these tests.
- Separate key-projection source `2371ca51` on the `520f3b57` measurement
  branch: four JSON serialization/isolation/scope tests pass; `rust_verify`
  checks successfully; clippy for the selected projection test passes. It
  measures a possible key and never skips fresh lowering or verification.
- The reviewed pruning patch also builds on combined fork `8a373f81` on
  native Linux c7i.8xlarge, Rust 1.98.1. Both A/B `vargo` logs explicitly
  execute `vstd_build`: 2,060 verified, zero errors. Every full build and
  subsequent Cargo-Verus release build succeeds. Runtime copies and all
  source/patch identities are hashed in the build manifest. These direct
  compiler research runtimes have no Sail source-build manifest; no Sail
  runtime or corpus conformance has been qualified.
- The bounded project controller restores pristine source, manifests and
  lockfiles before each arm and in its final cleanup. Its research gate
  requires positive verification, matching verification/proof-note JSON and
  complete raw SMT stream multiset equality. It returns nonzero on failures,
  retains raw output unchanged, and always leaves default acceptance false.
- Corrected controller (SHA-256 prefix `da694112`) completes with exit 0: all 20 verification
  samples pass. All eight cold/warm A/B verification and proof-note JSON
  comparisons match; complete raw SMT multisets match (266 Vest streams and
  25 IronKV streams per arm). Runtime hashes are unchanged. Two rounds,
  with arm order reversed, give Vest edit medians 35.46 to 35.21 s and
  IronKV 8.863 to 8.887 s. This does not establish a useful overall gain.
  Direct-Z3 timings include solving; no all-cache-hit scope is claimed.
  IronKV's generated member lock was not retained, limiting cold attribution.
- A disabled observer is built on combined `8a373f81`,
  without pruning or span projection. It captures bounded ordinary AST JSON
  and explicitly incomplete context/fresh-SST observations. It has not been
  qualified for reuse. Source `80a19b1a` builds on native c7i.4xlarge,
  us-east-1; vstd explicitly verifies (2,060/0), and both unit tests pass.
  Seven offline Python controls pass for lossless positional
  analysis, original span fields and strict raw-byte exit statuses. This does
  not establish a complete key or diagnostic rebinding. Independent review
  and complete native capture remain separate; see the report's bounded plan.
  Five capture-controller controls also pass, including observed default-off
  emptiness and refusal of incomplete/modified evidence. The first direct-runtime
  capture fails on incomplete SST Debug in both edited buckets.
  Source `7812863f` raises only the bounded record capacity to 256 MiB,
  retaining the separate 64 MiB raw-hash cap and all completion/equality
  checks. It also builds, explicitly verifies vstd (2,060/0), and passes both
  unit tests. All ten project control/capture runs verify and all 16 bucket
  manifests complete. Same-source raw SMT and verification/proof-note checks
  pass in both projects. Comment SMT also matches; both body edits differ
  and remain recorded. This passes only the limited capture gate: default
  acceptance is false, and offline AST field analysis remains pending.

Raw logs are in the external `frontend-review-2026-10-16` evidence set:

| Artifact | SHA-256 |
|---|---|
| `build-compare/native-build-result/manifest.json` | `b751dad93f2561e3cf42b88d8c40215213cca3a43acef9ba4c35839a43c70e53` |
| `build-compare/profile-attempt1-results.tgz` | `867c51156034f4d041ccad2d7406ec2052ede9783c423e01ef6e639de0047ee3` |
| `build-compare/native-profile-v2/manifest.json` | `bc2f1f7426108b22d61494a7aeefd142a6ac2a86faed643bc0d2255fa0181d15` |
| `build-compare/m8-native-build-profile-v2.tgz` | `3aa85479b6d3bce3320a0ef4853611058d4b8794b7d2eeaa66b10e08b4a686e2` |
| `raw-input-probe/m8-observer-attempt1-complete.tgz` (first runtime, complete raw refused captures/source) | `1e63601527929baa602212b86303ba331d5d8155ca99b5ce6556fcefb0ed42a1` |
| `raw-input-probe/m8-observer-attempt2-complete.tgz` (revised runtime, source and every raw capture) | `ec08c6220ce5a4717dd89ba94b713f920878d3794b0d28166793e20f7785f0a5` |
| `readonly-walk-test.log` | `d81f488c81e29e7d99eea3820c890651e96deb8b24f5715a5f63428d90d6b969` |
| `readonly-walk-clippy.log` | `a2db7a82dd2f91484b2976605b34d3f8a3421b54e72cb806cf4121e09eadabbf` |
| `key-projection-test.log` | `20ca1717e589501587d25697e33c53fc662ed5e08c5cfe547cae1bf5edafcb77` |
| `key-projection-check.log` | `1951f1dd0bdd7d2c195b9ac7196ec09df9aea98a3353fe843631181f74270b2f` |
| `key-projection-clippy.log` | `95e83b967575ed6a1de786feb1306ee655f450f6265e1d8957212a0e547928aa` |

## Profile scope and failed attempts

The [profile report](../reports/2026-10-16-warm-frontend.md) identifies
source/runtime/environment and each raw input's checksum. Three projects
have 32 successful samples each; these are profiling evidence with the
stated source-attribution and conformance limits. The intended all-hit
IronKV scope still has one solver start. Direct-shim timing samples lack
independent cache coverage counters.

- `520f3b57`, 2026-10-16: all 14 focused managed Anvil attempts fail before
  verification. Cause: `sail verify` correctly reproduces the refusal of
  partial selectors under Cargo-Verus `verify`; the plain arm uses `focus`.
  No paired Anvil speedup or frontend acceptance exists.
- `520f3b57`, 2026-10-16: raw pruned-VIR key proxy does not support measured
  body/comment edits: zero matching keys despite many matching SMT digests.
  Exact cause and a complete stable key remain unresolved. Persistent reuse
  is not implemented.
- `8a373f81` plus reviewed pruning patch, 2026-10-16: first direct native
  comparison interrupted after five successful Vest samples. Harness review
  found missing pristine-input restoration, strict mismatch failure status
  and interrupted-source cleanup. Raw results and the original controller
  remain preserved; no acceptance is inferred. The corrected controller is
  a separate attempt.
- `80a19b1a`, 2026-10-16: first observer runtime builds and project
  verification passes, but both edited buckets' SST Debug exceeds the
  63 MiB payload bound. Both captures refuse; controller exits 1 and later
  edits do not run. Control buckets fit. Raw partial files remain incomplete.

## Remaining gates

Native Linux gates, macOS and Linux runtime regressions, deterministic
exact-SMT comparisons, full fast/standard conformance and native performance
measurements remain required. The
comparison must include cold and edited runs and every recorded failure;
previous timings do not qualify the candidate. A positive native gain must
be established before a fork runtime is selected. A bounded next comparison
would use at least three interleaved comment/body edit loops for each direct
compiler and full-prefix shim arm. An all-hit claim requires complete
current-invocation provenance, zero solver starts and matching daemon
hit/get counts with no puts. The generated member lock must be pinned and
retained. A negligible or inconclusive result does not justify adoption or
an expanded benchmark catalogue. ADR 0052 remains proposed.
