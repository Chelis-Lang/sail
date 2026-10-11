# Enabled guarded runtime identity reads

The production Linux path now uses the exact sidecar eligibility established by
[the initial component controls](m7-runtime-sidecar-components.md). One fixed
boolean on the original validation owner replaces the redundant hot parse when
both raw sidecar paths are covered. Canonical-only aliases or absent membership
keep the original parse. Cold initial and final identity reads, complete runtime
validation, native owners, request keys, generation/pointer checks and every
per-read freshness barrier remain unchanged.

The baseline comparator is test-only. Its switch defaults to enabled so ordinary
unit fixtures exercise the same path as production. The same six real mutation
controls passed after selection in 7.683 s, and release all-target strict Clippy
passed in 5.935 s. The private target reused existing dependencies, with two build
jobs and incremental compilation disabled. No timing comparison, project proof
or live census was repeated.

The existing owner layout includes the boolean; there is no added parsed graph,
path inventory or shared owner. This does not create an aggregate memory or RSS
claim for the existing index or its evicted in-flight owners. Dirty owners still
lose their runtime/coherent state; the bit cannot restore them.

This is a small redundant-read removal. The earlier component p95 improvement
from 1.142 ms to 0.343 ms does not establish a remedy for the separate 51.489 ms
freshness tail or the complete Result 343.142 ms reader failure. Original
collector code, complete report graphs, four-reader concurrency, 200 samples,
200 ms threshold and five-second RPC bounds remain unchanged. Full integration
and live reader qualification remain separate work.

The [production check record](m7-runtime-sidecar-production.json) retains exact
source and test-binary identities, passing controls, original initial failures
and an inventory of the separately sealed raw archive. The initial component
archive is unchanged.
