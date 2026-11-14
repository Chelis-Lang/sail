# Raw SMT solver backends

Status: development candidate ported from the retained cvc5 spike. Current
implementation is uncompiled and native qualification is pending. ADR0134
records the policy and authoritative release/option sources.

The default config remains Z3-only. An explicit cvc5 config uses its own pinned
identity, option/reply dialect and cache policy. The Linux contributor fixture
installer copies only the binary and its original notices after both SHA-256
checks. No Mac binary, released availability or speed gain is claimed.

Required evidence includes exact legacy Z3 argv/key bytes and capability output,
per-solver native direct comparisons, models/cores/multiple checks, unknown and
resource negatives, cache isolation/corruption, process failures/cancellation,
full gates, independent review and Mac before advertisement. Consumer skill
and released user workflows stay unchanged while this config is unadvertised.

Documentation changed: ADR0134, this validation record, errors and changelog.

Initial native source8f73ab5e stops before compilation: Docker's explicit
build-context allowlist omits the new fetch helper. The original image-build
failure is retained. The corrected source includes the helper and binds the
allowlist itself into image tags; native gates must run again.

Local Python validation of the original official archive agrees with both pins
and all eight curated executable/notice files byte-for-byte. This performs no
solver execution and is not native backend acceptance. Four synthetic helper
controls,27resource copies, shell syntax and book source/build/links also pass.
