# Solver snapshot prototype

Status: disabled research prototypes, with no accepted implementation. The
separately authorized timing-only one-use experiment has partial native
evidence for IronKV and a retained Anvil RSS-observer refusal. No release uses
these candidates. Historical C-snapshot checks below retain their original scope.

Source: hardening immediately after `88076b62`, with native source
`native/sail-snap/sail_snap.c` SHA-256
`feee55f411ae04ed4af4b25db17fc6198c042b8f76b3211bdc5c5777454a100e`.
The runtime oracle for the bounded macOS correctness checks is the pinned
Z3 4.16.0 from the combined `8a373f81` source runtime. These tiny checks exercise
transport and resource admission; they do not measure performance.

| Gate | Result | Scope and limits |
|---|---|---|
| Native preload regressions, macOS | Pass | 9 checks: NUL/string/comment/all bytes, resumed NUL, single and concurrent count, one-byte memory cap, zero count |
| Rebuilt shim with daemon, macOS | Pass | 2 additional cold checks, armed preload and incompatible preload; actual solver start required, stdout/stderr/exit match plain Z3 |
| Selected Rust tests | Pass | `sail-api`, `sail-shim`, `sail-bench` library tests, 78 total |
| Selected clippy | Pass | Those three packages, release profile, all targets, warnings denied |
| C warnings | Pass | macOS `-Wall -Wextra -Werror` |
| Native Linux, full host/gate and conformance | Pending | No new host or competing measurement job started |
| Revised resource-bound performance | Pending | Earlier candidate timings do not qualify these bounds |
| Raw-byte exactness across the corpus | Pending | Masked statistics are not acceptance |

Raw evidence stays outside the repository, in the snapshot review evidence set:

| File | SHA-256 | Result |
|---|---|---|
| `old-tests.json` | `67783489700cf1042efb905639b4a6aaf394f06925c73cb5f63802445aeb8e65` | Old prototype fails 6 of 9 regression checks; count 8/2 and concurrent 32/2, ignores memory/zero budgets |
| `corrected-tests.json` | `d798add7e412242b8faeb8a73a46d02d3cfc375ab90c87501a2efded83e474fc` | Revised library and rebuilt shim pass all 11 checks |
| `rust-tests.log` | `afc098bdb87429b606ff08b24db3039937438be0d6fec2e811310b21bd5ba883` | 78 selected Rust tests pass |
| `clippy.log` | `a7531379dd862d1e047ebcce1ce5da565ab3ffa88129aa25cf5c35cac54a4f00` | Selected packages and all targets pass |

Failed/interrupted attempts remain visible:

- `88076b62`, 2026-10-16: original preload changes real NUL-containing input
  and exceeds parked limits; deterministic negative controls above.
- Candidate hardening, 2026-10-16: the first shim regression omitted creation
  of its provenance directory, so solver start was unestablished; the malformed
  preload also exposed premature forwarding of loader stderr. The final test
  requires provenance and the implementation defers that forwarding until the
  preload acknowledges its framing.
- Earlier native `exact-ironkv-base` run: statistics comparison differences
  reported; no claim of a raw exactness pass. The recovered experiment's
  zero-record verismo output does not qualify that project.
- Earlier Anvil managed partial-selector measurements: invocation refused
  before verification; retained as failed samples in the recovered archive
  cited by the [report](../reports/2026-10-16-solver-snapshots.md).

The source walk, syscall interposition, stale/partial socket recovery,
platform behavior and active-worker policy need independent review before any
default integration. [ADR 0051](../decisions/0051-exact-solver-snapshots.md)
remains proposed.


## Process-state admission and strict raw probe

Candidate C source SHA-256
`8a5d586e610544250c0c14bccf87d19de4dd51dd3a0a2af50ea97ba1abb6cf72`.
The bounded Mac runtime for this follow-up is pinned Z3 4.16.0, SHA-256
`edae32f9e37ea4b5bb35310d72f0e352d0dc07626cac4e9e30bc1ea9a5bc8efb`.
No Mac performance claim or native Linux execution is inferred.

All 16 local correctness checks pass, including actual pinned idle/global
Z3 timeout workers and an output-channel descriptor. The unchanged C-state
fixture additionally covers another thread, an interval timer, a finite CPU
limit and a foreign descriptor. The nine earlier framing/budget checks remain
strict and pass. The old preload fails exactly the seven new admission checks,
publishing an unsupported snapshot in each. The Linux POSIX-timer case and
Linux procfs/thread/fd APIs remain pending native execution.

The strict raw probe proves an empty-output prefix, records the original input,
runtime/library digests, every raw stdout/stderr stream and every exit status,
and compares two fresh runs, marked and repeated resumes without masking.
It covers one first-push point of one copied IronKV transcript only, not later
output-bearing prefixes or cross-stream output interleaving. Both raw attempts
fail on clock-statistics bytes, and the revised run also observes fresh-versus-
fresh clock variation. No admission test, source review or non-statistics
agreement is promoted to corpus acceptance.

External `snapshot-exact-review-2026-10-16` evidence:

| Artifact | SHA-256 |
|---|---|
| Actual pinned timer-thread probe | `9f1925cadef7dc272dbcdfb7f2c1161f0cf512209481eb0fbf3d4e870c361da1` |
| Old preload, 7 of 16 checks fail | `3f64a851854c6b7a9150fd6196cb84769810cb57f75afc5d2d53bae0f1fcd764` |
| Final candidate, all 16 checks pass | `9d3f5a7e3204752b1c03e19114e109a2466cc96253cc7d995277cd69f64a7e3f` |
| Old preload strict raw manifest, failed | `73e152d77f43695a62fa146c1aa14b9cedca2d64c9deb3d222460de054b5bb1a` |
| Admission candidate strict raw manifest, failed | `4e7269b7cb2f770f6ffc21044020e5c099edadf89fa03edcd483ae506515b126` |
| Indexed primary pinned source | `8adb549507b1f230f3c91ab46d5b7662fa746ab12e0fbc7cd5dfe1d6393c46c5` |

Failed setup attempts stay visible: initial raw-probe setup used an unsupported
`uname` convenience method before starting any solver; corrected before the
recorded attempts. Initial admission compile failed a signedness warning;
the dependent invocation had no valid library and cannot qualify any check.
The first output-file fixture incorrectly compared an absent file with an
already appended file; both runs now start from the identical absent-file
state and compare exact side effects, streams and exit status.

Bounded native handoff, after a host is allocated and otherwise idle:

```sh
native/sail-snap/build.sh /tmp/sail-snapshot-check
python3 native/sail-snap/test.py --lib /tmp/sail-snapshot-check/libsail_snap.so --z3 Z3
python3 native/sail-snap/raw_audit.py --z3 Z3 --lib /tmp/sail-snapshot-check/libsail_snap.so \
  --input INPUT --offset 105643 --resumes 3 --out NEW_EVIDENCE_DIR
```

The offset above is valid only for the retained input SHA-256
`78df424db1a37494205ed9164b9bb376c3321edd91b073dfa7401ce72175fa5b`.
Do not apply it to a different transcript. The output directory must be new;
a failed or refused sample remains failed. Full native conformance,
resource-policy review, raw counter qualification and changed-check performance
remain required before selection.


Admission naming is now version 3, while lossless framing and the three-stream
wire protocol remain version 2. The preload must acknowledge `sail-snap/3`;
a library without these gates is incompatible and falls back before input.
Names under the old admission namespace cannot discover an earlier parked
generation. Three selected `sail-api` snapshot tests pass, including the old
namespace exclusion (log SHA-256 `0ffdb4a7f9c867824c51db1acbb0ab8b65d3ef55bef06ad64b1452f56af8f961`). The final
C library still passes all 16 checks with the same result SHA-256 above.
A tiny no-statistics Boolean transcript also passes the new raw probe across
two fresh runs, marked and three resumes (manifest SHA-256 `0eb259e89669cbe09efe91608c2aad4e0b4490c501388681667401ff2316678d`);
this is a transport positive control, not real-project acceptance.


The two retained IronKV raw attempts precede the admission namespace bump;
their library digests are recorded in their manifests. The final version-3
source is qualified locally only by the 16 admission/framing checks, the tiny
raw positive control and the three selected API tests. No final-source native
or real-project raw acceptance is claimed.

All-target release clippy for `sail-api` also passes, with warnings denied
(log SHA-256 `485da955ab634320c2d5067cb64254a19a5beaaa717de9677667f333b03d086e`).


## One-use live-prefix experiment: implementation, not acceptance

A separate disabled prototype keeps the original solver alive and consumes
it once. The first local Mac fixture pass is superseded by an independent,
actual executable pathname ABA: reviewed source SHA-256
`9051aa78377bf31e6029a6068fc17f82977344eb437a5ad5a4c01919f1f851fe`
accepted a running shell substitute after the pinned pathname was restored.
The candidate returned `sat`, fresh pinned Z3 returned `unsat`, both exit 0.
The raw `executable-aba.json` SHA-256 is
`bee4842e33607d1479de2c18025bf9075f9e3818e2e0276ce16b6c5dfb020319`.
An earlier reproduction setup failed its alias-path assertion before the
substitution; the corrected actual wrong-image result is the rejection evidence.

Current code binds actual Linux executable images through a held procfs image
FD and pidfd before any prefix input and revalidates at claim. Mac live reuse
refuses pending an equivalent qualified guard. The current focused Mac refusal
manifest has SHA-256
`de1dc7f0fc49abcd696c1fadd514ccd2eda95d6d9e2518637b9b1ce4c4091440`.
The exact fresh-fallback manifest has SHA-256
`93fbf7bf5bdde53c73e2f453e83f87efefb54ae706bc7dabcc8da4b26308fb84`:
raw streams/status match, but candidate acceptance is false and the sample
exits 1.

The rejected source is preserved externally byte for byte under its `9051`
digest. The frozen Linux-control candidate is `live_prefix.py` SHA-256
`4fe47fe2d6c1708ec0f73ed780054b8c32b59823f5eb75846d1ae3eb95ddf956`;
test SHA-256 `11414c9e1286f042b596a2121056464ae1494f369b9175e65383d1062c472aea`;
driver SHA-256 `1dbc9198e76aaa6a8757a7b9401bffc161f231836bd302d43489b608b376d491`.
That exact core/test/driver passes all 23 controls on native x86-64 Linux,
kernel `7.0.0-1014-aws`, in the existing Sail CI image with pinned Z3 SHA-256
`e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765`.
The native manifest SHA-256 is
`5c9b8579e43adcba0d98aff908623daadc6f1f5a66b74496c330bc23f1fcc833`.
The foreign executable actually runs, its pathname is restored, and capture
refuses; fresh pinned Z3 returns `unsat`, exit 0. Genuine Boolean, NUL-comment,
model and timer continuations match exact stdout/stderr/status, the serving PID
is unchanged, and the idle timer retains two threads. Wrong-prefix, double and
concurrent claims, capacity, RSS, expiry and unexpected-prefix controls pass.
Two initial native launch attempts fail before testing because the supplied
solver paths do not exist; both evidence directories remain preserved. These
are setup failures, not passing controls. The successful run uses the actual
installed solver path after verifying its exact digest.

An owned no-output fixture confirms bounded fresh timeout retention: partial
streams and signal status remain recorded, raw equality and candidate
acceptance are false, and the driver exits 1. Its manifest SHA-256 is
`8093f5eb5d471c616b78c91cb8fbd3a61b7e75f5f1c82ac1b4bd5ca2aa8209d6`;
this tests artifact/error collection, not the pinned solver or native guard.

See the [prototype commands and scope](../../native/sail-live-prefix/README.md).
The driver records long-workload limits explicitly and retains timeout/error
artifacts. Defaults remain five-second correctness limits. Native metrics
include actual preparation work, fallbacks and identity-validation overhead;
a guarded solver probe is not a full plain-Verus benchmark. Session/ledger
eligibility, actual shared-runtime/OS/filesystem scope, unsolicited-output
handling, daemon fairness, full conformance and real edit-loop measurements
remain required. No source inspection or old rejected-source fixture pass
satisfies those gates.

### Real changed-check datasets and bounded-input candidate

Two actual base/body-late pairs have been selected from the retained native
archive (`m112a-results.tgz`, SHA-256
`bcccdcc149d31cb6bef770e5b760897c085d2ebbf7e917d83552c385ffcef573`).
Selection compares exact input bytes, chooses the last complete top-level
push boundary before the first changed byte, and requires previous solved
checks plus a remaining changed suffix. Unchanged sessions are excluded by
exact byte equality, not by statistics or query normalization.

| Dataset | Edited input bytes | Shared prefix bytes | Prior / remaining checks | Edited input SHA-256 |
|---|---:|---:|---:|---|
| IronKV body-late | 2,464,885 | 2,426,183 | 23 / 1 | `c6744afeba46d2277efeadf69af43f8a5ebbeca21a3266d26632403985f6cf87` |
| Anvil body-late | 6,285,771 | 5,658,439 | 7 / 2 | `148bdb444d0b31f998ad989b3919ef86ad90444c40d6febed845760db531e7cf` |

Expected prefix stdout comes literally from each complete, paired, successful
archived native trace, cut at its existing echo-response boundary. The IronKV
prefix contains 80,087 output bytes; Anvil contains 24,799. Clock and allocation
statistics remain present. Preparation may therefore refuse even an identical
logical prefix. These are selected datasets, not current runtime/performance
acceptance. The next gate is three independent full fresh runs per edited
trace, compared byte for byte before any live timing. Every failed sample
remains visible.

Both real inputs exceed the prototype's unchanged 2MiB default. A reviewed
candidate adds an explicit input budget capped at 16MiB, strict integer resource
budgets, oversized-input controls and a separate fresh-only driver mode. It
records source/environment/input digests, all raw outputs and status, finite
run limits, sampled RSS/PSS and native CPU work. All 28 bounded controls pass
on the same native Linux/pinned-solver scope, including explicit larger input,
default and whole-input refusal, the ceiling and noninteger budget controls.
The exact source SHA-256 values are core
`a9704a14845e69c61fca6190b4fb986500612165613d68df99f3ea56918a7cf0`,
test `5f9351cd0408a0081b8c46e39a9e9c9d90fe3cd2db4c3648f49a8851ead6f379`,
driver `cadb166f602bc697b60e8c4c55202b4ff7f02ab6f58fc3b0c13ec061b7e99e67`.
The native manifest SHA-256 is
`e42238f526ce2e1c8d695bfc00fdfa36ea971bb8ce9e4b5591112ab0df406627`.
These 28 controls execute the core, not the driver's fresh-only branch.

### Actual native full-trace fresh controls

The separately reviewed `run-real.py` controller now exercises that branch
with the frozen core/driver above. Controller SHA-256:
`606910691e32f53957c1f58bbbf0dd58860ab08d6fcc65217608a1004f3cc157`.
Native EC2 c7i.8xlarge in us-east-1, Xeon Platinum 8488C, 32 logical CPUs;
Linux `7.0.0-1014-aws`, CI image `sail-ci:3c02e445e25df766`, image digest
`03d6865e6602ae702f480b4396543c5c1c51fe07f88fd80faa210880a66abef6`.
Actual pinned Z3 image identity is verified before input in all six samples.
The limits are 300 s per preparation/serving/oracle/fallback, 600 s TTL,
16MiB input, one slot and 1GiB sampled resident memory. The owned container
also has a separate 2GiB memory limit. No other benchmark runs concurrently.

| Dataset | Fresh samples | Actual check replies per sample | Guarded wall seconds | Literal raw result |
|---|---:|---|---|---|
| IronKV body-late | 3 | 24 `unsat`, no `sat`/`unknown` | 2.190 / 2.178 / 2.177 | Fail: `:time` bytes vary |
| Anvil body-late | 3 | 9 `unsat`, no `sat`/`unknown` | 55.068 / 55.910 / 55.853 | Fail: `:time` bytes vary |

Every process exits 0 with empty stderr. All changed stdout lines in the
recorded comparisons are `:time` fields; the other bytes, including
`num-allocs`, match in these samples. This observation is specific to these
fresh runs and does not resolve historical snapshot allocation differences.
One IronKV sample additionally records `ESRCH` reading `smaps_rollup` during
normal exit, after earlier successful memory samples. The driver already
rechecks child status on observer errors; this sample still refuses and is
retained as an unresolved observer/exit race. No error exemption was added.
Observed peak RSS is approximately 68–75MiB for IronKV and 211–214MiB for
Anvil; these are samples rather than hard peak-memory proofs.

Both strict fresh controls fail. Both one-use samples are explicitly skipped
with that reason; the parent exits 1. Final source and solver identities
match. No live latency, changed-check speedup, default integration or issue
completion is claimed. Raw stdout/stderr/status and all samples remain intact;
no clock or allocation field is masked and the acceptance gate is unchanged.

The external `live-prefix-review-2026-10-16` evidence set contains:

| Artifact | SHA-256 |
|---|---|
| `real-trace-result.tgz` (source, inputs, every raw sample and parent/driver manifests) | `4b43d3a60365266f3f9be19d54e350a9654f07fce4e0395fe619c430f48ac657` |
| `real-trace-result-summary.json` (unmodified-byte difference locations/classification) | `b05dbaf9eca11fb942db7e591aba4a844a9dc062b04db8abc872b94a6b19b405` |

- `60691069` controller digest, 2026-10-16: all six complete fresh controls
  produce the same check replies but fail literal clock-byte equality; one
  IronKV sample also has the retained memory-observer exit refusal. Live
  timing is not run. This is a failed research gate, not a solver verdict
  disagreement or an accepted performance result.

### Driver success and evidence-retention correction

The isolated follow-up changes only the standalone Python driver's success
predicate and final manifest retention. A candidate now requires actual exit 0
from both its process and the fresh reference, as well as the unchanged exact
stream/status/image conditions. Equal nonzero or signal exits no longer pass.
If final pool cleanup raises, the manifest retains that cleanup error alongside
an earlier failure and marks the candidate failed; writing the manifest is
attempted independently of cleanup. No native guard, raw comparison, timing
policy, input or production integration changes.

Eight actual-driver/owned-child fixture controls pass: equal successful exits,
equal nonzero exits, equal signal exits, different statuses, fresh fallback,
final cleanup error, nonzero plus cleanup error, and an original identity
observation error plus cleanup error. Raw NUL/non-UTF8 bytes and child statuses
are retained. Linux/image/RSS admission is explicitly doubled; these are
functional driver tests, not native Z3 or performance qualification. The same
controls against archived driver `cadb166f` fail the two equal-error acceptance
and three cleanup-retention cases. Core `a9704a14` is unchanged, so the native
28 core checks above remain their original source scope. The actual ESRCH
failure is not excused, and no real-trace sample or timing decision is rerun.

External driver-control manifests: corrected
`20d72a1d764e12924447e7de3b9c9d39df4c67e05465a56aa16afc2119fd12ef`,
archived negative control
`d6cf9f75774e5ce51b2233af2259d57279ce598b92b89045979f045205eba797`.

### Numeric-time-only experiment: partial native evidence, pair blocked

A separate opt-in research policy now permits differences only in numeric
`:time` value tokens in complete top-level flat numeric statistics replies.
The default and original raw gate fields remain byte-exact. The experiment
retains original inputs, every stdout/stderr byte, both hashes and all time
values/offsets; models, errors, verdicts, other statistics, framing and spacing
remain exact. The separate experimental pool binding cannot collide with the
default prototype key. This is not query/cache normalization, a selected mode,
a default correctness claim or a shipped optimization.

Local controls pass: 31 structural/streaming comparison cases, nine actual
owned-child driver/core experiment cases, all eight original driver cases,
twelve whole real-trace parent-entry controls and two actual Entry readiness
barriers. The initial experimental checkpoint incorrectly latched readiness
when extra incomplete bytes arrived before claim; the owned-child barrier
fails that checkpoint and passes after readiness is invalidated on every
observation until the exact complete prefix matches again.
Native image/RSS checks are doubled in the driver fixtures. The cases reject
other-statistic, verdict, model and stderr changes, equal nonzero exits,
`ESRCH`, final cleanup failure and timing differences without opt-in. An
offline comparison of the preserved three-fresh-run bytes finds only permitted
time-token differences for both selected projects. The original IronKV
`ESRCH` and both original strict raw failures remain failures; this offline
comparison is not a rerun or native qualification. The subsequent native
experiment below is separate evidence; it does not replace either failure.

The first timing-only packet also omitted the fresh child process exit/error
from its parent eligibility predicate. A true-looking child manifest followed
by actual exit7 reproduced a false parent success in a controlled-child test.
The corrected parent requires fresh exit0 and no retained process error before
any live attempt. A second actual-child control retains exit0 plus a timeout
observation and refuses it too. Native aggregate admission independently
requires the recorded fresh process outcome. The original packet is rejected,
and no native run used it.


The native attempt used source `133b4721`, the fixed IronKV/Anvil trace pairs,
and pinned Z3 4.16.0 (`e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765`)
on one quiet `c7i.4xlarge` in `us-east-1`. The immutable payload SHA-256 is
`3bced99d81c12aa1f23f9cab2f2ce87d398d0de25fb58e627a694c6d621b0caf`.
All 28 native process/image/resource controls and the 31/8/9/12/2 parser,
original-driver, experimental-driver, parent and readiness controls pass.
Source, executable and runtime checks pass before and after execution.

| Dataset | Actual result | Cost and limits |
|---|---|---|
| IronKV | Three fresh runs and one live continuation pass the named experimental policy; each returns 24 `unsat`, zero `sat`/`unknown` | One sample: 2.132 s preparation, 0.322 s foreground versus 2.196 s guarded fresh reference (6.83× foreground ratio); 2.171 s total solver CPU versus 2.167 s fresh CPU |
| Anvil | Three fresh runs each return 9 `unsat`; non-time stdout bytes, stderr and exit status agree, but fresh run 0 records RSS-observer `ESRCH` on `smaps_rollup` | All three fresh runs remain retained, at 54.203/53.700/54.105 s; the error refuses eligibility and no live sample runs |

IronKV's prepared and serving PID is identical, with native image guards,
no fallback and no refusal. Its foreground reduction spends real preparation
work and demonstrates no CPU saving, application speedup or achievable hit
rate. Original exact-byte gate fields remain false when clock bytes differ.
The selected-pair aggregate exits 1, with `qualified=false` and
`selected=false`; Anvil's observer error remains a failure despite its exit 0
and matching non-time responses. No selection, integration or shipping follows.

External evidence in the timing-only native experiment set:

| Artifact | SHA-256 |
|---|---|
| Native controls archive | `c8b8e768a6a756a5d235450b6eab261cb94eb9f21121fe319855424bb2654902` |
| IronKV archive | `06fd98dad93fb47f1b514d6d5ca2cf95f890f2e1db4e3da0db8f5e82d0cbf70e` |
| Anvil archive, refused | `243dbc83efb5401d5efe10c9c9a39a22cfa0dac54c171a4d9063e948cbee2606` |
| Final archive, aggregate failed | `59fff5bb03ae54fda0a4b0b60aeb5e8ff1767a837d132f6c66b199137dff6938` |
| Actual outcomes and cost summary | `7a00d113e3eb8073d2d75c1f47d4181853a2c9417504333843bbdea16e28ccf4` |

Failed attempts remain indexed: `6001346c`, 2026-10-16, local owned-child
readiness control fails on additional partial output; corrected by `ebbf09a5`.
The `c3c99584` packet is rejected before native execution after an actual-child
exit7 fixture exposes parent admission that trusted its manifest; corrected by
`133b4721`. The native `133b4721` pair fails on the Anvil observer error above;
no replacement fresh sample or Anvil continuation is substituted.
