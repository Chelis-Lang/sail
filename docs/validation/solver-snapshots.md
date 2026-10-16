# Solver snapshot prototype

Status: local hardening checks pass; native Linux and raw-output acceptance
pending. Disabled by default. No release uses this candidate.

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
