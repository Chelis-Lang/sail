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
