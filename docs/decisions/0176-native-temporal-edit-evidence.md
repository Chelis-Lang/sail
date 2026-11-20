# 0176. Native evidence for edited temporal jobs

Accepted for the point-release work in [#774](https://github.com/Chelis-Lang/sail/issues/774).

[Exact receipts](0175-exact-temporal-receipts.md) can restore an identical
job by digest. An edited invariant can fail on an earlier state, introduce
an evaluator error or cause a bounded search to stop. A trace that still
replays cannot by itself certify how the current checker would finish.

The authenticated native worker owns one immutable Dive artifact in an edit
session. Dive binds each edit and decides which obligations that artifact can
discharge. An ordered safety prefix can supply a current failure. A complete
safety closure can decide a new invariant. A complete labelled temporal graph
can decide a new temporal property and its current false-property set. A
complete ordinary safety proof can discharge an ordered subset of its prior
invariants, including on aggregate state, without retaining the state graph.

The shared runtime keeps its existing receipt format. An invariant subset can
also be checked across processes. The old completed safety receipt supplies
the proof and worker identity. A new invocation of that same worker binds the
previous and current closed models and certifies their relation. The initiating
CLI combines the two and attributes the result to the current job. Neither a
relation by itself nor a previous refutation can supply this proof.

The separate edit-session transport uses the existing authenticated image and
deadline supervisor, a bounded request and a bounded whole-group response.
The parent verifies each basis and current digest. The batch carries artifact
kind, prior counts, current decision position and current independent ITF
validation. A failed operation falls through to ordinary native checking or
ends with an operational failure. The runtime never imports a partial batch
as an exact receipt.

The implementation uses the source-development dependency provision of
[ADR 0172](0172-native-temporal-in-main-workspace.md). Pin the full upstream
revision for both Dive dependencies and check the worker's reported source.
Before a stable package publication, move the dependency to published Dive
crates and repeat native conformance.
