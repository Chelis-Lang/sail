# Compiler-reuse integration

Status: source `83128ad0` passes the complete native default qualification and
final guards. The composed delivery preserves its executed code; current PR
checks and merge remain pending. The native 72-row cohort and same-host Linux portable
component pass. These results qualify their stated scopes separately; they do
not establish release-bundled first-use vstd reuse or combined proof authority.
See [retained restoration](instant-revert.md) and
[vstd boundary reuse](vstd-compile-boundary.md).

## Source and runtime

The executed source combines the retained implementation at `aeea0587`, the
portable-input fixes at `6573b195`, and the dependency-profile prefilter at
`c5fc9697`. The six changed implementation files match those components; the
runner adds the configured shim argument without combining their policies.
The composed delivery preserves every executed Rust, Cargo and engine file
byte-for-byte, alongside main's independently qualified performance scripts.

Both native cohorts use the normal `e5365c05` fork runtime: Linux archive
SHA-256 `689004c074aca8883e225a2b89efebc137f3e563f24df2abd398516112b13928`,
manifest `a845cd2fcba411148566d8214c1285f083689df6a8ffa5479afcf3845382b033`.
Genuine stock commit `426d8b01` is the oracle. Neither cohort rebuilds Verus.

## Separate authority

| Request | Candidate behavior |
|---|---|
| Neither research switch set | Automatic retained eligibility on x86-64 Linux, the exact admitted runtime and the public alloc/std dependency profile; unsupported contexts compile freshly |
| `SAIL_VSTD_BOUNDARY_RESEARCH=1` | Explicit compiler-boundary experiment; automatic retained selection is suppressed |
| `SAIL_RETAINED_CONTENT_RESEARCH=1` | Explicit retained-content experiment, including profiles outside ordinary eligibility |
| Both switches exactly `1` | `SAIL3135` before runner setup or adapter child execution |

Only the literal value `1` requests an experiment. The ordinary prefilter grants
no proof authority: the complete source, feature, runtime, native-input,
private-graph and Current checks still apply. The default-feature small-project
control remains fresh. Eligibility follows the dependency profile rather than
a project-name or estimated-cost test; another small library can share the
eligible profile. macOS ordinary retained activation remains disabled.

The four literal source policies remain distinct:

| Policy | Scope |
|---|---|
| `engine/verus/vstd-boundary-inputs.json` | Published vstd boundary source/features; explicit experiment only |
| `engine/verus/retained-public-vest-inputs.json` | Public alloc/std profile eligible for ordinary retained selection |
| `engine/verus/retained-public-inputs.json` | Public default-feature profile; explicit retained research only |
| `engine/verus/retained-vest-inputs.json` | Fork path-provider profile; explicit retained research only |

The public profiles bind all 15 archive identities, 662 file/checksum entries
and exact feature/dependency vectors. The path-provider view has 718 files;
its 400 official provider files match `e5365c05`. The boundary view instead
contains published package bytes: 307 corresponding official files match that
Git tree, 15 differ, and 22 package-only paths have no Git-tree counterpart.
Those views cannot lend one another source authority. The shared dependency
snapshot implementation does not combine certificates or caches.

## Current validation

Both Linux hosts are native EC2 in `us-east-1`: the workspace/conformance cohort
uses `c7i.8xlarge`; the targeted cohort uses `c7i.4xlarge`.

| Gate at exact source `83128ad0` | Result |
|---|---|
| macOS host | Pass: clippy and 742 tests, with solver tests excluded by the host profile; two test threads, unchanged assertions/deadlines |
| Native workspace gate / solver | Pass: 753 tests plus two doctests / 35 solver tests |
| Native end-to-end | Pass: all phases and local-only extra cases |
| PR conformance and all four FAST arms | Pass; each full FAST arm 147 Same, 3,450 functions, zero Different, Unstable, infrastructure errors or skips |
| STANDARD and final guards | Pass: all four arms each 159 Same, 12,726 functions, zero Different, Unstable, infrastructure errors or skips; exact source maps, all six binaries, stock/runtime files and MCP checks hold |
| Fresh targeted cohort | Pass: all 72 rows admitted; complete final growth inventory and final source/runtime/binary guards retained |
| Genuine bundled ordinary entry | Pass: separate Vest A → B → A → unchanged functional control, without runtime or research overrides; original A origin and zero verifiers on return/unchanged |
| Portable vstd component | Pass on the same Linux host with changed physical Cargo, project and install roots: schema 12, 1,872 function entries matching stock, Current, six comparisons and original shipped origin |
| Release/default first-use vstd | Pending; the portable component does not establish a distributed bundle, another machine, or changed Rustup-root portability |

The timed Sail actors in the 72-row cohort use the same explicit matching
runtime override. The genuine bundled control is a separate functional result,
not a relabeling of those timings. Three rotated rounds measure the complete
four-command Vest cycle at 130.66 s for the earlier Sail build and 113.08 s for
the candidate (13.45% less); stock takes 148.10 s. The returned A median falls
from 26.54 s to 2.51 s. Cold, changed-B and unchanged costs rise, and final
cache size rises from about 560 MB to 1,080 MB. The small control's four-call
median is essentially unchanged against earlier Sail and remains slower than
stock. [The retained validation record](instant-revert.md) carries the detailed
performance and storage limits.

The targeted final archive is SHA-256
`3f42ea4f70bbeee125e1bf4b67b7cacd3e5e26612a13fe401c5a9bd846287af2`.
The native gate, solver and end-to-end archives are respectively
`d693b227d2c570ebdd5cbaf09b49668b3c73d082863c1fb51948461bd29879e9`,
`981880742000974e7cd57721a0bd22ff0a0a66297b0e263d6cf6b13732f4e7e0`
and `67b111b6114102c8a48085ba74a88b31dd28a7d767d7e73fdd3b9965ba1248fa`.
The four-arm FAST aggregate is
`ad031a6332bf5a54eaddcd7cc418e930ccb2ca8c8a60bd7ff0cc4073d354fc83`.
The four-arm STANDARD aggregate is
`57e6af6fa54d551ab06c171934547182a25f9aa1db9aeca7f456cc85f8d1470a`;
the final source/runtime/body-guard archive is
`bd3a2f085d1da5eb61147da468150f6ed0ca1243e395036cad08939ffb31cf5c`.
The earlier `c3e64d21` default cohort also passes its own complete gates and
four FAST/STANDARD arms; its final archive is
`1746416ad8cf31f77ae982a41b47dbc292ede5e170296c581478daae873e8920`.
That earlier source is a separate qualification, not evidence retagged as `83128ad0`.
FAST and STANDARD retain 12 and 18 units with rlimit-statistic drift respectively;
no response bytes or conformance predicates were relaxed.

| Earlier attempt | Result and limit |
|---|---|
| Initial integration draft, 2026-10-17 | Clippy refused three redundant closures before tests; corrected before the passing 731-test component host run. Stderr SHA-256 `68a39f00d58ec0b7bd1fd7e93a889e8451c11168863eb19fa2e5333710e5ed01`. |
| Earlier automatic native cohort | Whole cohort failed; retrospective semantic comparisons do not repair its missing aggregate inventory or qualify the narrowed successor. See the retained validation record. |
| Initial default-thread final Mac runs, 2026-10-18 | Two full host attempts failed; cause unknown. The later exact-source two-thread host pass keeps the original assertions and deadlines. See the component records. |
| Earlier changed-home Mac portable probe | Functional component passed, whole probe failed its compiler-diagnostic capture bound. Its failure remains a failure; the later Linux cohort is a separate run. |
