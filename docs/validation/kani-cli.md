# Fresh bounded Kani CLI candidate

Status: built, unselected; native Linux frontend/default, strict original-driver,
package verification and extension gates pass. Complete originals are lead-verified
and externally retained; independent final native review approves the bounded scope. No Mac,
performance, cache/currentness/proof authority or release acceptance follows.

CLI executable source `cd10f235577d44f0cb2dc1bbae3e8516ec67b89f` implements
explicit experimental setup/status and fresh bounded checks. Successor
`070d2d215d39a3ce21166b1c7048555e54487d8a` changes only the independently reviewed
Cargo.lock checker binding and ADR text; all Rust, manifests, lock and embedded
resource bytes remain identical. The later `1cbd7cae` successor changes the optional Kani harness-name parser
only; its own full gate and root-harness checks pass. Documentation amendments
change no executable. Native environment is x86-64 EC2 c7i.4xlarge, us-east-2, Ubuntu
kernel `7.0.0-1014-aws`, Rust 1.99.0, native-built CI image
`6c19f5c8d397b61c-core`, original Kani 0.68.0 / CBMC 6.11.0 and the pinned
original nightly payload. No emulated measurements are used.

| Actual gate | Result and boundary |
|---|---|
| Candidate build | All-target CLI Clippy, parser libtest and release compilation pass |
| Three original arrayvec harnesses | Bounded success, counterexample and insufficient bound have expected CLI exits 0/1/1; source/harness/lock bytes unchanged |
| Five CLI refusals | Missing bound, mixed/background mode, retired requirement, unknown backend and unsupported setup selector refuse with exit 2 |
| Embedded consumer setup | Actual original network installation, authentication and status pass; corrupt original binary refuses status/install and stays intact; restoration rechecks |
| Frontend cancellation | One-millisecond budget refuses without an observation; SIGINT after an observed started original driver refuses and reaps that driver |
| Full workspace successor | 1,453 tests pass / 59 declared skips; complete fmt/Clippy/doctests/rustdoc/layering/deny/generated-page gate passes; all 35 model and 316 refinement positive/broken cases pass at `070d2d21`; their bodies and mappings remain unchanged by the optional Kani parser successor |
| Current root-name successor | `1cbd7cae` full gate passes: 1,453 tests / 59 skips; original driver accepts a root harness, old CLI refuses, new CLI accepts; module suffix still refuses |
| Default Verus regression scope | At `070d2d21`: 55 solver tests, full E2E including extra and all four PR arms pass (33 Same / 311 functions each); no default-path source change in `1cbd7cae` |
| Strict original-driver comparisons | All nine final pairs pass at `1cbd7cae`: six successes, two counterexamples, one insufficient bound |
| Package verification | All 16 public package archives pass actual Cargo package verification at `070d2d21` and again at `1cbd7cae`; no publication |
| Temporal extension | All 22 tests pass at the extension-lock successor and again at `1cbd7cae` |
| Independent source/data review | Bounded approval after 33 embedded-helper, 25 transport and 18 mocked publication controls; 35 exact resource copies and 16-package manifest closure checked |
| Independent lock-only review | Exactly one internal CLI edge; all 256 package records otherwise match, 25-package parser closure, seven parser bindings and 61 checked-context files unchanged |

Original source review report:
`ae63aedf260f5bdb975e866fbd4f146a1ed7b0db5efcd446487f2c23b436d269`.
Lock-only review report:
`f44778d8a584c920607d91b26f505f2fec74a8a9a05d1952e49cef9e6533a8df`.
These reviews do not substitute for native frontend or package acceptance.

## Repetition on current main (2026-11-27)

The nine commits are replayed on `main` at `89820bc9`. Workstation: AMD Ryzen AI Max+ 395,
32 threads, Fedora 43 kernel `7.1.5-100.fc43.x86_64`, native x86-64, Rust
1.99.0. This is a developer host, not the EC2 lane, and its timings are not
performance evidence.

| Actual gate | Result and boundary |
|---|---|
| Host gate | `cargo fmt --check`, workspace all-target Clippy, layering (19 crates), book test and `crate-resources.py check` (35 copies) pass; 1,525 tests pass / 59 declared skips |
| Tool authentication | A link-free copy of an existing original Kani 0.68.0 bundle and the rustup `nightly-2026-08-21` tree authenticate and stage; the upstream install layout itself refuses on its `toolchain` symlink, as designed |
| Nine arrayvec pairs | Release `sail verify --backend kani` against the same staged driver, argv and environment replayed directly into another fresh target: six bounded successes, two counterexamples, one insufficient bound; CLI exits 0/1/1 and driver exits equal in every pair; sources and lock unchanged |
| Pair differences | Export: `metadata.timestamp`, two `duration_ms` fields and six `cbmc_stats.runtime_*` fields only. Streams: equal line counts; 134 differing lines, all Cargo or CBMC timing lines |

The comparator here is a local script, weaker than the reviewed external
runner above: it does not check repeated JSON fields or observation-to-check
bindings. The strict runner on the native Linux lane and default Verus
conformance (`ci/local.sh conformance fast pr`) on this source remain owed.
Setup by network download (`sail engine install --backend kani`) was not
repeated. Raw pair log sha256:
`3cf3a34a9b40569fd908f8545e2a9c7e4265d15d6fcc15e4c39ae64086eaf8e3`.

## Paired original-driver scope

Nine ordinary non-contract harnesses cover all six positive, two property-failure
and one insufficient-bound cases in the pinned arrayvec pilot. Each CLI run uses
a fresh target; its paired direct original driver runs without Sail's execution
runner against another fresh target. Original raw streams/exports stay intact.
The original whole-export assertion fails on named wall times/timestamp and
request-bound target paths; it remains failed. ADR 0156 records the exact bounded
comparison policy, which grants no production-output normalization exception.

The first tree-only paired oracle is independently rejected: it collapses
repeated JSON fields, admits lexical parent-path escapes and omits raw diagnostic
comparison. Its nine historical pairs do **not** qualify the frontend. Successor
review checks strict JSON, canonical requested-target paths, exact other native
export fields, complete stdout/stderr line order/count/endings and only named
timing observations. It also checks exact public result/backend/harness/bounds,
false typed authority flags, new unique captures, original request/tool identity
and independent observation-to-native-check bindings. Corrected source/oracle review
`b299ce193de7380e50d181eecb6b01cc702d57930e939bc1bf41b206dc2a834b`
approves the exact external runner after all 71 positive/negative controls.
Its earlier revisions and all 2,034 current review files are independently
lead-rehashed and externally retained. Final native repetition passes at `1cbd7cae`; independent native review approves the bounded component.

## Preserved unsuccessful attempts

| Attempt, 2026-11-15 | Actual result |
|---|---|
| Initial CLI full gate | Correctly refuses unknown dependency-lock fingerprint before workspace tests; exact single-edge review precedes the one-hash binding update |
| Direct pair 2 | Whole-export equality fails on eleven timing/timestamp/target-path observations; original bytes retained |
| Tree-only direct pairs 3 | Nine pairs match their original narrow comparison, but independent oracle review rejects validation gaps; never promoted to frontend acceptance |

Prepared Cargo-home cache bookkeeping can change during use. Project sources
remain unchanged; no immutable whole-input/source-closure claim is made. The
process budget covers the child, separately from tool/filesystem authentication
and generated target disk use. Original inherited-group controls remain qualified
at the unchanged runner component; escaped process groups are outside its scope.
The temporal workspace lock initially lacked the new CLI edge. The candidate
adds only the existing-version sail-kani record and CLI dependency; private
D.I.V.E. identity and all other records remain unchanged. The native extension
fmt/Clippy/test gate passes all 22 tests, including the final parser successor. This fixes an unmerged integration gap, not a main bug.

Default Verus solver/E2E/PR conformance and actual package verification pass
in the scopes above. Mac and final release gates remain separate. Documentation includes CLI help, Kani guide,
consumer skill, changelog, roadmap, dependency ADR and this record.

The final frontend review also removes an unnecessary `::` shape restriction:
root-level native harness names must be admissible through the same original
`--exact` selector. This successor changes the optional Kani parser only;
its own build/full gate and native root-harness comparison pass. Earlier
module-harness evidence stays attributed to cd10/070 and does not qualify the
new parser automatically. Default Verus paths and all model/refinement bodies
and checkers remain unchanged.

## Complete native originals

The lead independently streams every indexed file and metadata entry and checks
external read-back: 21,858 files, 5,398 directories, three symlinks and 124
hardlinks. Archive SHA-256:
`61f8001a9c7c21b5f3c71367c9164f480ddf8b74d5b7b5c7a894195c42c8d078`;
index: `fdf0d62d37ce83b2f0d2fafa1be07c26fd4b921ae6a1fa18235b083da2d2e1da`.
The packet retains original streams/exports, rejected attempts, all model and
refinement reports, exact source/Git, verified package archives and executed
release binaries. Original tool archives bind the separate component packets.
Private extension sources are owner-only evidence, never a public release payload.
Regenerable build intermediates, socket bodies (metadata indexed) and temporary
fixtures already deleted by their original runners are outside the packet.
No immutable whole-input/source closure follows.

Independent native addendum SHA-256
`73a029ec29df9ee9c0c4d469949fa39565b82066a0fc479dd978555ed3025476`
approves only this bounded component and the scoped unchanged-default regression
evidence. It independently verifies every packet body/metadata entry, all 1,443
tracked target blobs, both original tool trees/Stage, all nine full export/stream
pairs and 16 package source/resource closures. It recomputes 97 export and 138
stream observation differences under the exact reviewed comparison policy.
All 2,743 addendum files are separately lead-verified and externally retained
(index `b32f253318816a466a0f0c6acd85c89727713e9c9455d252f849965f4b9182c6`).
The broad old refinement snapshot includes the changed unproved Kani CLI context;
every actual checked/native role, checker, stencil and input remains exact.
The 070 executions remain attributed to 070, with no widened proof scope.
