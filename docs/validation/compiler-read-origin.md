# Compiler-read source origin qualification

Status: draft PR #572 stages the paired compiler-input fix; it is not merge-ready.
The frozen Linux checkpoint `96de5097` passes solver, complete unchanged e2e,
full fast and standard conformance, both semantic counterexample controls and
useful-reuse/lifecycle controls. Its raw packet is being closed. The current
integration rebases that work onto main after #552 merged; it has not inherited
an exact-head whole-gate pass. macOS runtime construction passes, with additional
native qualification and final platform integration still pending. This record
tracks the paired Verus observation producer and Sail reuse consumers for
[#551](https://github.com/Chelis-Lang/sail/issues/551) and the distinct caller
replay gap [#552](https://github.com/Chelis-Lang/sail/issues/552). The #552
old-main reproduction returns a stale Current, confirmed verified report after
only the ghost caller input changes; fresh pinned stock rejects the same source
and changed value with a concrete bit-vector SAT counterexample. The #551
compiler-child overlay cases remain source-established main defects, without an
executed old-main wrong-verdict reproduction. ADR 0088's visibility exception
does not permit stale actual input reuse.

Schema2 requires explicit COMMON driver-origin metadata, separately from both
phases' tracked environment. Other driver declarations remain in those phase
maps. Genuine source COMMON reads stay raw and refuse reuse even when unset, empty or
equal to driver metadata. The same shared eligibility/marker-clear guard applies
to ten exact Sail compiler-child controls listed in `docs/freshness.md`.
Provenance and the compiler-input endpoint/binding have source-established main
holes alongside COMMON; the remaining seven controls are conservative policy.
Before-Cargo socket/solver/bootstrap values remain fingerprinted by Cargo.
Routine Z3/Singular declarations and inherited user inputs retain reuse.
A family-scoped write-ahead marker makes unknown/ineligible units compile
before Cargo freshness; a one-time family cleanup retires legacy origin state.
Receipt and archive versions also retire legacy reuse authority. Markers bind
the full target identity, including compiler arguments. A fresh different-unit
observation cannot clear an older marker; complete owned package cleanup may
retire those facts only after strict physical fingerprint invalidation.

The Linux descriptor now stages the real schema2 candidate from fork `47fb3be6`,
archive `8da9b487...`, manifest `d6096d5b...`. It is not final selection or a
published asset. The macOS descriptor now proposes the actually constructed
schema2 archive `52ba48d4...`, manifest `ea6853a8...`; its remaining native
qualification is running. Earlier Linux checkpoints correctly refused the old
macOS descriptor. This new proposal has not passed the final whole-head gate.
See [actual runtime construction](compiler-read-origin-runtime.md) for the
normal no-rust-src build, real pack/install and independently audited receipts.
The producer source gate passes at exact `47fb3be6`; the source tests and
runtime constructor remain distinct from paired consumer acceptance.

| Gate | Result |
| --- | --- |
| Producer source gate | PASS at `47fb3be6`: libraries23/13/144, actual four-case origin matrix, all162 unfiltered fork targets (4,839 pass,63 inherited ignored) |
| Sail unit and workspace gate | PASS on `dbda2535`: 1,170 tests, 54 inherited skips; fmt/clippy/docs/layering/deny/generated benchmark page |
| Direct stock conditional models | PASS on `dbda2535`: 4 positive models and all 31 controls; normal resources |
| Production refinements, including bound native caller | PASS on `dbda2535`: 3 positive roots, all 168 controls; query282 / inputs54 / authority184 verified, whole crates |
| Latest Linux source/Rust checks | `96de5097`: strict clippy and focused20 PASS; host 1,186/1,187 PASS with 54 inherited skips, sole failure the old macOS descriptor |
| Solver and complete native e2e | `96de5097`: solver51 PASS and unchanged complete e2e/all ten phases plus extra PASS |
| Full fast and standard conformance | `96de5097`: all four arms PASS; fast147 and standard159 units per arm, zero differences, instability, infrastructure failures or skips; standard mutation controls PASS |
| Native useful reuse and lifecycle | `96de5097`: basic64 and lifecycle10 PASS on both actual normal build variants; genuine changed-input SAT failures preserved |
| Native added cost | Coherent minimal production pair: three alternating pairs, zero refusals; tiny-fixture warm medians 310.863ms baseline / 304.480ms candidate, overlapping samples; no speedup or isolated-overhead claim |
| Native macOS runtime qualification and final whole-head gate | Construction/pack/install PASS; further Mac qualification running, final integration gate pending |
| Runtime oracle and determinism | PASS at fork47/candidate8da9: engine11, cancellation, direct320×3; both-platform selection pending |

Tests cover source-vs-driver COMMON classification, strict origin decoding,
source-only reads in either phase, legacy refusal, retained/shipped admission
and durable pre-Cargo origin state. These fixtures are not real-project
acceptance. Native useful-reuse checks must cover ordinary JSON/jobs/MCP,
retained restore and cached shipped dependencies, plus observation-mode/log-path
changes and source-read removal. No private provider/transport probes form part
of this qualification.

## Attempts

- 2026-10-28, `c78bae84`: native Linux workspace gate failed at strict clippy
  (a collapsible conditional and an oversized regression function). Later gate
  stages did not run; no runtime or proof qualification is credited.
- 2026-10-28, `f597d5ad`: native Linux workspace gate failed at the route
  regression's empty-string assertion lint. No later stage is credited; the
  assertion is retained with value-bearing failure output.
- 2026-10-28, `e02529b2`: native Linux strict clippy passed; workspace tests
  ran 1,169 cases, with 1,168 passed and one existing managed-child Runtime wire
  fixture missing the new required origin fields. All newly added origin,
  route-selection and legacy-family-clean tests passed. Remaining gate stages
  did not run; the fixture is updated without changing its checks.

## Local paired-runtime runner

[The local runner](compiler-read-origin-runner.md) uses a normal validated
installed variant and public CLI/jobs/MCP requests. Its corrected basic scope passes all 64 native cases against the real candidate,
including both SAT semantic controls; complete acceptance gates remain open.
Actual owned interruption and eligible retained A-to-B-to-A pass all ten lifecycle
controls at `96de5097` as well as the earlier `a1600f19` checkpoint. These receipts
retain their actual source and executable identities; they do not qualify the
new integration head automatically. Neither runner scope alone closes overall acceptance. Complete compiled
proof programs retain explicitly checked correspondence to `dbda2535`; new
runtime and cleanup code remains surrounding unproved code.

## Caller-only replay requirements

The #552 repair carries names from actual target-record admission into the
private no-op facts. It does not reopen unused receipt reports or broaden any
namespace rule. Current, sibling, seeded and restored receipt requirements use
the same admission results; an unknown required receipt prevents recording.
Seeded runs already cannot record replay; their adopted receipts are ordinary
requirements on later runs. Version4 rejects earlier incomplete replay records.
Actual native prepare/replay unit controls pass, and the paired shadow semantic
witness reaches a real replay hit followed by the required changed-input SAT
failure. These controls do not substitute for remaining end-to-end gates.

- 2026-10-28, `4220886e`: final Linux-only whole gate correctly refused the
  old macOS descriptor under the new shared fork pin; no whole-head gate credit.
  A controller-only attempt first refused Git ownership before checks. The
  separate scoped Rust run then stopped at one diagnostic-free empty-vector
  assertion lint in the new #552 unit test; no tests were credited for that run.
- 2026-10-28, `61eda6b8`: scoped clippy passed and 1,175 unit tests ran;
  1,172 passed, including all new #552 controls. Three runtime-discovery fixtures
  failed because the controller leaked an explicit build override into tests
  of default-cache discovery. The normal source-check environment leaves that
  override unset; no production or test assertion is changed for this failure.

- 2026-10-28, `4607ccdf`: native scoped Rust clippy passed; 1,174/1,175 tests
  passed (54 inherited skips). The remaining test correctly rejects the old
  macOS artifact's variant under the shared new fork pin. Independent release
  build and all 51 solver tests pass. The first paired runner passes 53 checks
  before stopping at its incorrect expected public exit code: the changed-input
  proof actually recompiles and is confirmed failed, with Cargo exit 101 and
  nested Verus exit 1. Its pinned-solver check is UNKNOWN with incomplete
  quantifiers, also reproduced directly with stock. A bounded direct oracle
  confirms that the same assertion using `by(bit_vector)` produces SAT failure
  when present and UNSAT success when absent in stock and the candidate. This
  corrects the fixture while preserving its required SAT and failed-proof checks;
  the stopped runner remains failed and later scenarios unrun.

- 2026-10-28, `9e86ce6a` runner / `4607ccdf` built consumer: corrected basic
  scope PASS, 64/64 cases in 222.248 seconds; final source/runtime checks pass.
  Actual interruption scope stops before any probe signal on its strict live
  compiler identity guard; no lifecycle acceptance is credited.
- 2026-10-28, `4607ccdf`: full native end-to-end attempt FAILS at the unchanged
  output control after a temporary compiler-input witness is removed. An older
  argument-variant marker causes an unnecessary rebuild. The compiler-origin
  cleanup correction requires both owned layouts to be cleaned and independently
  checks physical fingerprint invalidation before retiring package facts.
  Later e2e phases and full fast/standard are unrun in that attempt. The output
  failure collector is incomplete; original stdout/stderr and producer records
  remain preserved, with no conformance credit.

- 2026-10-28, `7a1c420b` lifecycle runner / `4607ccdf` consumer: interruption,
  public cancellation, process extinction, restart and read-removal controls
  pass. Real Vest A1/B verify and returned A2 naturally restores with zero
  verifiers and A1's establishing run, but the runner refuses full attribution
  JSON equality: restored attribution is unavailable. The original failed
  assertion and complete field differences remain preserved. ADR 0050's
  verdict/diagnostic/origin contract does not promise identical attribution;
  the next control must recognize only its precise documented unavailable
  shape, retaining exact checks on every other field. No product fix or main
  bug is inferred from this harness assumption.

- 2026-10-28, `204f7322`: strict native checkpoint stops at two new test
  identities missing their Cargo-name field and one production conditional
  style lint. No tests or later runtime gates run. The identity fixtures and
  equivalent branch ordering are corrected without dropping any check.

- 2026-10-28, `aefef5ca`: strict native workspace clippy and all 10 focused
  compiler-origin/cleanup tests PASS, including seven new controls. The current
  312-path source capture includes the new test file; assembly, inventory and
  working-source checks pass. All 35 model and 171 refinement compiled programs
  remain byte-identical to the actual `dbda2535` Stock calls (no new proof calls).
  The added cleanup I/O is surrounding unproved code, qualified by Rust tests
  and runtime gates rather than that proof correspondence.

The final replay-coverage audit also refuses recording when a compiler
observation has no artifact-backed admission. Its observed report remains
available; unknown receipt inputs cannot become no-op authority. This correction
is local to the open replay fix and receives a focused actual-route regression.


- 2026-10-28, `a1600f19`: scoped clippy and release build PASS; native host
  tests pass 1,182/1,183 with 54 inherited skips, with only the known old macOS
  descriptor refusing the shared new fork pin. Basic acceptance passes 64/64,
  lifecycle passes 10/10 (actual owned cancellation, restart and profitable Vest
  A1/B/A2/A3 restoration), and solver tests pass 51/51. Full end-to-end FAILS
  at unchanged-call recording: the carrier incorrectly required every missing
  target to have a report even for human output's permitted unreported target.
  Later full fast/standard remain unrun. The correction binds the exact returned,
  admitted subset, preserves requested-coverage failure for omitted targets,
  and refuses included unknown/artifactless reports. A strict readable invocation
  inventory independently excludes any real or uncertain compiler start before
  recording; the public compiler counter is unchanged. The original end-to-end
  assertions remain unchanged for the next attempt.

- 2026-10-28, `0ae88ef9`: strict clippy and all 18 focused replay/admission
  controls PASS; host tests pass 1,184/1,185 (54 inherited skips), with only the
  known pending macOS descriptor failure. Release build and basic64/64 PASS;
  solver51/51 PASS. Full unchanged e2e progresses through ordinary replay but
  FAILS at focused `partial-record`: all three calls genuinely start demo and
  its no-verify vstd dependency (two starts each), despite known source-eligible
  observations. The marker-clear predicate mistakenly requires whole-proof
  success, which focused and no-verify reports intentionally cannot claim.
  The correction separates non-authorizing input-observation completion from
  proof success; all scoped receipt and publication rules stay unchanged.
  Full fast/standard remain unrun for this attempt; its failure collector is
  incomplete, and absent raw marker captures are not inferred.
- 2026-10-28, `0ae88ef9`: the independent public #553 counterpart PASSES:
  unchanged `--release` warm call uses zero compilers; changing only FORCE for
  one identical call genuinely starts the selected unit and returns a new
  Current confirmed report (CLI0, one compiler, no SAIL3107). The matching
  actual start, non-retained invocation, raw complete-scope verifier output and
  final source/runtime/tool guard are preserved. The main issue stays open
  until its fix merges; this is an operational rebuild check, not a false-proof
  reproduction.

## Current WIP integration

PR #572 is based on main `e1365eb2`, which already contains the independent
#552 replay repair. The latest Linux receipts remain attributed to `96de5097`.
Its normal e2e/example-enabled binary (`11246f3c...`) and normal conformance /
minimal production binary (`2837f369...`) are distinct builds. Each separately
passes basic64 and lifecycle10; complete e2e uses the former, full corpus and
mutation checks the latter. The ordinary minimal release command selects the
latter bytes exactly. No binary equivalence is inferred from shared source.

The actual macOS descriptor and selection guard are staged proposals, pending
remaining Mac gates and reviewed runtime publication. Overlapping fixes for
#553 and #555 are being reconciled with focused PRs #570 and #557; this WIP
branch must not duplicate their delivery. API-only socket-key issue #571 remains
separate. Final source-closure/proof-program correspondence, platform gate and
required hosted CI are pending on the final reconciled head.
