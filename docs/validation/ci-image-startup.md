# CI image startup

| Field | Value |
|---|---|
| Status | Packaging and correctness accepted. The retrospectively observed November 10–15 startup target is met: current-image pull command-window p95 40.2s, maximum 60.9s. No causal latency gain or future latency guarantee is established (ADR 0171). |
| Tracking | [#195](https://github.com/Chelis-Lang/sail/issues/195), [ADR 0013 candidate](../decisions/0013-ci-image.md#candidate-2026-10-16-share-the-core-image-without-the-optional-mcp-client), [ADR 0171](../decisions/0171-observed-ci-startup-target.md) |
| Scope | One Dockerfile, a shared `core` target and its complete `mcp` extension. Every existing toolchain, verifier, solver and runtime digest check remains. Only the existing pinned MCP client layer is excluded from jobs that do not invoke it. |
| Current checks | Native Linux gate, solver, all seven e2e phases plus extra, and PR conformance pass. Mac host checks pass on the earlier prototype source, as scoped below. No startup performance claim follows from these checks. |

## Observed operational target, 2026-11-16

[ADR 0171](../decisions/0171-observed-ci-startup-target.md) accepts the
#712/#195 pull p95 ≤120s target on the following bounded observations.
This is a retrospective operational window: the UTC calendar interval was
selected after a single-run observation, before collecting the complete
population below.

All 43 main/push CI runs created in
`2026-11-10T00:00:00Z ≤ created_at < 2026-11-16T00:00:00Z` are included,
with all attempts and all 937 job records. The runs comprise 42 successes
and one failure. All 570 container initialization steps completed
successfully; none is missing, failed, cancelled or incomplete.
Skipped and non-container jobs remain in the source data.
Two non-container documentation API records inconsistently report
`in_progress` alongside a successful conclusion and completion timestamp;
this does not affect the container population. The record does not assert
that all 937 API job statuses are terminal.

| Population | Samples | Nearest-rank p95 | Maximum |
|---|---:|---:|---:|
| All initialization intervals, both image variants | 570 | 50s | 79s |
| Current-image initialization intervals | 195 | 50s | 72s |
| Current core pull-command → next create-command window | 180 | 40.2s | 60.9s |
| Current full MCP pull-command → next create-command window | 15 | 37.4s | 37.4s |
| Combined current-image command window | 195 | **40.2s** | **60.9s** |

Each command window starts at the logged Docker pull command and ends at
the immediately next runner command, Docker create for the same image.
The successful status and digest occur before that create command.
Sequential runner execution bounds pull completion by this envelope,
including logging, scheduling and command setup overhead. A successful
status line alone is not a process-exit timestamp. Whole initialization
metadata has integer-second resolution. These are observed bounds, not
nanosecond-accurate process runtime measurements.

All 195 current-image logs record `Downloaded newer image`. Their tag
`6c19f5c8d397b61c` is independently reconstructed from immutable source
inputs for all 15 corresponding source revisions. Actual manifests are:

- Core: `sha256:76c4c60b8def407ef53e8299fb811949e00efb8a3cb75ee64189997253fa370b`.
- Full MCP: `sha256:2f5ee2f54232c0f4d00b26fd9e21bd65843d156ca547216720d630cea02d560b`.

The other 375 initialization samples use the earlier image variant
`f57ed8a3e26baaff`, independently bound to its 28 source revisions.
No inference about a completely empty Docker layer cache follows from a
successful pull status, and no causal packaging-size latency gain is claimed.

Three later successful workflows are reported separately:
[PR #761 predecessor](https://github.com/Chelis-Lang/sail/actions/runs/43783278569),
[its final head](https://github.com/Chelis-Lang/sail/actions/runs/43787629476),
and [merged main](https://github.com/Chelis-Lang/sail/actions/runs/43788894765).
Their 39 whole initialization intervals have p95 65s and maximum 66s.
Each workflow completed all six required checks. These are API initialization
observations, not additional raw pull measurements; they are not blended
with the original 195 or 570 samples.

The complete original raw packet contains 252 bodies; archive SHA-256:
`52979b6e15b438feb936e34c085b8788441b6dd6d8177515c26163bcb824fbc0`.
Independent original review:
`62474e3f63c5c58ae88cbeebecbddb20855fe97702385e520db020d64d9c4205`.
Independent conservative-bound and later-cohort follow-up:
`1d376c814869189abbfa93211e71b8d05aa89bed2ade93263faa775d0a7b7432`.
The lead verified every bound review artifact and actual input body and
read back the external preservation copies. Independent corruption controls
reject missing jobs, duplicate runs, failed initialization and wrong
image identities.

The historical five-minute pull, 247/255-second initialization tails and
November 4 six-minute-fifteen-second startup below remain failures outside
this window with unresolved causes. No future latency guarantee follows.
No workflow, correctness check, gate, image contents or timeout changed for
this acceptance. Mac behavior and release publication are outside its scope.

## Baseline

The existing published image is
`ghcr.io/chelis-lang/sail-ci:3c02e445e25df766`, manifest
`sha256:981949aaa1fe4e53eed6ea19d023ff8be7005374b1e42aa19ab03e27d86efbec`,
config `sha256:b7ee31237c82ad3ac551210e69764870512e852d2adc5be4b3276d407a5ed417`.
Its manifest records 1,325,466,562 compressed bytes across 12 layers.
The MCP client layer accounts for 119,609,629 bytes (9.0%). This is an
investigation lead, not a measured candidate size or proportional time saving.

| Actual GitHub job | Container initialization | Actual conformance | Total job |
|---|---:|---:|---:|
| [PR #191, run 38856336946](https://github.com/Chelis-Lang/sail/actions/runs/38856336946/job/116828574805), source `572ebaae` | 5m02s | 4m29s | 9m52s |
| [PR #194, same image digest](https://github.com/Chelis-Lang/sail/actions/runs/38857037557/job/116830772732) | 1m13s | 4m00s | 5m34s |
| [PR #194, subsequent same-digest run](https://github.com/Chelis-Lang/sail/actions/runs/38860123741/job/116840441757), source `be2837c4` | 43s | 4m42s | 5m43s |

The first job's actual `docker pull` spans 300 seconds. Its 491,732,876-byte
stock-Verus layer completes downloading much later than the 463,015,964-byte
toolchain layer, but the log does not distinguish queued transfer, active
download or registry/CDN/network causes. Removing the MCP layer is not claimed
to fix that stock-layer wait. A locally inspected same-tag image has a different
config identity; its file inventory is separate evidence and cannot establish
the published image's bytes.

Baseline raw archive SHA-256:
`0101dd17a9a9621630305835f5e80e1a8640890a20d6442e835fdefa58949c27`.
Same-digest control archive SHA-256:
`10c3ff58ad9641adb6914b1bdd4dbbc25079d8362d6956711c5daded43c4f34f`.

The subsequent run has 11 container-job samples: initialization median 45s,
range 42–69s; actual pull median 42.80s, range 40.32–68.13s. Every sample
uses the baseline image/digest. Source, job and step metadata plus full logs
are retained; summary SHA-256
`835e544ebcf8271f237d5ec8041a0ddb892f548c8d5ff711cbb43abc76fd4a82`.
The variation prevents attributing a single slow pull to image size alone.

A second contemporary baseline, [PR #196 run 38863425499](https://github.com/Chelis-Lang/sail/actions/runs/38863425499),
source `c586119b`, retains 11 samples of that same image: initialization
median 45s (42–68s), actual pull median 43.83s (41.17–66.70s). Its
predecessor run `38863173250`, source `9bce4c3b`, was cancelled before
acceptance; its four completed startup samples remain indexed separately
(median initialization 44.5s, range 42–70s). No cancelled checks are called
passing. Aggregate/source/raw-log bindings SHA-256:
`d36153100db5352ba14692bc651bd38605709f665677149ed58e380fae852e2a`.
Only digests inside the actual Docker pull block bind an image; unrelated
downloaded e2e-bin artifact digest mentions in the full logs are separate.

## Native qualification, 2026-10-16

Source `1e80012a152283ee72c107ff38792c980122fbaf`, based on main
`8bd1948e`, built both images on a native c7i.8xlarge in us-east-1 (availability zone us-east-1c). The 507 tracked source-file
hashes, excluding agent settings, match before and after every stage; affected internal
packages were cleaned and Rust/Cargo source timestamps refreshed first.
Candidate tags are `44261d52f2b2ff03-core` and `44261d52f2b2ff03`. These
are local build identities, not yet published manifest identities.

The retained local baseline image is explicitly
`sha256:03d6865e6602ae702f480b4396543c5c1c51fe07f88fd80faa210880a66abef6`;
it is not asserted equal to the published baseline config. Baseline, core
and MCP inventories match across 5,037 toolchain/verifier/runtime entries,
198 installed dpkg package versions, both compiler versions, and image
environment/entrypoint/working-directory settings. Core lacks the client;
baseline and MCP retain the identical 265,342,136-byte executable with the
existing pinned SHA-256. No package/version difference was waived.

| Check | Result | Elapsed |
|---|---|---:|
| Workspace gate | 633 passed, 35 skipped; remaining gate checks pass | 46s |
| Solver | 35 passed | 9s |
| Actual `ci/local.sh e2e` | Seven phases and local extra pass, including the real pinned MCP client | 388s |
| PR conformance | Four arms each 33/33 Same, 311 functions; no differences, instability, infrastructure errors, drift or skips | 55s |

These are check durations, not cold-start benchmarks. The local selector
ran unchanged behind an ownership wrapper that named each container; its
final cleanup found no owned containers. The raw archive also retains older
nested conformance directories from previous lanes; only the four root PR
reports belong to this candidate and authorize the table above.

Native raw archive SHA-256:
`34bdff388f77029c98cba8796fcd1cb777b433c36dea3255adc2ef532c614535`.
Qualified Sail/shim/conformance/helper binaries archive SHA-256:
`0ce8ab2ecc2521df5dc8bccfb01bb01d719c8f9113ea1b76e54786bdf55b143f`.
Verified source, scope and report summary SHA-256:
`940aeb13cb3f3bbc19603af5a35959645c9741749494715cfc391a2bd6bec8a0`.

The earlier Mac prototype `4154bb39` passed `ci/run.sh host`: strict clippy
and 622 tests, 35 skipped, with tracked source hashes unchanged. Subsequent
changes are a workflow environment-variable rename and a rebase onto the
existing main changes; this is not a claim of a second full Mac run on
`1e80012a`. Mac raw log SHA-256:
`fe082d776e1783ea1a26413028a0e71b5674bd18f72f7e60a1f9ac5ef85e985d`.

Shell/YAML checks and a 14-case fixture of the actual local image selector
also pass, including full/managed/core phases and solver private-mount
flags. That fixture is not runtime acceptance. Its result SHA-256:
`0f275535912736267ca635d0649918cbf585353990e39903e16228eff6120bbf`.

## Joined current-main gate

After #196 merged, source `7999d0b1a4df4ca257af921abf96636251da5be1`
rebased onto main `2891710b` passes the native workspace gate again:
638 tests, 35 skipped, 50.75s. All 508 tracked source hashes (agent settings
excluded) match before/after every stage; affected packages were cleaned
again. The old/core/MCP inventory parity still passes without differences,
and both candidate image tags are unchanged. This is a final source-delta
gate, not another solver/e2e/corpus run or a startup measurement.

Joined raw archive SHA-256:
`02688a75effbd2380cb73ddf7d036b9c0b5cf7b16c22c3da90b7c301ecc44ffd`.
The subsequent current-main checks are recorded below.

## Reproducing the packaging selection

```sh
ci/image-tag.sh core        # complete input hash, with -core suffix
ci/image-tag.sh mcp         # complete image; also the no-argument default
ci/local.sh gate            # core; same gate command as CI
ci/local.sh e2e managed     # mcp; includes the unchanged real client smoke
ci/local.sh e2e             # mcp; all phases plus the local extra phase
```

Direct builds without a target still produce the complete image. Both
targets retain all validated runtime paths and bytes; no runtime-file removal
or duplicate-file optimization is part of this change. Image publication must
succeed before the existing completion check can accept dependent jobs. A
missing MCP client remains a managed-phase failure.

## Published images and first actual CI run

Both target manifests are now published by the normal private CI workflow.
The first 11 layers of the MCP image equal the complete core layer list.
Only the final client layer, 119,609,622 compressed bytes, is added.

| Target | Actual manifest | Compressed layers |
|---|---|---:|
| Core | `sha256:bec4be7e84bc6e8b7a27a7e71243b0bbf26a9db1417816af819fc1b418b9b33a` | 1,205,854,041 bytes; 11 layers |
| MCP | `sha256:54c33f42143a25b377d8836e167dde4e2c20d00006098f631ac3c222c681567c` | 1,325,463,663 bytes; 12 layers |

This proves a 9.024% compressed-payload reduction for core jobs relative to
the new full image. Raw registry manifests and source-independent digest
validation summary SHA-256:
`715f4a4e039b05208f3cc6082365eedb83c5ace8badc50f5f3456dc375bf2fde`.

[Actual run 38865557667](https://github.com/Chelis-Lang/sail/actions/runs/38865557667),
source `2d2869178aa83fea7a86f9be8467a45ad153b556`, passes all 15 jobs,
including image publication and every required check. Managed uses the full
MCP digest above; the other ten container jobs use core. Image creation
takes 174s; actual runtime/job initialization is recorded separately.

| Actual samples | Core-equivalent jobs | Initialization median / mean (range) | Pull median / mean (range) |
|---|---:|---|---|
| Baseline run 38860123741 | 10 | 45.5 / 48.9s (42–69s) | 42.90 / 47.18s (40.32–68.13s) |
| Baseline run 38863425499 | 10 | 44.5 / 52.2s (42–68s) | 43.57 / 50.87s (41.17–66.38s) |
| Candidate run 38865557667 | 10 | 52 / 52.4s (40–69s) | 51.03 / 51.42s (39.48–66.11s) |

The unchanged full-client managed job initializes in 44s and pulls in
43.12s. Every sample is retained. The candidate does **not** establish a
startup improvement or fix the earlier five-minute tail: registry/network/
runner variation remains unresolved. #195 stays open. No timeout, test,
acceptance condition or slower sample is removed. Check durations remain
separate in the raw per-job/step records; they are not attributed to packaging.

Final first-run source/job/step/log summary SHA-256:
`257afd41e97f7fa1ef104c74d4e2c5fcd4b7ba910f8045d07167c8338ef1798e`.
Complete first-run/baseline comparison SHA-256:
`47061e6f695c0c4c48843819b5826fcd031bc7cb65c54d8d1b598d9c6be5de87`.
The earlier partial capture is retained separately; it does not authorize
acceptance.

## Final current-main gate

Source `31c6ad26128fd57231b54306308068d39d97e5c1`, rebased onto main
`16111e24` after #199, passes the native workspace gate again: 638 tests,
35 skipped, 50.5s. All 519 source-file hashes (agent settings excluded)
match before/after every stage. Affected packages were cleaned, all three
existing image inventories still match except the client, and no image was
rebuilt. Final native raw archive SHA-256:
`807bd890871f687648eb5b8fe48767d077430a19b7c64c21f27fa68aab112585`.

[Final current-main run 38869064211](https://github.com/Chelis-Lang/sail/actions/runs/38869064211),
source `cc83fcc21a9cb5e8373912a5271c2d3a2e85687f`, passes all 15 jobs,
including every required check and the complete client smoke. Both images
retain the published manifests above. All ten core startup samples are
retained: initialization median 67s, mean 103.4s, range 41–255s. Solver
initialization takes 247s and workspace initialization 255s; the smaller
image has not resolved the slow tail. The managed full-image job initializes
in 43s. Full source/job/step/raw-log summary SHA-256:
`5dd62c1d89db0406c1875a1d2c0cc1755bd68fd54a8389442093225a57f43820`.
Complete second-cohort comparison SHA-256:
`e7a188c05ce88dc7db2c4fccf3368de7cacaa9350f2b9a2d74af121a8619d8f8`.

At the October acceptance, the lead accepted packaging and correctness only.
No latency performance gain was claimed, and #195 remained unresolved.
That historical documentation update
does not change the qualified code, image inputs or measurements; normal
required CI still applies to its final PR head.

## Later historical startup failure, 2026-11-04

[PR #663, source `68ae0e82`](https://github.com/Chelis-Lang/sail/actions/runs/41876597188/job/126738439566)
recorded container initialization from 17:21:40 to 17:27:55 UTC (6m15s).
Its gate step was cancelled near the unchanged ten-minute job limit; no
assertion failure or registry/network/runner cause was established.
Job/step metadata SHA-256:
`8867204936417177c8d69f0846487e479449d250cebfbe4e254cc70fcfcfefb1`.
The [identical-source retry](https://github.com/Chelis-Lang/sail/actions/runs/41876597188/job/126771249703)
initialized in 50s and passed the required checks; it does not erase the
failed attempt. Retry metadata SHA-256:
`dfdd14955005242f623507b3cd1a61232d9a99a9a98ad9e08842f4396821bac6`.
Both observations are retained in [#195](https://github.com/Chelis-Lang/sail/issues/195#issuecomment-6637554559)
and remain outside the November 10–15 operational acceptance population.
