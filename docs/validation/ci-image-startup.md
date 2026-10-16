# CI image startup

| Field | Value |
|---|---|
| Status | Packaging and correctness accepted after native parity and two complete actual CI runs; published core is 9.024% smaller. **No latency gain established.** #195 stays open. |
| Tracking | [#195](https://github.com/Chelis-Lang/sail/issues/195), [ADR 0013 candidate](../decisions/0013-ci-image.md#candidate-2026-10-16-share-the-core-image-without-the-optional-mcp-client) |
| Scope | One Dockerfile, a shared `core` target and its complete `mcp` extension. Every existing toolchain, verifier, solver and runtime digest check remains. Only the existing pinned MCP client layer is excluded from jobs that do not invoke it. |
| Current checks | Native Linux gate, solver, all seven e2e phases plus extra, and PR conformance pass. Mac host checks pass on the earlier prototype source, as scoped below. No startup performance claim follows from these checks. |

## Unqualified follow-up: partition the stock installation

The next source candidate puts the complete stock installation into four
deterministic layers rather than one large layer. It retains both toolchains,
the existing archive and solver checks, the fork engine, and the real MCP
client. `ci/partition-verus.py` keeps each internal hardlink group together,
preserves independent files even when their bytes match, and copies symlinks
without following their targets. Every part retains the directory inventory.
The final image build compares every installed path's type, file bytes, mode,
owner, group, symlink target and internal hardlink topology with the validated
installation's inventory. The partition helper is an image-tag input.

Filesystem and independent tar-layer replay controls pass on macOS, including
empty directories, executable/read-only modes, internal hardlinks, equal but
independent files, and dangling/external symlinks. Invalid destinations,
aliased roots and special files are refused. These controls do not qualify
Docker COPY behavior, actual Linux runtime identity or startup performance.
The earlier accepted records below do not qualify this new candidate.

Remaining gates are an actual native image build and complete runtime/toolchain
inventory comparison with the current image, native workspace/solver/all e2e
phases plus extra/PR conformance, and multiple complete actual CI pull cohorts.
Record each attempt's published manifest and every slow or failed sample;
compare initialization, pull and check durations separately. More layers may
add transfer/application overhead. Neither smaller compressed payload nor a
latency improvement is established, and #195 remains open.

```sh
python3 ci/test-partition-verus.py
python3 ci/partition-verus.py inventory /opt/verus
```

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
`4846cfe1`, built both images on a native c7i.8xlarge in us-east-1 (availability zone us-east-1c). The 507 tracked source-file
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
rebased onto main `35c089b4` passes the native workspace gate again:
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
`555b8467` after #199, passes the native workspace gate again: 638 tests,
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

The lead accepts the packaging and correctness result. No latency performance
gain is claimed, and #195 remains unresolved. This final documentation update
does not change the qualified code, image inputs or measurements; normal
required CI still applies to its final PR head.
