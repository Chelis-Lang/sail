# Warm frontend: profiles, traversal experiment and input diagnosis (#162)

The remaining floor is real. A comment-only edit still takes 25.33 s on
Vest, 24.03 s on Verismo and 4.79 s on IronKV through managed Sail. The
first implementation candidate removes unused AST reconstruction during
Verus reachability pruning. [Qualification](../validation/warm-frontend.md)
records the current checks and remaining gates. Its overall edit gain is
inconclusive and it stays unselected. Persistent frontend reuse remains
unimplemented; the recovered raw bucket key does not survive ordinary edits.

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

### Native research runtime comparison

The read-only traversal has passed independent static review. Two complete
research runtimes were built natively on an EC2 c7i.8xlarge in us-east-1
(32 logical CPUs, Xeon Platinum 8488C). Arm A is combined fork source
`8a373f81a507b7ef4f144767ccd237d3d3de6c48`; arm B applies the reviewed
read-only pruning patch, SHA-256
`e5333c1c71292a6f939a99b75e1fb7de0337942645bbdc2282998e2e08ccfc58`.
B is identified by that source and patch, rather than claiming an unmodified
`8a373f81` build. Rust is 1.98.1. The native CI image is
`sail-ci:3c02e445e25df766`, image SHA-256
`03d6865e6602ae702f480b4396543c5c1c51fe07f88fd80faa210880a66abef6`.

Both `vargo build --release` logs explicitly invoke `vstd_build` and report
2,060 verified obligations and zero errors. The subsequent Cargo-Verus
release builds also succeed. The copied runtimes have identical `verus`,
`cargo-verus`, pinned Z3, `libvstd.rlib` and `vstd.vir` bytes; only
`rust_verify` and the vstd fingerprint differ. These are direct compiler
research runtimes. They do not contain Sail's source-build manifest and
are not selected or qualified Sail runtimes.

The bounded comparison is implemented in
`scripts/profile-readonly-pruning.py`. It verifies all runtime hashes against
the build manifest, uses each arm's absolute Cargo-Verus path and sibling
compiler, and binds its own Z3. Two rounds reverse A/B order. Each arm starts
from the same pristine project manifests, lockfile and source, with an empty
project target directory. Cold and comment-only edit runs are both retained.
Separate raw SMT captures are excluded from timing samples. The controller
requires positive verification counts, identical verification/proof-note
JSON and identical complete SMT stream multisets, including multiplicity.
Any failure makes the research gate fail with a nonzero exit. Raw stdout,
stderr, timings and failures remain unchanged. This research gate does not
replace corpus or diagnostic conformance.

The corrected native controller completes with exit 0: all 20 verification
runs pass, including the four separately timed capture runs. All eight paired
cold/warm verification/proof-note JSON comparisons match. Complete raw SMT
multisets match exactly: 266 streams for each Vest arm and 25 for each IronKV
arm. Runtime hashes remain unchanged and tracked project inputs are restored.
The research gate passes; default/corpus acceptance remains false.

Medians of two samples per arm and phase, with captures excluded:

| Project | Metric | A: existing traversal | B: read-only traversal |
|---|---|---:|---:|
| Vest | Cold command wall | 64.77 s | 63.96 s |
| Vest | Comment edit wall | 35.46 s | 35.21 s |
| Vest | Comment edit child CPU | 192.26 s | 191.83 s |
| Vest | Reported Rust / VIR stage | 6.969 / 5.668 s | 6.954 / 5.353 s |
| IronKV | Cold command wall | 41.00 s | 38.37 s |
| IronKV | Comment edit wall | 8.863 s | 8.887 s |
| IronKV | Comment edit child CPU | 22.566 s | 22.592 s |
| IronKV | Reported Rust / VIR stage | 1.492 / 1.167 s | 1.490 / 1.113 s |

VIR stage time falls in both projects, but the overall improvement is small
and inconclusive: Vest improves by about 0.25 s (0.7%), while IronKV's median
edit is 0.025 s slower. Vest's warm samples are 35.21/35.71 s for A and
34.96/35.46 s for B; IronKV's are 8.838/8.887 s and 8.787/8.988 s. Two
rounds do not establish a useful default gain. Cold times include dependency
resolution and download/cache warming, so their difference cannot all be
attributed to pruning. Vest's resolved lock hashes match across all four
arms. IronKV has no committed lock; the generated member lock was not
retained by this controller, which only records a root lock. A future floor
comparison must pin and retain that resolved lock before timing. No sample
is omitted, and these measurements do not select the candidate.

For an already prepared owned build directory, inside the same native
environment:

```sh
python3 scripts/profile-readonly-pruning.py \
  --work /path/to/prepared/research-build \
  --rlibs /path/to/existing/gate-target/release/deps \
  --timeout 600
```

The directory must contain the exact A/B `build-results/manifest.json` and
runtime copies. The helper compiles only a small patcher against existing
read-only Sail library artifacts. It creates fresh owned project directories,
preserves all raw evidence and restores edited project inputs even after
interruption. It refuses to overwrite an existing result directory.

Direct-Z3 edit timings include solving and do not isolate the frontend floor.
If the stage measurements justify another experiment, the bounded follow-up
is three interleaved comment-only and single-body-edit loops on Vest and
IronKV, comparing both compilers directly and through the full-prefix solver
shim. The cache preparation is real work and must be recorded separately.
An all-hit label requires complete current-invocation provenance, zero solver
starts, every daemon get hitting and no puts; missing sidecars or mere
regex counts do not establish that scope. No managed runtime identity is
invented for these direct compiler runs. An inconclusive gain keeps the
candidate unselected; persistent-key design remains the larger task.

Building proc macros at opt-level 3 is another candidate: six warm samples
per arm show Vest 38.72 to 36.58 s and IronKV 8.89 to 8.27 s. One cold sample
per arm worsens from about 73 to 85 s and 42 to 56 s, respectively. That is
an incomplete experiment, not an accepted default or a cross-corpus claim.

The separate measurement-only candidate `2371ca51` extends the original
timing branch. With `SAIL_M8_KEY_PROXY=relocated` and `--time-expanded`, its
JSON identifies `key-policy=span-relocated-v1-unqualified`. Span identifiers
are mapped bijectively in encounter order; the projected serialization writes
empty packed-position and display-string fields instead of their real values.
Their irrelevance and a current-diagnostic mapping have not been proved;
no cached artifact is loaded and fresh lowering/verification always run.
Four focused serialization/isolation tests and a `rust_verify` check pass.
Native key hit rates, exact fresh outputs and diagnostic rebinding remain
unqualified. The raw key policy has its own hash namespace. This experiment
does not change the selected runtime.

After these cheap measurements, the across-edit prototype must demonstrate
stable complete keys and current-source diagnostics before skipping work.
A raw key that misses every body edit cannot establish a performance gain.

## Next measurement: explain raw input misses

Prepared source on combined fork `8a373f81` adds a disabled observer, without
the pruning optimization or the `2371ca51` span projection. Source `80a19b1a`
builds natively, explicitly verifies vstd (2,060 verified, zero errors), and
passes both observer unit tests. Its first project capture refuses incomplete
SST output, so no key-stability result is established. At most two exact
buckets in one named crate are captured per compiler process. Capacity
revision `7812863f` reserves 255 MiB for
artifacts and 1 MiB for a manifest per record; raw bincode hashing retains
its separate 64 MiB work ceiling. Failed/truncated representations remain marked incomplete;
all fresh lowering and verification still run.

The observer writes ordinary pruned-AST JSON, a bounded raw bincode digest,
available lowering/context Debug observations and fresh SST Debug. The AST
retains `Span.id`, `Span.data` and `Span.as_string`. Existing serialization
omits opaque `RawSpan`. Packed span data contains crate identity and source
positions, not a source-file digest. The lowering call also reads selected
functions, pruning results and `GlobalCtx`; some private maps, warning state,
graphs, counters, rustc callbacks and imported-source mappings are not fully
represented. Existing Spanned-node Debug in SST prints display locations
instead of their packed positions and span IDs. These are incomplete
observations, never sufficient keys or
reusable lowering artifacts. Diagnostic rebinding remains unproved.

Offline analysis uses:

```sh
python3 scripts/frontend-input-diff.py before-pruned-ast.json after-pruned-ast.json
python3 scripts/test-frontend-input-diff.py
```

The analyzer preserves object-pair order and duplicate occurrences, array
positions, numeric tokens, and every ordinary span field. It counts
positional differences, with at most 256 detail entries; original raw files
remain the evidence. Exit 0 means raw bytes match, 1 means they differ and
2 means analysis refused. Equal parsed values do not excuse byte differences.
Seven Python controls pass, including command exit statuses. Five separate
capture-controller controls reject missing buckets, incomplete or modified
payloads, failed verification and any observer artifact in the disabled
control. The larger-cap revision also builds and passes its two native unit
tests; its completed capture is described below.

The proposed native capture is bounded to one 16-CPU Linux host for at most
120 minutes, allocated only after independent patch review. Build the exact
committed observer candidate with Rust 1.98.1 and the retained source lock;
record all source, runtime, toolchain and pinned-Z3 hashes. Do not present a
direct research runtime as a qualified Sail runtime. For pinned Vest and
IronKV, retain the actual generated member lock and pristine manifests, then
select the edited bucket plus one unchanged control bucket. Capture two
identical-source fresh compilations, a comment insertion and one body edit.
Force the primary crate to compile using package-scoped Cargo cleaning in an
owned target directory; keep dependencies warm and record preparation cost.
Restore edited source in a final cleanup, including interruption. A comment
can affect `line!()` expansion and is not assumed to preserve input.

Every process must preserve complete raw stdout/stderr and status. Check
expected bucket count, manifest completion and every artifact digest; a
missing bucket or capped artifact is a failed capture. Retain independent
full SMT streams and verification/proof-note JSON. Byte inequality stays a
failure, not a normalized match. Use the ordinary fresh path with the observer
disabled as a control. This is input diagnosis, not a latency claim or frontend
reuse acceptance; coverage remains unqualified even when observations match.

`scripts/build-frontend-observer.py` builds that exact observer commit from
the two retained bundles in a new owned native checkout, with eight build
jobs and per-command deadlines. It verifies bundle, dependency lock and Z3
identities, captures the full build and two observer unit tests, copies the
direct runtime and hashes all its files. Generated versioned toolchain
manifests are retained as build inputs; unexpected source changes fail.
`scripts/capture-frontend-inputs.py` consumes the resulting runtime file map
plus an independently built, hashed project-manifest patcher. It refuses an
existing output directory and records every subprocess failure. The exact
selected buckets are Vest's `asn1::der_ord`/`asn1::boolean` and IronKV's
`host_impl_v`/`verus_extra::clone_v`; missing selections fail rather than
silently changing the scope. Its research gate compares same-source raw SMT
multisets and verification/proof-note JSON, observes that disabled output is
empty, and validates all captured artifact hashes and completion flags.
Edited-source raw SMT comparisons remain separate recorded observations;
no equality is assumed. The default candidate gate always remains false.

The first native capture on a c7i.4xlarge in us-east-1 verifies each project
during preparation, the disabled control and the first enabled sample. Both
enabled samples then fail the strict artifact-completion check: Vest's
`der_ord` has complete AST/context but truncated SST at the original 63 MiB
payload ceiling; IronKV's `host_impl_v` does too. Both unchanged control
buckets fit. The controller retains the refusals and exits 1 without running
later edit samples. A single bounded 256 MiB record-cap revision follows;
it changes no field, input projection, raw-hash ceiling or correctness gate.
The original runtime, source and raw captures remain preserved in compressed
`raw-input-probe/m8-observer-attempt1-complete.tgz`, SHA-256
`1e63601527929baa602212b86303ba331d5d8155ca99b5ce6556fcefb0ed42a1`.

The second source (`7812863f`) explicitly verifies vstd again (2,060 verified,
zero errors) and passes both observer unit tests. All ten project control/
capture runs verify, with all 16 selected bucket manifests complete. Both
research capture gates pass: the disabled observer emits nothing, exact
same-source SMT multisets match, and verification/proof-note JSON matches
across each project's five samples. Comment-edit SMT multisets also match;
body-edit streams differ in both projects and remain recorded as different
inputs. The default candidate gate remains false. This is complete capture
of the stated, incomplete observations, not complete-key/output qualification
or a latency measurement. Offline field analysis remains pending.
The compressed complete attempt, including actual runtime/source and every
raw artifact, is `raw-input-probe/m8-observer-attempt2-complete.tgz`, SHA-256
`ec08c6220ce5a4717dd89ba94b713f920878d3794b0d28166793e20f7785f0a5`.

## Evidence

Raw logs remain outside the repository in the
`frontend-review-2026-10-16` evidence set, recovered from lanes `m11-8a` and
`m11-8b`. Original archives remain preserved separately.

| Artifact in that evidence set | SHA-256 |
|---|---|
| `build-compare/native-build-result/manifest.json` | `b751dad93f2561e3cf42b88d8c40215213cca3a43acef9ba4c35839a43c70e53` |
| `build-compare/profile-attempt1-results.tgz` (interrupted, unaccepted harness attempt) | `867c51156034f4d041ccad2d7406ec2052ede9783c423e01ef6e639de0047ee3` |
| `build-compare/native-profile-v2/manifest.json` | `bc2f1f7426108b22d61494a7aeefd142a6ac2a86faed643bc0d2255fa0181d15` |
| `build-compare/m8-native-build-profile-v2.tgz` (runtimes, source/controllers, logs and all raw captures) | `3aa85479b6d3bce3320a0ef4853611058d4b8794b7d2eeaa66b10e08b4a686e2` |
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
