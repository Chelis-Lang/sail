# General performance qualification

**Current status:** broader tooling implemented; initial runtime bundle
unselected after native whole-call evaluation. The separate ADR 0092
prototype is implemented, off by default and undergoing native qualification. No new release performance acceptance or frontend reuse is claimed.

The [review](../design/general-performance.md) defines the unchanged authority
boundaries. The [evaluation guide](../performance-evaluation.md) defines the
expanded corpus, explicit runtime selection and separate diagnostic controls.
The existing [v0.2.0 matrix](v020-performance.md), observer-overhead failures and
strict stock source-suite failure retain their original scope and requirements.

| Gate | Current evidence | Limit |
|---|---|---|
| Broader selection | 18 reviewable workloads; four new repository pins; Verus-TLA baseline passes (141 functions compared); hekate-math passes all 18 units (1,848 records); semi-persistent passes all three selected packages (2,430 / 909 / 29 records) | Baselines compare stock with bare stock/shim; selected-fork, managed and targeted-edit coverage remain separate |
| Run-scoped observations | `32f7bba6` bundle evaluated in complete native real-project and generated whole-call controls | Unselected; no material gain, with small warm-call regressions. Runtime paths retain `715c0f5e` behavior |
| Linux functional controls | `32f7bba6` passes the complete ordinary gate, including 1,284 tests (56 skipped) and eight evaluation controls; composed functional checks below pass | Separate ordinary-gate correction; no overall pass claimed for the earlier controller that continued after Clippy failure |
| Algorithmic scaling | 2 generated units / 4,314 functions compare same; 84 complete whole-call samples match stock bytes, with Current/no-verifier warm checks | 8/64-package full-call medians remain flat/slower. The earlier 54× lookup component result does not justify selection |
| Per-file artifact observations | Owner-approved ADR 0092 prototype implemented behind exact opt-in; every platform defaults off | `b980b77c` ordinary gate and eight controls pass; active integration and independent on/off measurements continue. The narrower composition requires requalification; no first-after-cold parity implied |
| macOS | No result for this candidate | Native host gate, paired performance and release memory gates remain |
| Compiler reuse | No persistent frontend reuse implemented or selected | ADR 0052 complete-input/output shadow qualification remains open |

Raw records stay outside git. Add their source/runtime/environment identities,
scope and SHA-256 here after completed captures; do not describe a build or
matching failed baseline as a passing real-project workload.

## Whole-call selection

Source `32f7bba6`, previous Sail `715c0f5e`, fork `47fb3be6`, stock
`7d829946`, native c7i.8xlarge / us-east-1. Six rotated blocks retain every
stock/current/previous sample on pmemlog, Verus-TLA and held-out containers:
288/288 rows, no refusals, unchanged source and executable identities. Values
below are Sail medians in seconds; first and full-path rows are distinct.

| Workload | Previous first | Candidate first | Previous full path | Candidate full path |
|---|---:|---:|---:|---:|
| pmemlog | 0.18195 | 0.19471 | 0.13351 | 0.15038 |
| Verus-TLA | 0.17861 | 0.19039 | 0.13757 | 0.14869 |
| Containers | 0.92503 | 0.90681 | 0.55707 | 0.54127 |
| Anvil, separate control | 1.82556 | 1.91660 | 1.09161 | 1.08376 |

Anvil contributes 28/28 successful rows: two fresh contexts per version,
with ten repeated full-path calls per version, not ten independent cold
rounds. It has no stock timing-acceptance claim. Cold medians in the three
paired projects remain broadly flat; their paired stock controls are retained.
Stock full-path medians are 0.10078 / 0.09892 / 0.20572 respectively. The
bundle does not remove the first/full-path parity blocker and is unselected.

The separate serial-Cargo generated control uses source
`6f9241658d4fee01155a4919d480087d47302f91`: chain 8/64 packages, four
modules/package, one root binary and every package a default member. Its
strict stock/fork comparison reports 2 same / 4,314 functions, zero
different/unstable/infrastructure/skipped results, with resource drift in two
units. All 84 timed rows match stock stdout/stderr/exit exactly. Every warm
Sail observation is Current with zero verifier starts and complete requested
targets. Two fresh contexts/mode supply ten repeated full calls per size;
these are not ten independent cold rounds.

| Packages | Stock full path | Previous Sail | Candidate Sail |
|---|---:|---:|---:|
| 8 | 0.11203 | 0.17735 | 0.18255 |
| 64 | 0.17443 | 0.32924 | 0.32855 |

This complete whole-call control does not close the separate parallel
workspace/all-targets framing/identity failure. Archive SHA-256:
`72f783b6adb904ddd7f2a4528f16d30691de7d5e3a78e58e5deba7b99942e421`.
It retains reports, every sample, raw outputs and Current producer records.

## Linux functional scope

On native c7i.4xlarge / us-east-1, source `ab5fcddb` passes 53 ignored tests
(51 solver/fault controls plus two component diagnostics),
the complete E2E command including the local extra cases, and all four fast
and standard conformance arms. Each fast arm reports 147 same / 3,451
functions; each standard arm reports 159 same / 13,166 functions. All report
zero different, unstable, infrastructure errors or skipped projects; resource
limit drift appears in 11 fast / 17 standard units. Standard mutation/revert
checks also pass. Its earlier ordinary Clippy failure remains a failure.

The corrected source `32f7bba6` changes only the two Rust comment spellings
relative to that production implementation, and passes the complete ordinary
gate on c7i.8xlarge / us-east-1. Its release stages and strict clean-source
intake control build and pass; the later generated comparison refuses as
recorded below. No complete controller success or release acceptance follows.

Conformance archive SHA-256:
`cdc12f0bf5a9146b99cf4ce223e4a371dcafe6a4865cef057e0d5549db15412f`.
Its gate log SHA-256:
`0e9b0212600f1ffa46bdedfbcb00367330c3c28d9f8007fd22c77d74c253de03`.
The corrected ordinary-gate/build/control log SHA-256:
`327debb625c64a8036d4bfc7c69fad35c1b6f44cb61f0a17a1d84234133cd153`.

## Compatibility findings

The initial successful baselines use stock `0.2026.10.18.7d82994`, its pinned
Z3 and Rust 1.99.1 on c7i.8xlarge / us-east-1. Verus-TLA uses scratch source
dependency adjustment; hekate-math runs its original 18 standalone proof roots.
Both arms of every passing unit exit zero with zero verification errors;
hekate-math reports 3,050 verified occurrences per arm across its roots. These
are compatibility/comparator controls, not timing acceptance.

Initial intake/component archive SHA-256:
`1b28f575e3317810594778665e628f017ab8267ad0a6fcd94eb426ff7aeecc67`.
It retains both intake attempts, the original comparator reports, selected
binary digests and all component samples, including slower JSON samples.
Source is the recorded working tree based on `715c0f5e`, before the complete
workspace preparation fix; it is not a clean-head release acceptance record.

Semi-persistent exposes a harness preparation defect: patching only the
selected package leaves conflicting exact vstd pins in other Cargo workspace
members. The candidate now discovers the complete workspace with Cargo's
offline, dependency-free metadata and adjusts each scratch member consistently
on both sides. The clean `b9466ece` rerun passes all three selected packages: containers,
abstract domains and AU, with 2,430 / 909 / 29 function records compared.
Fetched sources and proof bodies are unchanged. Selected-fork held-out
timing and proof edits remain separate gates.

The TLSF pin does not compile with the pinned vstd: `Set::new` returns an
`Option<Set<_>>`, and `Map::new` expects a set rather than its predicate argument.
Both arms report the same two E0308 errors and compare zero functions. This
is an incompatible candidate, not a successful allocator baseline. No proof
port or runtime substitution is included.

## Evaluation controls

The subsequent seven-control harness passes natively on c7i.4xlarge /
us-east-1. It additionally refuses changed or missing measurement inputs.
Two actual syscall-observer executions preserve an unset child `LC_ALL` and
an explicitly supplied `C.UTF-8` value. Their native commands exit zero and
produce complete syscall records. Archive SHA-256:
`9d3feb9245421e9cdd6e0db9fbef44ebe50cc3c54078737e275e41f83e6842f9`.
This is harness correctness, not a product speedup. The compiler operation
[audit](../design/frontend-lowering-boundary.md) is static source review;
it supplies no complete-key or restoration acceptance.

## Failed attempts

- 2026-10-31, `b980b77c`, active ADR 0092 E2E: the compiler-environment
  witness's clean-caller block leaves the new opt-in setting present. Stock
  and Sail have identical streams/exit 101 with 1 verified / 2 errors, but
  the unchanged clean-caller assertion requires 0 verified / 3 errors. Extend
  that block's existing unset list and its separate ambient-caller case to
  the new switch; retain both strict tallies and byte comparisons. The failure
  collector reaches its existing 1,024-file limit; relevant original streams
  are captured but the archive is explicitly incomplete. No limit is raised.

- 2026-10-31, fixed generated source `7e7ec297`, candidate `32f7bba6`:
  serial Cargo retains all targets and produces complete JSON streams, but
  independent crate order differs. Chain compares same; diamond/shared
  compare different in function details (1 same / 2 different). Equal
  aggregate verified totals do not close per-function or parallel acceptance.

- 2026-10-31, fixed generated source `7e7ec297`: all three parallel
  workspace/all-targets comparisons still refuse. Every arm exits zero, but
  repeated stock output fails JSON parsing after 1–7 complete documents;
  function/verified totals are incomplete. Preserve 3 different / 0 same and
  the additional plain controls. A serial-Cargo control is separate; see
  [VI-0039](../verus-issues/VI-0039-concurrent-cargo-json-framing.md), #632.

- 2026-10-31, `ab5fcddb` / `0ea95898`: composed ordinary gate stops at
  Clippy on two unquoted Rust identifiers in the closing-barrier comments.
  Its controller continues the independent solver/E2E/conformance checks;
  those results do not establish an ordinary-gate pass. Quote the identifiers
  and rerun the complete ordinary gate.

- 2026-10-31, `b9466ece` held-out capture: Git exited zero while warning
  that a build-output directory was unreadable. The initial runner treated
  this as a clean checkout; full clean-source inventory was not established.
  The interrupted capture remains limited evidence. The runner now refuses
  Git warnings, with an actual native warning-refusal and clean-source control;
  composed-candidate timings require a new sealed capture.

- 2026-10-31, `b9466ece` generated source `0f63559f`: all three shapes
  refuse the full workspace/all-targets comparison. Generated binary targets
  lack the required Verus prelude; parallel Cargo failures produce different
  trailers and partially completed crate output (3 different, 0 same). The
  generator now imports the prelude and checks a root-library call. Original
  comparator checks remain unchanged; the valid rerun exposes the separate
  framing/identity failures above (#631, #632).

- 2026-10-31, `b9466ece` benchmark with first external loop catalogue: the four
  arithmetic edit expectations and first five temporal-logic expectations
  pass; the final refinement matches stock's compiler failure and refuses
  its expected-positive outcome. Stock's minimal reproduction
  reports E0308 (`nat` addition assigned to `int`), zero verified/functions
  and zero verification errors. The matching nat local passes a native minimal
  control. The corrected four-edit/six-state catalogue passes six complete
  rounds: 84 same comparisons, every expected stock outcome matched, zero
  different/unstable/infrastructure errors. This qualifies exact-query-cache
  edit behavior, not a managed-receipt speedup. Do not
  count the matching compiler failure as passing edit qualification. See
  [VI-0038](../verus-issues/VI-0038-literal-addition-infers-nat.md).

- 2026-10-31, `b9466ece` diagnostic controller: perf discovery did not follow
  the installed kernel-tools directory symlink; the retry then lacked an
  explicit shipped vstd lock beside its wrapper. Both refused before a timed
  invocation. Corrected the diagnostic controller paths.
- 2026-10-31, `b9466ece` first syscall diagnostic: setting the tracer's locale
  also changed the child's `LC_ALL`, causing receipt invalidation and verifier
  work. Interrupted that capture and retained it as an invalid unchanged-call
  diagnostic. The helper now restores the original child's locale; the next
  profile sets one consistent locale for cold and all subsequent calls.

- 2026-10-31, `8f298701`: workspace preparation composition failed to compile
  because registry-adjustment logging lost its repository-root parameter
  during extraction. Restored that explicit parameter before native rerun.

- 2026-10-31, working tree based on `715c0f5e`: first native candidate gate
  failed Clippy's needless-borrow check in the benchmark CLI. Corrected in the
  subsequent native subset gate; no check or baseline changed.
- 2026-10-31, working tree based on `715c0f5e`: first intake refused all six
  selections because independently installed tools looked for a sibling shim.
  The runner now passes explicit executable paths; the rerun retains all units.
- 2026-10-31, working tree based on `715c0f5e`: semi-persistent's three package
  attempts refused Cargo resolution due to incomplete scratch workspace pin
  adjustment. TLSF reports matching compiler errors. Both remain explicit.
