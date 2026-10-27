# Known issues and limits

- [#508](https://github.com/Chelis-Lang/sail/issues/508): a proof-only false
  assertion plus an erased empty statement could masquerade as a runtime
  mutation. The correction erases every consecutive empty statement after an
  erased group; regressions cover zero/one/multiple separators and keep real
  runtime mutations accepted. The first single-separator correction remains
  recorded as incomplete. Native metadata qualification passes all 676 current
  descriptors and four focused source tests at `2c2106d8`; all three captured
  proof programs match qualified `f8b26511` byte-for-byte. This does not execute
  the negative suite or prove semantic inequivalence from token differences.

- [#510](https://github.com/Chelis-Lang/sail/issues/510): the earlier complete
  query suite refuses four controls because the unchanged nonempty stdout
  comparison witness reaches default Stock resources. No query partition earns
  full qualification from that attempt. Exact proof cuts and the four original
  negatives remain required on the expanded source; no witness or assertion
  may be removed to make the suite pass.

- [#505](https://github.com/Chelis-Lang/sail/issues/505): the combined authority
  control `request-verification-enabled` produces its intended postcondition
  failure but also exhausts the unrelated human-footer renderer at default
  Stock resources. Whole scope and all 1,011 authority functions remain, so
  the strict runner refuses qualification. The correction moves the unchanged
  byte-copy loop and every invariant into a checked helper; the renderer keeps
  its complete original contract. Six controls follow their actual moved
  operations. Frozen `ad019e9a` passes the complete authority positive, all nine
  selected ordinary controls, strict package Clippy and 15 native replay tests.
  The complete combined authority matrix remains required.

- [#506](https://github.com/Chelis-Lang/sail/issues/506): the hosted output phase
  retains empty stdout and one ordinary colored Cargo footer, but byte-only
  replay recognizes plain prefixes only. The exact ANSI bytes are preserved;
  arbitrary escape stripping or broad stdout acceptance is not a correction.
  Exact colored-prefix support and full output qualification remain open.

- [#507](https://github.com/Chelis-Lang/sail/issues/507): the managed ghost-only
  include case remains Unknown after both existing allowed verification calls.
  Both requested targets are complete; the second call re-verifies vstd and
  reports freshly established build-script declarations. The underlying
  observation/declaration mismatch remains unconfirmed. Conservative Unknown,
  the original positive assertion and ghost-file invalidation stay required.

- [#496](https://github.com/Chelis-Lang/sail/issues/496): combined runner `eaf51208` drops three parser import-binding hash entries during table reconciliation; existing native source tests catch the lost hidden-import refusal. The correction restores the exact parent-table union. Canonical source/Stock barriers remain independent; no false proof is demonstrated.

- [#495](https://github.com/Chelis-Lang/sail/issues/495): first parser/cache/service composition `eaf51208` preserves both dependency sets but orders them differently from the fixed driver. The native source preflight correctly refuses before Stock; the correction retains the exact set in canonical order, without weakening closure checking.

- [#493](https://github.com/Chelis-Lang/sail/issues/493): `68b84947` human-context spec accessor omits its two retained lifetimes, so direct Stock compilation refuses before proof checking. Native strict Clippy and all 15 focused tests pass because Cargo erases that spec. The follow-up names both lifetimes; complete requalification remains pending.

Open Sail defects, integration limits and current product limits. Fixed defects are
removed from this page; their issues and the [changelog](../CHANGELOG.md)
keep the history. The [roadmap](ROADMAP.md) tracks release work.

The frozen `3dbebad5` full native end-to-end run passes contracts, legacy and the
initial output comparisons, then fails unchanged-call recording
([#476](https://github.com/Chelis-Lang/sail/issues/476)). Native diagnostic
`a73d13c2` identifies the preceding proof as Historical and the following Current
Cargo no-ops as empty observed target inventories. Whole-request admission
correctly refuses those no-ops. A separate human-output candidate retains that
refusal and exposes only empty stdout plus a parsed Cargo footer, with exact
request/path/readback checks and no ProjectResult or proof authority. Frozen
`0df9c812` passes the complete authority positive (520/912), strict package
Clippy, all 15 selected replay tests and source suites. Actual end-to-end output
now records/replays it, then fails an invalidation trace assertion because the
subsequent proof-cache lookup overwrites the original human refusal. The source
correction retains that refusal and ordinary execution; new negative controls
and complete end-to-end/conformance qualification remain pending. The same
failure's evidence collector reaches its file cap before producer state
([#477](https://github.com/Chelis-Lang/sail/issues/477)). Selection now prioritizes
the last issued command and recent producers under the original caps; truncation
or omission still fails capture completeness. Native collector controls and the
real failing output run confirm the prioritization under the unchanged cap;
the output phase still fails recording before the byte-only candidate.
No false Sail verdict has been demonstrated.

The frozen `4c43e5a9` full native end-to-end gate stops on the neutral-contract
positive with backend currentness Unknown and `NotProved`
([#471](https://github.com/Chelis-Lang/sail/issues/471)). Its journal records the
actual provider intent/start and artifacts but omits the provider completion.
The accounting observer read its endpoint directly from the environment,
while the original managed driver uses the private transport file. The source
correction uses that same transport accessor. A freshly rebuilt native baseline
fails the intended assertion; the correction passes the real file-backed alias
regression for absent and conflicting caller variables and strict package
Clippy. Full end-to-end/conformance qualification remains pending.
The conservative incomplete-observation refusal and positive expectation stay
unchanged; no false proof has been demonstrated.

Local containers previously shared one Cargo target volume across prepared
checkouts mounted at `/work`, allowing Cargo to reuse a different snapshot's
binary when both source trees predated its build
([#475](https://github.com/Chelis-Lang/sail/issues/475)). A controlled native
attempt performs no corrected-source compilation and reruns the baseline
failure; explicit per-source package rebuilds produce the intended red/green.
The local wrappers now select a target volume per physical checkout and expose
its name for evidence collection. Real wrapper-command tests reject the old
cross-checkout sharing and pass the correction, same-checkout warmth and symlink
aliases. Frozen `fa1b7e74` passes the full actual native workspace gate using a
fresh physical-checkout volume, including all 1,279 tests; custom qualification
runners must independently bind their compiled artifacts.
Direct-source stock proofs are unaffected.

The frozen `4c43e5a9` ordinary-control matrix stops at query control 151:
its mutation selector replaces `1nat` in an ensures clause instead of changing
the executable comparison method ([#468](https://github.com/Chelis-Lang/sail/issues/468)).
The strict runner refuses its pre-verification type error, zero obligations and
empty function inventory. Three other query partitions terminate normally;
inputs and authority never launch. No complete component or matrix qualifies.
The correction preserves the full original method and contracts, changing only
terminal acceptance to refusal, and adds balanced-contract executable-location
checks plus source regressions. Source inspection of all descriptors finds no
other misplaced production selector; native ordinary-negative requalification
remains required in the [owning record](validation/acceptance-boundary-refinements.md).
This is a qualification-tool defect, with no demonstrated false Sail verdict.

The pinned stock verifier rejects the actual `Arc::ptr_eq` operation used by
native memory settlement ([#473](https://github.com/Chelis-Lang/sail/issues/473)).
Its whole-file nonempty support probe exits before verification with no resource
marker; this is a proof-support gap, not a native Arc defect or false verdict.
The replacement insertion-generation/accounting operations have complete Stock
positives and all 53 new ordinary controls; full inherited/service qualification
remains open.
The existing native LRU clock also increments without an overflow guard
([#474](https://github.com/Chelis-Lang/sail/issues/474)); its bounded source
reproduction recipe is unrun. The source candidate uses the checked clock in the actual map paths and
preserves it across clearing; its bounded exhaustion regression and reset mutation
qualify natively. A separate public Config.queue_wait overflow also has a checked-refusal source
fix and a passing native regression ([#478](https://github.com/Chelis-Lang/sail/issues/478)).
The [owning record](validation/cache-candidate-refinement.md) separates these
limits from future production and solver-origin proofs.

The candidate's incremental slot charge originally adds 8 bytes for generation,
while complete old/current native types measure 80→96 bytes after alignment
([#486](https://github.com/Chelis-Lang/sail/issues/486)). The correction preserves
the original logical 128-byte allowance and adds actual carrier growth with a native
layout/admission regression. Its corrected-source qualification is pending in the
[candidate record](validation/cache-candidate-refinement.md). This is an
incremental accounting gap, not a proved total-budget/RSS violation or false
verdict; the shared accounting proof intentionally covers supplied charges.
The next `cc98352f` pass refuses opaque public predicates and unsupported
`mem::replace` (#481). Exact state projections and the already-modeled
Option::take preserve the real owned transfer without a new trusted primitive.
The copy contract also needs exact continuation because input Idle and Comment
have the same raw-history predicate (#482); the actual copy implementation is
correct. Source corrections, nonempty copy witness and whole-copy control are
implemented; new whole-root/native/control qualification remains pending.

The `0ef78997` outer/owner whole-query capture refuses proof-program
compilation at erased match grouping and Seq index projections
([#481](https://github.com/Chelis-Lang/sail/issues/481)). The correction preserves
runtime code and contract meaning; actual whole-root verification has not yet
begun for this wider candidate. No new inventory or native/verdict claim follows.

The outer extraction left two native-delegation source-fixture selectors
pointing at removed implementation text
([#484](https://github.com/Chelis-Lang/sail/issues/484)). The targeted source
unit reproduces 0 != 1; its correction mutates the actual thin input/finish
adapters while retaining all four baseline, unique-selection and refusal
assertions. The same targeted source test passes; no native/proof/service credit
is implied.

The newer outer-stream candidate's owner wrapper initially exposed only the
raw-byte relation, which allowed an empty returned inventory in its contract
([#480](https://github.com/Chelis-Lang/sail/issues/480)). The correction retains
exact parser continuation and complete ordered emissions through the actual raw
owner, with a concrete nonempty shared witness and drop/reorder controls. This
is a candidate refinement gap, not a demonstrated native wrong verdict. Fresh
whole-root/control/native qualification remains pending.

The expanded outer/raw-owner native package at `88ab662d` passes all 22 SMT
tests after a forced actual rebuild, then refuses four strict style lints
(#470 recurrence). Documented local annotations/backticks preserve all qualified
body tokens, predicates and assertions; a fresh strict native rerun is required.
No native verdict/test failure is demonstrated by these style diagnostics.

The [actual owned streaming-list kernel](design/acceptance-query.md#actual-owned-streaming-list-kernel-candidate)
passes the complete query positive at `a182c1e4` (482/837, all earlier functions,
final identity checks). The `e6e0e67a` native package passes all 22 SMT tests,
strict Clippy, the actual dependency graph and six layering unit tests. Ordinary
controls, conformance and the broader parser/solver joins remain unqualified:
actual top-level emission inventory, absolute offsets/capacity and lexical
classification still need production refinement under #371. No origin or
full-service authority follows from this structural kernel.

The [owning record](validation/acceptance-boundary-refinements.md) retains all
failed proof attempts and the corrected proof-pattern (#469) and native-style
(#470) defects. The reviewed #468 runtime-body guard repairs comparison control
151 without changing the other 170 descriptors; the ten new edits and corrected
151 target actual executable operations and await whole-root diagnostics.
The root-qualified `9099f673` correction also excludes loop-contract-only
selectors and preserves loop annotations. All eleven actual runtime edits pass
that stricter source guard; the bounded 9c3b4ea6 run now passes all eleven complete-positive/ordinary-negative
pairs with final checks. The new outer/owner source and full matrix remain
unqualified.
The request/domain candidate passes all complete production Stock roots but
native test compilation at `206c077f` stops on three old-interface uses in the
noop and runner fixtures ([#489](https://github.com/Chelis-Lang/sail/issues/489)).
The correction uses the independently declared original library tuple through
checked insertion, propagates request-construction errors, and inserts the real
additional missing target. Every original assertion remains; native tests and
ordinary negatives are still pending. The fixture-only file is not a proved
input adapter; only the reviewed surrounding runner binding is refreshed.

The original request/domain candidate's first complete native source check
refuses inconsistent capture bindings, dependency ordering and an incomplete
assembly fixture ([#479](https://github.com/Chelis-Lang/sail/issues/479)). The
correction binds both roles of the actual API library, retains every dependency
in its declared order and adds the real immutable key's public-field negative.
The stopped run reaches no Stock or native compilation stage; corrected-source
qualification remains pending in the
[owning record](validation/acceptance-boundary-refinements.md).

The unselected original-target comparator's first whole-file probe stops at four
VIR errors because its Eq/Ord/Clone methods use an envelope that emits an explicit
`requires(true)` instead of inheriting the trait preconditions
([#472](https://github.com/Chelis-Lang/sail/issues/472)). A separate checked
trait-method envelope preserves every native body and postcondition without an
implementation precondition clause. A later whole-file library support probe
passes 17 verified functions and all 333 actual function details after three
native-equivalent enum variant tests become direct matches. The complete
production request/domain proof and native caller/ordering checks remain pending;
the library support result establishes no full request or physical-input closure.

The original requested-target scan counts raw `--workspace` and `--all` text
after the Cargo/Verus separator
([#467](https://github.com/Chelis-Lang/sail/issues/467)). A source candidate
routes the actual selection operation through a checked prefix scan and adds a
two-member/default-member regression. This is a source-confirmed request-scope
defect; no new native reproduction, Verus acceptance of those forwarded flags
or false verdict is claimed. Option/value-state parsing and full metadata
selection remain open. See the
[request-construction boundary](design/acceptance-production-refinements.md#pre-execution-request-construction-candidate).
Original request selection still needs full Cargo option/value-state and metadata
coverage correspondence ([#375](https://github.com/Chelis-Lang/sail/issues/375)).
The candidate's checked prefix scan and native fixture preserve workspace
selection before `--` (#467); its constructor/dictionary and actual captured-row
selection pass complete Stock roots and targeted native fixtures. Those checks
preserve the supplied original data. They do not prove that metadata enumerates
every real target or that the decoded journal authenticates the physical worker.
Ordinary controls, conformance/cost and physical/provider joins remain open. See
the [request boundary](design/acceptance-production-refinements.md#pre-execution-request-construction-candidate).

The standalone layout-only source policy accepts alias/runtime substitution when
protected name counts remain unchanged
([#494](https://github.com/Chelis-Lang/sail/issues/494)). Full native SHA binding
continues to refuse the drift; no complete-gate or false-verdict bypass is shown.
Exact old-field/variant and sizeof-position checks plus a direct source
counterexample regression pass the frozen `d6f3d144` production-source suite.
Full inherited/service qualification remains separate.

Pinned Stock rejects actual native byte-key HashMap remove_entry/get_mut
([#498](https://github.com/Chelis-Lang/sail/issues/498)). Its whole-file support
probe stops before verification without a resource failure; no new assumed
specification or ordinary-negative credit is adopted. The separate supported-API
and key-model probe is unrun. This is a proof-support gap, not a native HashMap
bug or complete cache-origin result.

The `6e239d21` native workspace runs 1,278 tests: 1,276 pass, two fail and 57
skip. Its baseline fixture expects a whole-request facet after the real retained
read path correctly projects absent freshness to Unknown
([#464](https://github.com/Chelis-Lang/sail/issues/464)). The correction preserves
the independently checked original pair for the whole-positive assertion and
explicitly retains the retrieved Unknown/data-only case and all old assertions.
Both test-only fixture corrections need combined
native qualification; solver remains unrun in the
[owning record](validation/acceptance-boundary-refinements.md).

The combined `6e239d21` workspace passes strict compilation and Clippy, then
its existing focused-failure contract fixture writes a partial document at the
legacy sidecar root instead of the owned selected-attempt directory
([#465](https://github.com/Chelis-Lang/sail/issues/465)). The actual document
remains whole, so its eligibility assertion correctly fails under focus. The
test-only correction captures the partial document through a fresh original
producer and removes the terminal from its actual selected directory. Whole
under focus, partial without focus and unsealed calls still refuse; production
predicates remain unchanged. The separate daemon fixture failure is
[#464](https://github.com/Chelis-Lang/sail/issues/464). Both fail again in a
strict two-test baseline; corrected-source focused and full gates are pending.

The later `e36c749b` native workspace stops before tests and solver at one
test-only line-limit lint in the coherent baseline observed-request fixture
([#463](https://github.com/Chelis-Lang/sail/issues/463)). A documented local
allowance preserves every statement/assertion and all production behavior;
corrected-source qualification remains pending in the
[owning record](validation/acceptance-boundary-refinements.md).

The combined `e91627d0` native gate passes strict compilation and Clippy, then
two existing receipt-erasure regressions fail
([#390](https://github.com/Chelis-Lang/sail/issues/390#issuecomment-6432437950)).
Declaration retention looks up a package-only revision in a batch keyed by exact
package/target/kind, so a valid old declaration domain cannot survive erasure.
The correction uses the actual target key and keeps current-revision, original
validity, exact-map and fresh-scan checks. Direct controls recompute current
target revisions independently and exercise matching admission before missing,
changed, invalid-original and sibling refusals. The two-test baseline reproduces
both failures; both corrected-source focused tests pass at `fbab3fd0`; the combined native
gate remains pending. The frozen full workspace result is 1,269 pass, two fail
and 56 configured skips; later solver stages are unrun.

The frozen `720e461e` combined native workspace stops before native tests at
two fixture compile errors: an omitted shared settle-constant import
([#459](https://github.com/Chelis-Lang/sail/issues/459)) and a missing compiler
environment argument in the selected-attempt completion fixture
([#460](https://github.com/Chelis-Lang/sail/issues/460)). Narrow source fixes
retain the real settle rule, supplied empty declared-environment fixture domain
and all original positive/negative assertions. The same gate reports style
limits and an unused import; local allowances preserve production bodies and
guards. Corrected-source native workspace and solver qualification remain
pending; the stopped attempt is retained in the
[owning record](validation/acceptance-boundary-refinements.md).

The full production runner refuses loop-dropping negative controls
([#454](https://github.com/Chelis-Lang/sail/issues/454)). The correction preserves
complete bodies and loop obligations while changing only acceptance exits,
and restores qualified query mutations omitted during integration. No count
check is relaxed; fresh ordinary-control qualification remains pending.

The combined gate also stops before compilation at a stale eleven-phase
completion fixture ([#453](https://github.com/Chelis-Lang/sail/issues/453)).
Its correction retains the actual ten-phase guard and workflow and checks
every required result, including rejection of an extra phase. The lightweight
fixture passes; the combined native workspace still requires qualification.

## Owned query publication control qualification

The option-state candidate adds nineteen ordinary controls to the inherited
115 query controls. Complete discovery at `069e91f6` passes query 441/776,
inputs 308/538 and authority 498/878 with all previous functions and final
source/runtime rechecks. Later combined replay/presentation discovery at
`e57f065f` passes query 471/821, inputs 308/538 and authority 502/883, again
retaining all previous functions and final checks. Its 395 unique planned
cases still need all 392 ordinary negatives and native/differential gates.
The later reconciled main/presentation/receipt source passes complete discovery
at `e36c749b`: query 471/821, inputs 308/538 and authority 510/895, with every
earlier function and final source/runtime checks. Its 400-case plan requires
all 397 ordinary negatives; the 409-call partition schedule is held for native
compile/Clippy clearance. The independently audited `53757913` archive covers
all three earlier positives and one ordinary control-111 diagnostic with no
resource marker; it does not qualify a complete component or matrix. All six
service obligations remain open.
Its existing assembly/private-token source fixture
omits the required option module (#458); the source correction adds that
fixture input and retains every original mutation. Native integration exposes a missing production
`Duration` import (#455) and an inline entry budget undercharge (#456).
The narrow source corrections preserve forwarding and the original bound.
The actual parser/delivery budget regression fails ordinarily with the old
formula and passes after charging the complete carrier; this focused native
test does not qualify the workspace, solver suite or source-shared proof root.

Whole positive discovery at `f8d61408` passes query 431/0, inputs 308/0 and
authority 498/0, preserving all earlier functions. The first `35373419` query
control run remains unqualified: one partition completes, one refuses a
resource-limit hit, and two refuse mutations that removed loop obligations.
The corrected `c44cdcfa` four-partition union passes all 115 query controls with
full verifier/function counts and final source/runtime checks. Native workspace
then stops on eight style diagnostics; reasoned local annotations retain the
checked bodies and need a fresh native rerun. The complete 340-case three-root
matrix, final main integration and service theorem remain unqualified. See the
[owning record](validation/acceptance-production-refinements.md).

## Owned-compiler refinement qualification

The report classification fix in
[#449](https://github.com/Chelis-Lang/sail/issues/449) keeps hash-bound native
input adapters out of `checked_source` and discloses their separate unverified
role and the pipe-read primitive. The targeted metadata regression passes;
integrated native qualification remains pending. Earlier overbroad role labels
do not qualify native bodies; use each case's actual compiled-file inventory.

At frozen `573b6e0b`, the owned-compiler input root passes direct stock Verus
with 308 verified obligations, 538 function entries and all 115 ordinary broken
controls. Three-root positive discovery also passes, retaining every old
function. Native workspace qualification stops before tests at one
`manual_unwrap_or_default` style lint in the new pipe completion operation.
Its documented local allowance preserves the executable body, contracts and
all controls; the corrected source still needs native qualification. Earlier failures remain
in the [owning record](validation/acceptance-boundary-refinements.md).
Full physical input, launch and service obligations remain open. These
validation gaps do not demonstrate a production false mathematical proof; see
[#374](https://github.com/Chelis-Lang/sail/issues/374#issuecomment-6427046792).

## Compiler-phase witness selected without its inputs

[Sail #425](https://github.com/Chelis-Lang/sail/issues/425) records the native
`4bd22455` solver gate failure: 53/54 tests pass, but the actual compiler-phase
witness lacks its required frozen build and external output directory. Workspace
and both complete proof suites pass; the controller leaves e2e and conformance
unrun. The candidate routes that witness through `ci/run.sh boundary-witness
phases` with strict required inputs, separately from pinned-Z3 solver fixtures.
Its assertions and missing-input failures remain unchanged. The frozen
`73f928d1` native solver suite now passes all 53 tests; later integrated source
still needs final qualification. See the [owning record](validation/acceptance-boundary-refinements.md).

The separate remove-only command-interface integration correction
([#452](https://github.com/Chelis-Lang/sail/issues/452)) preserves outer Cargo's
private-setting removal without granting it compiler configuration authority.
The source correction is retained from `9b4ceea2`; native compilation remains
pending, and no native compiler diagnostic is claimed by the source finding.

## Default-mode verdict differences

These break the correctness contract and stay listed until fixed and
covered by conformance.

- **Sail's solver route and Verus-fork requests remain visible to compiled
  code** ([#435](https://github.com/Chelis-Lang/sail/issues/435)). The
  pinned fork and the `sail-z3` shim read these from the compiler's
  environment. Build scripts and compilers see `VERUS_Z3_PATH`,
  `SAIL_Z3_PATH`, `SAIL_SOLVER_MODE` and `SAIL_DAEMON_SOCKET`, and
  `RUSTC_WRAPPER` names Sail's driver alias. The verified compiler's
  ` __VERUS_DRIVER_ARGS__` carries Sail's observation options. The fork's
  shipped-build and compiler-input requests (`SAIL_RUSTC_DEV_SOURCE_BASE`,
  `SAIL_VIR_SPAN_REMAP`, `SAIL_COMPILER_INPUTS*`) reach compilers while those
  features are active. No ordinary project reads these names. A proof or
  macro that does can still differ from plain Cargo-Verus.

## Input coverage and cache integrity

The acceptance integration candidate retains fixes for symlink digest binding
(#381), incomplete declaration providers (#382), exact environment names (#383),
guarded retained readback (#387), dep-info filename decoding (#408), tactic-check
response accounting (#397), exclusive cache temporary-file ownership (#412),
original-request capture during replay (#413), retained-frame admission (#427),
paired baseline publication (#429) and exact partial replay (#430). Their
individual evidence remains in the
[owning validation record](validation/acceptance-boundary-refinements.md);
current-main reconciliation is source-only and does not qualify the combined
production source.

The merged private identity revisions are distinct from both prior contracts
([#446](https://github.com/Chelis-Lang/sail/issues/446)): receipt version 9 and
proof-context encoding 3 require observed compiler environment, original
producer history and ghost-only file declarations. Main independently used
revision 8 for ghost-file admission, so the combined source rejects both prior
revision-8 contracts. No acceptance of an older record or current-main false verdict
is demonstrated by that source review.

The bounded source inventory transfer previously used declaration revision 2
while the proof branch required revision 3
([#447](https://github.com/Chelis-Lang/sail/issues/447)). The source candidate
uses one shared declaration revision and a mandatory, separately versioned
inventory format. Old inventories cannot be relabeled as current; incomplete
private coverage still refuses admission. Native reuse qualification remains
pending.

Dependency publication can still collide with a staging directory
([#304](https://github.com/Chelis-Lang/sail/issues/304)). The native-artifact
store remains disabled under [ADR 0053](decisions/0053-disable-unqualified-native-store.md)
after a changed native linker at the same `PATH` escaped its old admission
([#187](https://github.com/Chelis-Lang/sail/issues/187)).

The service-wide acceptance theorem is not complete
([#368](https://github.com/Chelis-Lang/sail/issues/368), #371–#376): complete
physical compiler inputs, truthful solver origins, observation atomicity and
all service consumers remain separate obligations. See the
[refinement guide](design/acceptance-production-refinements.md).

The [scalar-option candidate](design/acceptance-query.md#scalar-policy-option-readback-candidate)
checks original raw option fields and persistent native Session readback for
`:print-success` and `:rlimit`; complete stock-positive discovery passes,
while ordinary controls and combined native qualification remain pending.
It does not establish pinned-solver defaults, option/alias/reset
semantics or the complete SMT interpretation required by #371.

The [served-byte replay candidate](design/acceptance-query.md#original-served-byte-replay-comparison-candidate)
adds the actual mechanical comparison/count transition and a same-owner stdout
completion join. Complete stock-positive discovery passes; ordinary controls
and native qualification remain pending. Its statistics shape exception
does not establish arbitrary statistics equivalence, truthful origins or numeric
effort/reason semantics; these remain part of #371/#372.

The candidate replay completion join originally checked comparison completion,
owner and count without relating comparison arguments to retained stdout
([#457](https://github.com/Chelis-Lang/sail/issues/457)). The source correction
directly rechecks each original suppressed frame against its served receipt.
The same-owner/count but unsat-compared/sat-retained refusal witness passes
complete stock-positive discovery at `e57f065f`; controls and native
qualification remain pending. This finding concerns the
missing desired relation, not a demonstrated runtime false verdict or violation
of the earlier postcondition.

The shared [MCP/API presentation override](design/acceptance-authority.md#success-presentation-call-graph)
preserves existing verdict/establishment behavior. Its whole authority positive
passes at `e57f065f`; ordinary controls and native qualification remain pending.
The helper establishes no request/process/cache authority and
does not close #376's remaining shaping and serialization joins.

The [paired process-local presentation candidate](design/acceptance-authority.md#paired-completed-observation-view-candidate)
retains original request/result data through explanation, comparison and
baseline consumers, with only an optional declared whole-request facet.
Its proofs, native behavior and added cost remain unqualified; it supplies no
serialized authority, physical observation or end-to-end #376 theorem.
Source review found its generic baseline wrapper's bare serde default would
add an unintended `ResolvedRequest: Default` bound
([#462](https://github.com/Chelis-Lang/sail/issues/462)). The explicit
`Option::default` correction preserves absent-request decoding and wire bytes;
combined native compilation and the unchanged baseline fixture remain pending.

## Performance

Current numbers are in [performance](performance.md).

- **Unchanged calls that miss replay pay setup cost**
  ([#314](https://github.com/Chelis-Lang/sail/issues/314)). An unchanged
  `sail verify` replays in about 26 ms on Linux (60 ms on macOS) when every
  recorded stamp matches
  ([ADR 0082](decisions/0082-unchanged-call-replay.md)). When replay does not
  apply (for example after a `touch`, a recent ctime or a different
  argument), unchanged Anvil takes about 1.7 s against stock's 0.3 s, with no
  verifier run.
- **Tiny edits cost more than stock**
  ([#156](https://github.com/Chelis-Lang/sail/issues/156)). Input checks,
  receipts and complete reports add a fraction of a second to small edits.
- **macOS cold Anvil is slightly slower than stock**
  ([#278](https://github.com/Chelis-Lang/sail/issues/278),
  [#232](https://github.com/Chelis-Lang/sail/issues/232)): about 0.1 s on
  100 s, within run-to-run range. Linux cold runs are at parity or faster.
- **A miss re-solves the whole bucket**
  ([#158](https://github.com/Chelis-Lang/sail/issues/158)). Editing one
  function re-sends its module's whole solver session; on Anvil one bucket
  costs about 66 s.
- **The shipped vstd proof is not used with `rustc-dev`**
  ([#264](https://github.com/Chelis-Lang/sail/issues/264)). Its libraries are
  loaded by rustc, so the proof does not match; `SAIL3138` names them.
- **macOS resource-limit subsumption is unavailable.** Resource counts vary
  for one captured vstd query while inputs and answers agree. Releases ship
  exact default and limit-100 profiles; other limits verify vstd normally
  ([reproduction](../corpus/repros/z3-resource-count-variation/README.md)).
- **An engine verify after a repair re-verifies the selected crate.** A
  repair rebuilds that crate in the shared engine target, so the next
  `sail verify --engine interactive` with unchanged source verifies it again.
  The cause of the byte difference is not investigated.
- **Default and engine runs each verify vstd once in their own target.**
  Whether they can share a target is not measured.

## Product limits

- **Setups verified without proof reuse** (`SAIL3112`). These verify exactly
  as plain `cargo verus verify` does, with solver caching, but record and
  reuse no proof receipts
  ([workspace verification](workspace-verification.md#setups-verified-without-proof-reuse)):
  - a Cargo outside the qualified 1.93.0 to 1.99.1, for example 1.92 or a
    newer release from a project's `rust-toolchain.toml`, `RUSTUP_TOOLCHAIN`
    or `CARGO` (`SAIL3139`, [ADR 0084](decisions/0084-qualified-cargo-range.md));
  - tests, benches and examples; `--target-dir`; Verus's `--log` options;
    `--no-verify`; Cargo `-v` and `--message-format`;
  - `--sysroot`, `RUSTC_WORKSPACE_WRAPPER`, Cargo `--config` and
    configuration includes, and compiler or sysroot overrides Sail cannot
    identify;
  - loader variables such as `LD_LIBRARY_PATH` or `DYLD_LIBRARY_PATH`;
  - a Sail cache and package root that overlap.
- **Only Sail's Verus,** at the release it pins
  ([ADR 0043](decisions/0043-default-mode-runs-the-fork.md)). A project that
  needs another Verus release cannot use Sail yet. `SAIL_CARGO_VERUS` runs
  another `cargo-verus` only as a labelled development override
  (`warning[SAIL0108]`).
- **No single-file projects.** `verus file.rs` projects are not supported
  ([#164](https://github.com/Chelis-Lang/sail/issues/164)).
- **Instant revert applies to qualified dependency profiles only.** Retention
  keeps up to 16 revisions per target (1 GiB), with the first verified
  revision pinned. It is skipped for unqualified profiles and for targets too
  cheap to be worth retaining; `SAIL3146` says why. A first run without a
  retention plan (for example a forced run) gets no instant revert until the
  dependencies are rebuilt
  ([#285](https://github.com/Chelis-Lang/sail/issues/285)). See
  [instant revert](instant-revert.md).
- **Current is about inputs, not the verdict.** In doubt a result reads back
  Unknown, never Current. A few rare changes during a run are not detected;
  see [freshness](freshness.md) and [trust](trust.md). On macOS, a Cargo
  configuration file created and deleted again within one run is not
  detected (ADR 0041 note of 2026-10-14).
- **A focused check is not acceptance.** A successful focused repair check
  still needs a complete Current full-target verification.
- **Repair selectors have a bounded source scope.** An arbitrary external
  include may need a full run in a new selector context before Current
  evidence is available; changed external bytes still force verification.
- **Managed compiler targets are removed only by `sail cache projects prune`.**
  There is no quota or automatic collection
  ([disk use and cleanup](workspace-hygiene.md#managed-compiler-targets)).
- **No `singular` feature.** Like the stock release, Sail's Verus has no
  `singular`, so `by(integer_ring)` proofs fail as they do in stock. The
  repair engine falls back to ordinary verification (`SAIL3124`) and cannot
  accept a repair for such targets
  ([details](proof-repair.md#verus-features-the-engine-lacks)).
- **Combined output order.** The managed relay guarantees final stdout
  completion, not one global order across stdout and stderr
  ([#215](https://github.com/Chelis-Lang/sail/issues/215)).
- **Summary wording.** A failing managed job summary leads with `0 verified`
  before `… passed by Verus's totals`, and after a Rust compile error reads
  `0 verified, 0 failed, 0 unknown`.

## Integration APIs (experimental)

- Positive native mutation kills are not established: the strong
  boolean-contract literal mutants return solver Unknown, so their outcome is
  inconclusive ([record](validation/neutral-mutation.md)).
- A failed Verus function can retain an unknown primary obligation; the
  neutral contract projection returns `NotProved` and keeps the unknown count
  and reason ([reproduction](../corpus/repros/neutral-contract-unknown/README.md)).
- Lowering version two preserves compound-field definedness; new workers
  require it and retained reads refuse affected version-one projections. The
  source-job hold stays until the correction is qualified
  ([#323](https://github.com/Chelis-Lang/sail/issues/323)).

## Tests and environment

- **Short fixture deadlines under heavy parallel load on macOS.** Exec of a
  freshly written executable has taken up to 10.5 s during
  `cargo nextest run --workspace -j 32` on an 8-core Mac. Fixtures whose
  1–10 s bounds include such a start fail there and pass alone
  ([#280](https://github.com/Chelis-Lang/sail/issues/280)); the rule for
  fixture waits is in [testing](testing.md).
- **Unclassified missing-file error in macOS watch**
  ([#306](https://github.com/Chelis-Lang/sail/issues/306)). Seen once in a
  host gate; not reproduced, cause unknown.
- **Shared HTTP deadline fixtures.** The 250 ms lookup and coalescing
  fixtures have failed intermittently; cause unknown.
- **Verifier wait under amd64 emulation.** Verification sometimes stalls
  under emulation, which is not used for performance or Linux validation
  ([reproduction](../corpus/repros/linux-emulation-verus-stall/)).
- The configuration selector unit test reads the developer's own Cargo home
  ([#419](https://github.com/Chelis-Lang/sail/issues/419)).

## Verus and Z3

[The Verus tracker](verus-issues/README.md) lists every locally recorded
upstream issue, with its reproduction and workaround. Sail preserves raw
diagnostics and refuses uncertain attribution or coverage. Z3 4.16.0 can vary
nonlinear rlimit consumption on identical input; see the
[reproduction](../corpus/repros/z3-nla-rlimit-nondeterminism/README.md) and the
[determinism policy](decisions/0009-solver-nondeterminism-and-unstable-results.md).
Seed-dependent proof discovery is available only through the experimental
portfolio, with separate keys. Stock Verus prints JSON `func-details` in hash
order ([VI-0008](verus-issues/VI-0008-func-details-hash-order.md)), so the
deep stock control can report unseen orders as Different; the fork's own
deep gate compares exactly.

## Resolved by decision

- **Error order across modules.** A cache-warm run can print a unit's errors
  in another module order
  ([VI-0007](verus-issues/VI-0007-nondeterministic-error-order.md)).
  Conformance compares messages as a multiset
  ([ADR 0042](decisions/0042-diagnostics-compared-as-a-set.md)).
- **No Singular engine variant.** A target that needs a Verus feature the
  engine lacks falls back to stock verification (`SAIL3124`), and the engine
  arm counts it as `engine-lacks-feature`
  ([reproduction](../corpus/repros/engine-without-singular/)).

The actual lexical bridge has separate candidate-only qualification findings:
[#487](https://github.com/Chelis-Lang/sail/issues/487) corrects a nonrecursive
proof-witness reveal form; [#488](https://github.com/Chelis-Lang/sail/issues/488)
records strict native lint/rustdoc refusal after all 24 SMT package tests pass.
Scoped reveal, native lint documentation and exact endpoint facts are being
qualified without relaxing parser contracts or controls. Neither finding is a
native incorrect verdict. Whole-root and full controls remain pending.

The paired stdout candidate has separate qualification findings:
[#490](https://github.com/Chelis-Lang/sail/issues/490) corrects an inaccessible
proof-only projection helper call; [#491](https://github.com/Chelis-Lang/sail/issues/491)
records the partial-frame fixture byte-array lint after all 25 native SMT tests
pass. Empty-tape/coherence and finite witness VCs remain pending under #371.
These are candidate proof/style failures, without a demonstrated native false
verdict. All predicates, assertions and ordinary controls remain required.


[#500](https://github.com/Chelis-Lang/sail/issues/500) records the authored
input-owner fixture's output-line expectation on trailing CR. Actual old
`4c43e5a9` and current `efd3fb3e` native splitters produce byte-identical traces
at all boundaries: CR terminates the fourth input atom, with no pending item or
extra LF emission. The corrected test retains every pending/raw/hash assertion
on genuinely unterminated `foo` and adds the separate CR/raw/hash case. This is
a fixture defect, without a demonstrated production parser regression or false
verdict; focused and full native qualification of the correction remain pending.

[#501](https://github.com/Chelis-Lang/sail/issues/501) records the subsequent
strict key Clippy refusal of the new CR fixture after all five native prefix
tests pass. Its correction retains the zero-item assertion as explicit empty
array equality; this is a test-style failure, without a production parser
failure or false verdict. Focused rerun and full workspace gates remain pending.

[#503](https://github.com/Chelis-Lang/sail/issues/503) tracks the output-owner
candidate's native Clippy refusals after all 26 retained SMT tests pass. The
source uses verified saturating subtraction and question-mark syntax; the exact
pending-field Option map trial leaves one proof postcondition unproved, so its
explicit owned branches retain a documented local allowance. Tuple signatures
keep the actual field/queue transfer visible.
[#504](https://github.com/Chelis-Lang/sail/issues/504) separately records the
subsequent shim lifetime lint, corrected by splitting the test-only borrowing
initializer from the anonymous production impl. These are candidate style/proof
qualification limits, without a demonstrated native false verdict; strict
native rerun and complete integrated controls/gates remain required.
