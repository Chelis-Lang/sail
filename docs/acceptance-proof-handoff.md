# Acceptance proof WIP handoff

This is the implementation handoff for draft
[PR #378](https://github.com/Chelis-Lang/sail/pull/378), as of 2026-10-26.
The owner explicitly authorizes publishing this WIP before the unfinished
pre-push qualification. This authorizes publication, not merge or a correctness
claim. Keep the PR draft and preserve the complete original objective.

The objective is to complete the scoped production proof in the
[design](design/evidence-acceptance-core.md) and fix the resulting defects.
All six rows in the [obligation ledger](design/acceptance-boundary-refinements.md)
remain open. Component proofs, passing fixtures, checksums and supplied input
lists do not establish the complete service theorem.

## Current-main source reconciliation

The source-only integration branch `proof/acceptance-current-main-integration`
combines proof candidate `a46c53b4` with main `e2b5af1b`. It preserves main's
file-backed alias settings, packed Verus options, two-phase ghost environment
observation, shared stat stamps, target-specific declarations and content-based
tool identities. It also preserves owned compiler construction and EOF tokens,
original selected-attempt histories, independent fallback directories, strict
readback, exact-partial replay and original-request/baseline admission.

Both lineages independently assigned receipt revision 7 and context encoding 2
to different contracts ([#446](https://github.com/Chelis-Lang/sail/issues/446)).
The combined source uses receipt 8, proof-context encoding 3, noop revision 5,
retained archive revision 5 and source-input revision 4. Both original-producer
history and observed compiler-environment guards remain mandatory. The fallback
builder now propagates packed-channel configuration errors while retaining an
owned command and its own allocated directory; native qualification of that
newly fallible branch remains required. The bounded source inventory transfer
(#447) now shares the current declaration-record revision and uses a mandatory,
separately versioned inventory, while incomplete private ledgers remain
incomplete. Main's accepted ADR 0083 remains the temporal extension; the
unmerged framing proposal is ADR 0084.

Only source inspection, formatting and normal commit hooks qualify this
reconciliation. No build, test, Verus, solver, consumer probe, native host or
conformance run was performed for it. Canonical native/source capture hashes,
complete function inventories and final-head proof qualification remain pending;
unchanged prior hashes do not bless the merged native files. The a46 checker
retains exactly 78 query, 115 inputs and 107 authority required controls; the query agent's later expanded inventory must be integrated
separately. Discover and qualify the actual new query function inventory before
rebinding capture.

Workflow policy is unresolved: main #434 moves direct proofs/refinements to a
local-only tier, while the owner's session AGENTS.md explicitly requires them
in CI. The draft keeps eight-status completion and proof/refinement jobs
internally consistent pending that decision. This source reconciliation does
not authorize publication or resolve that policy; the lead owns final review,
qualification and PR publication. All six obligations #371–#376 remain open.

## Current-main capture binding

The native capture follow-up starts from `182ad788`. It rebinds the reviewed
API `stamp` module assembly and expands the input/authority native-only closure
from 19 to 48 files. Added bindings cover packed options, file-backed settings,
ghost environment, actual Verus module/dependency assembly, shared stat stamps,
tool identities, content reads, receipt/archive histories and versioned source
inventory transfer. The exact paths are the checker's `INPUT_NATIVE_SHA256`
keys and the manifest dependencies. Source-role reports keep every one outside
`checked_source`; none of those native files is compiled by stock Verus.

Textual reconstruction authenticates the complete `f8d61408` positive report by
its source hashes and compares the generated drivers and every actual compiled
file. The current assembly is byte-identical: query 18 files, inputs 9 and
authority 14. The manifest preserves the report's exact 431/308/498 verified
counts and 759/538/878 function inventories, and the current exact 115/115/107
negative-control lists. These are source-correspondence checks, not a new proof
run; the f8 report remains positive-only and does not qualify the native union
or all mutation cases. Full native/macOS, solver, conformance, cost and final
canonical proof qualification remain pending. No pure executable body, proof,
control or golden is changed by this rebinding, and the CI policy conflict
above remains unresolved.

## Ghost-file integration on newer main

The source-only follow-up combines frozen `9b4ceea2` with main `1f0ecd30`.
Main's #445 adds ghost-only file declarations from both compiler phases to the
unit dep-info, including the checksum rustc computed of consumed bytes when
Cargo uses checksum freshness. Missing required checksums or an unmergeable
report refuse receipt publication. Its original managed cold/warm/edit/revert
assertions and minimal VI-0037 reproduction remain intact. Main owns VI-0036
for ghost environment and VI-0037 for ghost files; any later consumer
interpreter record must use the next free number. Imported VIR stays
under the existing extern/dependency checks rather than this file-list merge.

Main independently assigns receipt revision 8 to that file requirement, while
the integration already assigns 8 to producer history plus observed environment
(#446 recurrence). The combined source therefore uses receipt 9, preserving
both original guards and requiring the new ghost-file merge before publication.
No old-record acceptance or current-main false verdict is inferred from this
source comparison. Context encoding 3 and the strict inventory/archive policies
remain in force.

Only seven reviewed native hashes change: the compiler adapter, ghost reader,
receipts, retained compiler/driver and shipped compiler/call adapters. The
48-file bound-native closure, generated stock driver/file bytes, exact positive
counts/functions and 115/115/107 controls remain unchanged. Source-role reports
continue to classify these producers as bound native code rather than
Stock-compiled bodies. The checked compiler command, actual EOF ownership,
selected/predecessor histories and independent fallback directories are retained.
The remove-only interface correction from `9b4ceea2`
([#452](https://github.com/Chelis-Lang/sail/issues/452)) remains intact: outer
Cargo can remove private settings while production compiler configuration stays
on the owned builder. Its native compilation is still pending; the source
finding supplies no reproduced compiler diagnostic.

The new main implementation and its performance-documentation/tooling changes
do not resolve the direct-proof CI policy conflict; the draft eight-status
wiring stays in place pending owner resolution. Frozen `9b4ceea2` qualification
does not qualify this later union: executable, proof, solver, conformance and
cost gates remain separate and unrun for it.

## Published source and worktrees

The PR branch is `proof/acceptance-followup`. Its implementation checkpoint
before this handoff is `fc8e71a4`: current-main safeguards through `a6dee1ec`,
checked v2 key framing and source capture, per-section Cargo child accounting,
guarded retained readback, required CI control partitions and the candidate
per-target declaration-attribution correction. The subsequent
handoff commit changes documentation only.

Additional committed work is preserved on these branches. It is not all
integrated into the PR branch; inspect the branch diff before cherry-picking.

| Branch | Source checkpoint | Contents and integration status |
|---|---|---|
| `proof/acceptance-next-integration` | `46238edb` | Same-owner raw transcript/key controller, complete supported head extraction, checked response policy, the #397 tactic-check fix and #393 reproduction. Merges preserve main's replay comparisons and retained-response budget. Canonical source/type/function/control capture still needs updating for the new modules |
| `proof/acceptance-expand-inputs` | `a715a88a` | Phase request/full final command binding (`1a6447c0`), checked declaration-row expansion (`6bf42ed3`) and executable proc-macro read controls. Still separate from the lead's canonical proof assembly |
| `proof/acceptance-selected-authority` | `5aa58a93` | Checked engine decline and owned attempt identity; native selected-slot histories; Receipt 7; dependency-store version 2 with original producer-subtree transfer/adoption and strict archive versions. Not yet integrated with the lead's daemon producer revision or full canonical assembly |
| `fix/managed-declaration-growth` | `903bc8f3` | Candidate correction for the confirmed build-script reuse regression, integrated into the lead as `fc8e71a4`. Complete typed ownership journals and all original receipt/value checks remain required; native end-to-end validation is pending |
| `proof/acceptance-expand-query` | `124775a0` | Query/policy milestones plus the actual checked cache-write transaction and exclusive temporary-file creation. Its canonical source capture and full writer/origin simulation remain pending |

The corresponding existing worktrees are
`.claude/worktrees/acceptance-followup`, `acceptance-composition`,
`acceptance-inputs`, `acceptance-authority`, `acceptance-authority-domain` and
`acceptance-query`. Reuse them after checking status; preserve unmerged work.

## Evidence that exists

The [owning qualification record](validation/acceptance-boundary-refinements.md)
retains the failed and interrupted attempts as well as the passing scopes.

| Source | Actual evidence | Limit |
|---|---|---|
| `a615dbba` | macOS: all 92 production cases, 31 model cases and 1,133 host tests. Native Linux: workspace, all 92 production cases, all 31 models and all 48 solver-fault tests pass | Full native e2e fails at build-script introduction; remaining e2e and PR conformance are unrun |
| `53f7ef5f` | All 103 canonical production cases and 31 model cases pass coherently on macOS; 29 capture tests pass | Does not qualify the later integrated native service |
| `1a29859f` | All three captured whole positive roots pass: query 262/0, inputs 113/0, authority 299/0, preserving every prior function. Key tests, layering, strict clippy and 29 capture tests pass | A positive-root probe does not replace the complete mutation matrix |
| `1a8c771c` | All 31 capture-checker tests pass. Direct stock-Verus query partition 0/4 passes all nine selected whole-crate cases and correctly reports incomplete component/suite qualification | Full final-head native and macOS gates remain pending |
| `46238edb` | All 32 shim unit/actual pinned-Z3 cached-session cases pass on macOS, including main's replay divergence and overflow controls | Complete captured proof assembly still needs refreshing |
| Input component | Whole local stock assembly 197/0; all 30 ordinary failing controls; 46 focused native tests, clippy and ownership doctest pass | Local component assembly, not canonical combined qualification or complete compiler I/O |
| Authority component | Whole local stock assembly 356/0; three identity controls each 355/1. Native library snapshot 479 passed/3 ignored, followed by passing strict version controls and clippy | Full final source Linux/conformance/cost and production-to-model composition pending |
| `903bc8f3` | Strict clippy and 25 focused native tests pass, including incomplete typed ownership-journal refusal | Earlier three mutation failures precede the final journal wrapper; full final-head gates and native e2e remain pending |
| `124775a0` | Whole local stock assembly 295/0; all six writer controls fail ordinary verification. All 22 cache tests, three script controls, clippy and formatting pass | Local component evidence; canonical integration, full native gates, origin and filesystem composition remain pending |

Private raw artifacts are addressed by immutable object key and SHA-256:

| Artifact key | SHA-256 |
|---|---|
| `experiments/2026-10-26/acceptance-combined-a615dbba-native.tgz` | `4da33109a22e0da58ee0ddda81b13785da0dec766688f1e75bc5cebca236a22d` |
| `experiments/2026-10-26/acceptance-combined-a615dbba-mac.tgz` | `59d6800de66bb37d899b5145e68a45978347d57f7a1e5929d96af47282f1c5d8` |
| `experiments/2026-10-26/acceptance-growth-diagnostic-a615dbba.tgz` | `380b4011668bad95914b82b0a9688bec81ddfa652fa6cdf8c9c70b346a350fbd` |
| `experiments/2026-10-26/acceptance-physical-argv-721c1262.tgz` | `b762c32425d6eb81b358681f6cfe5466fdb8e3896a8c9bb4e889f2aaef46e40c` |
| `experiments/2026-10-26/acceptance-observer-12459c9c-native.tgz` | `b753fc555f8020edd415b9eb5ef8fc3fcc08a58902c1afd871bb245932664ac7` |
| `experiments/2026-10-26/acceptance-key-06f48beb-native.tgz` | `b66e9cf42b2323c121dc0867426c1ef8457932926d11ca8961022d7690b3095b` |

These archives were uploaded and independently read back. They do not contain
the private authentication token or presigned storage URLs.

## Immediate failed gate and candidate correction

The complete native union fails at `ci/test-managed.py:809`: after introducing
build scripts, Sail runs three verifier invocations where the unchanged test
requires one. Both reported targets are Current and complete. The earlier
cold-edit reuse, fresh-copy and native compiler replacement controls pass.
This failure demonstrates a reuse regression, not a false proof.

The instrumented repeat isolates one vstd matcher failure:
`current_declared_inventory_equal`. All other recorded matcher facts are true.
Its old receipt records three names; the fresh global scan includes eleven
additional names from an unrelated build script. The diagnostic records names
and Boolean facts, not environment values. This is confirmed on candidate
`a615dbba`; current-main behavior was not reproduced separately.

The candidate fix retains declaration names by actual file, attributes only
known Cargo unit files to packages, keeps unknown/ambiguous declarations in
every target's conservative domain and uses the target dependency closure.
Old names are freshly sampled only from independently admissible receipts for
that exact target. Missing scans cannot be repaired by old receipts. Original
map equality, value, scope, commit, artifact and dependency checks remain.
Ownership records must also come from complete per-section typed producer
histories; do not invent dep-info read successes merely to seal a provider.

Rerun the full native e2e union after integrating the fix. The isolated managed
CI phase alone missed this sequence-dependent build-script regression.

## Remaining proof and correctness work

1. **Exact query and solver-origin linkage (#371/#372).** Update canonical
   capture for the actual same-owner prefix/head/policy code, then connect
   actual owned Z3 input/output/termination to local solved-stream publication,
   every writer and recovery. A successful write or valid digest is not a
   truthful solver origin. Preserve the distinction between query-only origin
   and compiler/request context.
2. **Complete physical compiler inputs (#373/#374).** The tracked declaration
   provider is demonstrably incomplete: an ordinary procedural-macro file read
   changes the pinned fork's proof result while both phases omit that file.
   The declared `include_str!` control does record it in the verification
   phase, which compilation later erases. Watcher queues and before/after
   metadata do not yet establish a global content observation point.
3. **Original input fidelity (#385/#386/#393).** Sail-added compiler environment
   and physical `--time` arguments have actual false-verdict reproductions;
   ghost-only environment inputs can authorize stale reuse. Fix the transport
   and complete observation rather than treating supplied declarations as
   exhaustive. The selected-slot candidate also changes compiler-visible path
   values and is not claimed transparent.
4. **Execution/request/report composition (#375/#376).** Complete submission,
   effective options, actual target/attempt selection, retained transfer and
   every CLI/JSON/job/repair/MCP success path. When integrating Receipt 7, update
   the private daemon job/baseline producer revision consistently and refresh
   its reviewed source binding; the lead still uses revision 2. Archived
   metadata cannot remint a live attempt. The retained arena still needs its
   own preserved-history transfer, and the model needs the valid-protocol plus
   missing-feature refusal branch.

The candidate fork `12459c9c` passes its full Verus test suite but fails actual
proc-macro argv controls. Do not select it, publish its runtime or update pins.
The normal-wrapper transport remains a proposal. Direct parent/peer PID may
bind one actual Cargo-Verus child, but descendant parent/image checks do not
prove Cargo launch intent. A proposed Tokio dependency and transport decision have not been
approved or implemented. ADR 0080 is temporal packaging, 0081 cache response
framing. Main's ADR 0082 owns unchanged-call replay; the proposed checked-key
framing decision is renumbered to 0084.

## Resume and acceptance

The presentation candidate source-shares MCP/API provisional override through
actual borrowed enum fields and `FunctionReport` without assuming projected
enums are Copy. It preserves all eight combinations and the MCP tier path.
Authority controls now total 116 (107 inherited plus nine); actual whole-root
positive/function discovery remains pending with the prior inventory unchanged.
Native MCP/API callers are hash-bound source, not stock-compiled bodies.
This does not establish request/process/cache authority or full #376 shaping
and serialization correspondence. See the
[presentation boundary](design/acceptance-authority.md#success-presentation-call-graph).

The served-byte replay candidate appends `query_replay.rs`, actual comparison
history refinement functions and a same-owner stdout completion join. Query now
has 156 ordinary controls (the 134 option/inherited controls plus 22 replay
controls); no positive/function count has been guessed. Whole-root discovery
must retain all old functions and derive the expanded inventory from Stock.
The lead's #454 full-body/loop-preserving mutant correction is merged; do not
restore earlier truncated control objects. Statistics interpretation/origins,
numeric effort/reason policy and native slice/owner correspondence remain open.
See the [mechanical replay boundary](design/acceptance-query.md#original-served-byte-replay-comparison-candidate)
for exact data retention, refusal behavior, accounting and pending cost gates.

The scalar-option source candidate adds `query_options.rs` to the actual query
driver, preserving every earlier function and all 115 inherited controls and
appending 19 controls. Native Session and SMT parser helpers are separately
hash-bound native source, never promoted to stock-compiled bodies. Whole-root
positive discovery must replace the intentionally retained old query count and
function inventory with actual verifier output before qualification. Then run
every old and new ordinary control, the native lexer/Session bridge tests and
full raw-output conformance. Inputs/authority proof bodies, expected counts,
function lists and 115/107 controls remain unchanged. See the
[raw option/readback boundary](design/acceptance-query.md#scalar-policy-option-readback-candidate)
for concrete syntax and unresolved pinned-solver interpretation premises.

The merged transport uses a dedicated remove-only configuration interface for
the outer Cargo command and the owned compiler builder. The ordinary Cargo
alias needs that operation in production; the broader mutable standard-command
compiler interface remains test-only. Owned command mutation accounting and
private child/pipe completion construction remain required. This source-only
integration correction still needs native compilation and the full gates.

Inspect the PR head, branch status and complete diffs before resuming. Integrate
committed components without overwriting newer main safeguards. Capture every
actual module/type/import/factory/function and preserve every earlier function
and control. Derive new counts from directly invoked stock Verus; estimates
from an agent assembly are not canonical receipts.

Run the full macOS host gate and all captured model/production roots and ordinary
failing controls. On an isolated native Linux lane run workspace, proofs,
refinements, solver faults, the complete e2e union including extra, and PR
conformance. Before merge, run the required full fast/standard conformance,
native validation-cost measurement, final-head review and hosted checks.
Never weaken comparators, assertions, inventories, deadlines or resource limits.
CI's four required control partitions repeat the whole positive root; their
disjoint union contains every original mutation. Default local qualification
still runs the complete suite.

Keep all six original obligations open until their actual production evidence
is established. The current handoff does not authorize a merge or release.
