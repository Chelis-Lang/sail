# Explicit-files sysroot failure detail control

The test `explicit_file_failure_retains_its_first_observer_detail` covers the
second native observer in the failure-detail change. It first acquires the
original complete Baseline over fresh fixture files. Within this test only, it
replaces the namespace observer with a real observer over a disjoint file, then
writes the actually leased compiler-library fixture. The production sysroot
roots, acquisition policy, check order and refusal rules are unchanged.

The original explicit-files observer must reject that real content write. The
control checks the exact outer `sysroot-namespace-invalidated` label, the
`explicit-files` role and `namespace-tracked-input-event` detail, then checks that
the first detail stays sticky. It fabricates no observer result or event and does
not weaken the original rename, new-target, mapping, hardlink or owner-loss
controls. Local owned helper/observers are released before fixture cleanup.

Formatting and diff checks pass. Linux execution is pending root's targeted
validation; no native or full-gate pass is claimed by this source-only handoff.
The [historical D20 audit](m7-owned-reader-d20-sysroot-audit.md) remains unresolved
because that proof recorded only the coarse outer label. A later detailed
response cannot explain an earlier proof retrospectively. See
[coverage diagnostics](m7-proof-coverage-diagnostics.md) and
[freshness](../freshness.md) for the user-visible distinction between successful
obligations and complete Current evidence.
