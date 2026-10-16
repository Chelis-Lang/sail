# Warm frontend: recovered profile and first implementation (#162)

The remaining floor is real. A comment-only edit still takes 25.33 s on
Vest, 24.03 s on Verismo and 4.79 s on IronKV through managed Sail. The
first implementation candidate removes unused AST reconstruction during
Verus reachability pruning. [Qualification](../validation/warm-frontend.md)
records the current checks and remaining gates. Persistent frontend reuse remains unimplemented;
the measured raw bucket key does not survive ordinary edits.

## Environment and limits

These are recovered native EC2 c7i.4xlarge measurements in us-east-1, Intel
Xeon Platinum 8488C, 16 logical CPUs. The instrumented fork identifies itself
as `520f3b57949d9b27924e0243ea6788bbf43a1d40`, based on Sail's pinned
`73dea021` fork and upstream `426d8b01`. Rust is 1.98.1 and the pinned Z3 is
used. The raw headers report Sail 0.1.0 without a source commit; these
profiles therefore guide a candidate, not acceptance of a final Sail build.
The corpus pins and commands are recorded in `scripts/perf-real.sh`.

Each project has two rounds, each mode has one cold run, three comment-only
reruns, three edits and a final comment-only rerun: 32 samples per project.
All 32 samples succeed on each of Vest, IronKV and Verismo. The no-op table
uses all eight comment-only reruns per mode, including the final rerun.
These force the frontend to run; they are different from an unchanged
Cargo invocation or a complete managed receipt hit.

Verismo's inconsistent trusted axioms remain a limitation on proof evidence
(see the corpus entry and the real-project performance report). Its timings do not establish soundness.
No new conformance qualification is inferred from matching successful exits.

## Where time goes

Seconds, median of eight comment-only reruns per mode:

| Project | Plain instrumented fork wall | Managed Sail wall | Rust | VIR | verify-crate |
|---|---:|---:|---:|---:|---:|
| Vest | 38.88 | 25.33 | 6.47 | 5.93 | 8.62 |
| IronKV | 8.90 | 4.79 | 1.49 | 1.24 | 0.86 |
| Verismo | 33.52 | 24.03 | 7.57 | 5.51 | 7.51 |

The three stage columns describe the managed arm. Rust includes its initial
work and EXEC compilation. VIR includes type checking and lowering. These
stages do not partition command wall time: Cargo, wrapper work and other
Verus stages account for the rest. Stage timers below are not added to
these totals again.

Vest and Verismo's managed comment-only samples record zero solver starts,
zero cache puts and all daemon gets hitting. IronKV records one solver start
and 141 puts despite every 3,479 daemon gets hitting. IronKV is a
cache-served rerun with a remaining solver session, not a proven all-hit run.

The managed text renderer rebuilds the ordinary timing table from JSON and
does not print the instrumentation's extra stage sections. The separate
direct-shim experiments retain those sections. Four warm samples show:

| Project | Verus total | Bucket phase wall | Bucket share ceiling | Pruning CPU sum |
|---|---:|---:|---:|---:|
| Vest | 22.89 s | 6.53 s | 28.5% | 19.40 s |
| IronKV | 3.84 s | 0.57 s | 15.0% | 0.53 s |

The bucket share is an upper bound before key cost, changed buckets and
restoration overhead. Pruning CPU sums work across threads and is not
critical-path wall time. These direct-shim runs did not retain daemon or
per-session counters, so they cannot independently certify complete hits.
The old M8.0 25% gate is not met by this table alone.

## Why the raw memo key is insufficient

The instrumented proxy hashes a bincode serialization of the complete
pruned VIR crate. Its SHA-256 is not a qualified frontend key.

| Change against the first comment-only rerun | Vest matching SMT digests / buckets | Vest matching proxy keys | IronKV matching SMT digests / buckets | IronKV matching proxy keys |
|---|---:|---:|---:|---:|
| Next comment-only rerun | 200/200 | 200/200 | 34/34 | 0/34 |
| Body edit | 199/200 | 0/200 | 33/34 | 0/34 |
| Inserted comment | 200/200 | 0/200 | 34/34 | 0/34 |
| Inserted lemma | 197/200 | 0/200 | 33/34 | 0/34 |

The SMT digest is measurement instrumentation, not Sail's content-addressed
cache key or a proof of stream identity. The proxy includes source-location
and AST identity data, and serialization also needs deterministic ordering.
The precise cause of each miss is not established by these hashes. Dropping
spans blindly is unsafe: `Span.id` participates in resolution and erasure
maps. Any memo needs a complete semantic input boundary and current-source
diagnostic binding, as proposed in ADR 0052.

## Invalid Anvil scope and the corrected command

All 14 managed Anvil samples fail before verification, with:

```text
partial verification selector: `--verify-module`
Error: Partial verification must use `cargo verus focus`
```

The plain arm correctly uses `cargo verus focus`. The managed arm uses
`sail verify`, which deliberately preserves `cargo verus verify`'s refusal
of partial selectors. The roughly 0.01 s refusals are failed samples, not
speedups. Four focused plain samples succeed, but there is no valid paired
managed result and no Anvil frontend acceptance.

The benchmark now has an explicit `fork-shim` mode. It runs the same
`cargo verus focus` command as the plain arm, with `sail-z3`, the fork's
pinned solver and an owned cache daemon. This measures the shim and does
not claim managed CLI support for partial verification:

```sh
SAIL_PERF_MODES='fork-plain fork-shim' \
SAIL_PERF_FORK=/path/to/qualified/fork/runtime \
SAIL_PERF_OUT=/short/path/anvil-profile \
  scripts/perf-real.sh run anvil 2
python3 scripts/profile-frontend.py /short/path/anvil-profile
```

No replacement Anvil measurement has run yet. The original failures remain
in the evidence. The stage analyzer counts failed samples and labels missing
instrumentation unavailable instead of inventing zero-cost stages.

## First implementation and subsequent reuse

Reachability pruning used an identity rewriting visitor, allocated a new
function/datatype/type tree, and discarded it. The fork candidate `2f9669d2` uses the same
generic visitor recursion, postorder callbacks and scope handling in its
read-only `Walk` mode. Existing immutable AST nodes are reused. A unit test
compares the old and new expression/type/place event sequences and binding
scope, and proves the old identity traversal rebuilt its root. No persistent
key, proof cache, solver input or diagnostic formatting changes.

This is a concrete floor optimization, not completion of across-edit reuse.
It requires independent review, a measured native gain and exact whole-corpus
SMT/verdict/diagnostic validation before a runtime is selected. An unchanged
query's bytes must remain identical; a different verdict is a blocker.

Building proc macros at opt-level 3 is another candidate: six warm samples
per arm show Vest 38.72 to 36.58 s and IronKV 8.89 to 8.27 s. One cold sample
per arm worsens from about 73 to 85 s and 42 to 56 s, respectively. That is
an incomplete experiment, not an accepted default or a cross-corpus claim.

After these cheap measurements, the across-edit prototype must demonstrate
stable complete keys and current-source diagnostics before skipping work.
A raw key that misses every body edit cannot establish a performance gain.

## Evidence

Raw logs remain outside the repository in the
`frontend-review-2026-10-16` evidence set, recovered from lanes `m11-8a` and
`m11-8b`. Original archives remain preserved separately.

| Artifact in that evidence set | SHA-256 |
|---|---|
| `m11-8a/out/vest/results.tsv` | `31fd2bf26eea7baff8b7cdea4a3e6fb2dad6b15ded175f6d2d691f097a1d32e2` |
| `m11-8b/out/ironkv/results.tsv` | `1a8dfbfe27b3754a165da8701fd184eaaa2cc0a4c4a4f6286c07f1cc6ae2132c` |
| `m11-8b/out/verismo/results.tsv` | `3673a5e0092758f4606631ef8f5254e6cf79de27cca7bd0e423dcf59bbc22990` |
| `m11-8a/out/anvil-sail/results.tsv` (14 failures) | `ba13d90dcf5d482aca689799e42814795c25b14de01ec88ee9e4116654dcbcc0` |
| `m11-8a/out/anvil-plain/results.tsv` | `140521950d2336b337236ac3a54860d0fc6791eac8c1c1c4254134190f4a4394` |
| `m11-8a/exp/vest/noop1.json` | `70586c3b8a0fa73abc0e596646715c56c039b020b15803a9ced8dc170abc17ae` |
| `m11-8a/exp/vest/comment-top.json` | `a733d75168e5bcb49dadfe226d076d5c8d22d9f913470ca684a5357d1c98cddb` |
| `m11-8b/exp/ironkv/comment-top.json` | `62b21a0a26745cbfe363db94ed2649c6a90321d8962363f619ba58c1430687ac` |
| `m11-8a/exp/vest/hits0.out` | `46869b4ade41d488ec10ee7a5d4950169bc596317a20d8c9bb62c37579f7c03d` |
| `m11-8b/exp/ironkv/hits0.out` | `6bf376f28f151dce52afbe3a9b55a9f6bbe060a739b34e8cca75d781ef701ed4` |
| `m11-8a/exp-vest.log` | `6f38eb444f272b159a0853ef3178c1bdf6e500f29306d4b7b0bb9779223ec0b0` |
| `m11-8b/exp-ironkv.log` | `6c92e2c58c6e1dbe6ee0fcdc39a8b19461adfbcc3fedb413f9fec51915fa13d8` |
