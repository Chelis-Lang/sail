# Known issues and acceptance limits

Open product defects and release limits. The [roadmap](ROADMAP.md) tracks
release work; linked validation pages preserve the scope of each result.

## Acceptance authority integration

Compile-only Verus output can resemble a successful zero-function proof
([#367](https://github.com/Chelis-Lang/sail/issues/367),
[reproduction](verus-issues/VI-0035-no-verify-success.md)). The production
refinement candidate checks effective package options and withholds older
retained completeness, while preserving input Current observations. Checkpoint
`fe8be9ee` passes native/hosted gates and full fast/standard conformance; the
owner accepts its bounded timing result. Integration with main's revision-only
input projection requires fresh gates in the
[owning record](validation/acceptance-production-refinements.md).
The broader service-wide acceptance theorem remains open
([#368](https://github.com/Chelis-Lang/sail/issues/368)).

## Functional release qualification

Runtime `36d65c3f` passes all four complete installed mutation cohorts on
each platform: 651 matching comparisons each, no false verified results,
differences, instability or infrastructure errors. All ordinary and forced
reverts get the original's own answers. Both matrices have 31 valid
comparisons. The [current qualification](validation/shadow-index.md) names
the frozen binaries and later main integration; the
[private rc.6 record](validation/release-candidate.md) remains historical.

The extra Mac watch startup restart is corrected: unrelated sibling removals
or renames could disable suppression of delayed startup notifications despite
unchanged input bytes and filesystem identity. The fix passes the full 1,056-test
Mac host gate and twenty repetitions of both startup and edit/cancel integration
tests with unrelated filesystem activity. The historical missing-file RPC error
in #306 remains unreproduced with operation-specific diagnostics; its cause is
not established by this notification defect. See the
[watch qualification](validation/watch-and-store-races.md).

## Neutral source mutation

Positive native mutation kills remain unestablished. The strong boolean-contract
literal mutants returned original solver Unknown for incomplete quantifiers,
with no SAT rejection. Their public outcome remains inconclusive and the gate
remains false. The optional observation runner qualifies only separately
recorded API, survival, lifecycle and evidence-refusal controls. It does not
complete the mutation milestone. See the [qualification record](validation/neutral-mutation.md)
for the failed strict attempt and required future evidence. The [combined integration record](validation/integration-production.md)
binds the latest cumulative source and preserves the same positive-kill limit.

## Release performance

- **Mac resource-limit subsumption is unavailable on the current solver.**
  Resource counts vary for one captured vstd query, while inputs and answers
  agree. The recorder uses separately verified exact default and limit-100
  profiles; other limits safely miss. See the
  [reproduction](../corpus/repros/z3-resource-count-variation/README.md) and
  [qualification](validation/rlimit-subsumption.md).

- **Unchanged Anvil setup cost remains open** (#314). Exact-scope reuse
  starts zero verifiers without promoting partial reports to whole-target
  proofs. The current installed matrix measures 1.95 s versus stock 0.33 s
  on Linux and 1.83 s versus 0.28 s on Mac. Linux cold improves
  186.13 → 172.99 s; Mac cold remains 99.88 → 100.73 s. See the
  [runtime qualification](validation/shadow-index.md) and
  [exact-scope behavior](validation/exact-scope-reuse.md). The subsequent
  [projection follow-up](validation/ordered-input-projection.md) reduces a
  separately paired scoped Anvil unchanged workload from 1.713 to 1.647 s on
  Linux and 1.508 to 1.457 s on Mac. That small gain does not close this issue.
  The separately staged three-round Mac follow-up measures Anvil unchanged
  at 0.281 → 1.778 s; source and scope are identified in that record.
- **The release performance decision remains open** (#232, #278). Tiny edits
  and unchanged calls retain overhead. Mac Vest cold adds 0.06 s; Mac Anvil
  cold adds 0.86 s on about 100 s. All 31 comparisons are valid on each
  platform, with ten Linux and twelve Mac rows requiring review. Passing
  Vest rows use limit 100 on both arms; the corpus still retains the known
  default-limit failure. [Performance](performance.md) states the current
  numbers. Recording a loss does not accept it. The follow-up candidate
  has three valid cold pairs: Anvil 99.665 → 100.039 s, Vest
  39.116 → 38.029 s. The small Anvil gap and unchanged costs remain open.

- **IronKV's larger edit losses are removed by the tighter retention budget**
  (#330). All 153 current comparisons match on each platform; complete loops
  are 1.47× faster on Linux and 1.42× on Mac. Three individual Linux medians
  still add 0.02–0.22 s on roughly 8–9 s edits; one Mac median adds 0.22 s
  on a 4.14 s edit. Cheap-target undo uses ordinary solver caching, while
  the successful public Vest control retains instant undo. The earlier
  retention-disabled measurement does not qualify this product.

## Corrected defects and historical measurements

- **Explicit older Cargo no longer repeatedly rebuilds unchanged script projects.**
  Anvil with its selected Cargo 1.98.0 uses ordinary Cargo freshness and reports
  Unknown with no proof reuse. Three native unchanged-call pairs fall from
  60.97 s to 0.89 s. This fixes a separate unsupported-protocol path; it does
  not remove the pinned-toolchain setup costs above. See the
  [qualification](validation/unsupported-cargo-fallback.md).

- **CapybaraKV's repeated vstd rebuild is fixed** (#321, #292). The native
  three-round full mutation run now has 168 matching comparisons, no false
  verified result and about 1.9× faster edit loops. This supersedes the older
  0.63× failed cohort; it does not qualify another release artifact.
- **The deep source-suite harness correction passes** (#320, #315). The
  complete rerun has no shim disagreements or infrastructure failures. The
  fork's nonignored tests pass; stock retains its recorded existing failures.
  See the [source-suite record](validation/rvt-dispatch-tests.md).

- **The original focused Anvil benchmark never reached verification.** All
  18 retained Sail rows used non-JSON `sail verify` with `--verify-module`,
  which intentionally refuses partial verification. The benchmark correction
  uses the supported public JSON route and records unavailable stage timings
  explicitly. The selected runtime passes the focused Anvil stock comparison;
  three-run FAST determinism, all four DEEP modes and final guards also pass. The original failed
  rows remain indexed in the [entrypoint record](validation/perf-real-partial.md).

- **The earlier combined-runtime inventory failure remains recorded.** The native
  source suite observes 4,714 matching cases, but the enclosing run stops at
  its post-test inventory with 199 fully captured extra files. Original source,
  runtime and command/checker bytes remain unchanged. The independent complete
  final audit accounts for all extras without changing the original failed
  cohort. The later focused Anvil stock comparison and three-run FAST
  determinism, all four DEEP modes and final guards pass. The matching runtime
  pins are selected on main through merged #223, with all required CI checks green. See the
  [qualification record](validation/spec-parameter-names.md).

- **A first run without a retention plan never gets instant revert** (#285).
  After, for example, a forced first run, the native units and vstd have no
  producer certificates, so later runs refuse retention (at once since
  ADR 0061) until the project's dependencies are rebuilt.

- **The preverified vstd is not used on a toolchain with `rustc-dev`** (#264).
  On Rust 1.99.1 with `rustc-dev`, a traced fresh compile loads `rustc-dev`'s
  own `librustc_driver` and `libLLVM` copies through the dynamic loader. It
  also reads `liballocator_api2` and `libproc_macro_hack` during crate
  lookup. These are bytes the proof was not built with, so the miss is kept.
  `note[SAIL3138]` names the components; a minimal toolchain with `rust-src`
  gets the proof.

- **Instant revert has a narrow qualified profile and visible costs.** Final
  source `83128ad0` passes 72 fresh ordinary calls, genuine bundled Vest ABA and
  complete native default gates. It admits only the exact Linux/public alloc/std
  profile. Vest's complete cycle improves 13.45% against earlier Sail, but cold,
  changed and unchanged costs rise and final cache grows about 519 MB. The small
  control remains fresh and essentially unchanged. The earlier whole 72-call
  failure stays failed; see the [record](validation/instant-revert.md).

- **Portable vstd functionality is not release first-use acceptance (#161).**
  Current composed Mac evidence passes actual stripped recording, archive
  smoke and public first Serve with the shipped origin at changed physical
  Rustup/sysroot, Cargo, project and installation roots. All 1,872 function
  comparisons match stock; unchanged-index and failing-diagnostic controls
  pass. A separate native-only component checks fresh native records and a
  same-path compiler replacement. All six enclosing attempts remain failed;
  their successful components are bound and audited separately. Full Linux
  regression qualification passes. The latest complete Linux consumer passes
  nine Cargo-1.98 first-use rows (37.318 → 14.768 s, 2.527×), public diagnostic
  and index controls, fresh native checks and all 36 Vest regression calls.
  Including common minimal toolchain installation, totals are 46.202 → 23.652 s.
  It fails requested-target instant revert: returned A verifies again instead
  of restoring A1, while unchanged A3 reuses A2. Correctness comparisons pass;
  shipped/retained coexistence needs a product fix and requalification. Earlier
  operational/fixture failures stay failed. Earlier twelve-pair native ABI results retain
  their original source scope. The v1 store stays quarantined, and default
  release selection remains unqualified. The
  [compiler-boundary record](validation/vstd-compile-boundary.md) retains the
  failed attempts and precise component limits.
  The candidate factory records with the pinned Cargo, currently `1.99.1`.
  Managed runs now select that Cargo when the caller made no toolchain choice
  and it is installed with `rust-src`
  ([ADR 0058](decisions/0058-managed-builds-use-the-pinned-cargo.md));
  a caller's or project's own choice of another Cargo can still produce
  different exact compiler arguments and safely miss. The fresh-home
  measurements so far used the 426d8b0 runtime with Cargo `1.98.1` selected
  explicitly. On the 7d82994 runtime the Linux bundle is recorded, and first
  use from a newer default Cargo through Sail's selection measures 30.38 →
  15.35 s (1.98×) over four rounds; see the
  [compiler-boundary record](validation/vstd-compile-boundary.md). The macOS
  bundle is not yet recorded.

- **The stale solver-hash defect is fixed on main**
  ([#213](https://github.com/Chelis-Lang/sail/issues/213)). Every handshake reads
  the complete current executable bytes. A saved SHA is reused only after
  complete fresh-byte equality. Required CI and native acceptance pass; the
  matched 50-sample recovery screen measures about 1–3% lower active-case wall
  time and 34–37% lower daemon self CPU. The original same-size/restored-mtime
  failure remains in the [identity record](validation/solver-identity.md).

- **The duplicated source-suite progress flag is fixed on main**
  ([#203](https://github.com/Chelis-Lang/sail/issues/203)). The runner correction
  binds the original case and executable and keeps the strict baseline-must-pass
  check. The selected cc7 suite passes 4,709 Same with 58 ignored. The original
  rejection and its corrected private run remain in the
  [validation record](validation/rvt-progress-option.md).

- **Historical combined-output disagreement; final-stdout correction qualified** ([#215](https://github.com/Chelis-Lang/sail/issues/215)).
  The native memo candidate's original E2E run reordered the project summary
  and Cargo error relative to plain Verus. Cause unknown; the old trap removed
  the complete temporary streams. A bounded three-arm reproduction matched
  every retained comparison but used explicit daemons. A subsequent single
  automatic-daemon loop also matched with complete retained observations;
  neither accepts the memo candidate. The [failure observer](e2e-output-evidence.md)
  preserves bounded original bytes; [evidence and limits](validation/e2e-output-evidence.md).
  A separate actual-relay fixture demonstrates a causal output race (#215).
  Its first payload-IPC candidate passed Cargo-like fixtures but bypassed real
  Cargo diagnostic rendering. The original-pipe stdout-barrier successor passes
  focused/full Mac checks. Joined Mac759/Clippy and native gate/solver/all E2E
  plus extra/PR/full FAST and STANDARD, final guards and complete capture pass.
  PR #226 is merged after delivery CI and review passed. Complete retained
  Vest facts show no gain; both earlier capture failures and the unknown cause
  of the original missing-stream disagreement remain visible. The barrier
  guarantees final stdout completion, not a global cross-stream order;
  [qualification and limits](validation/managed-output-order.md).

- **Changed-check continuation remains unqualified** ([#158](https://github.com/Chelis-Lang/sail/issues/158)).
  Historical fork continuations have raw statistics differences. Three fresh
  full runs per real IronKV/Anvil trace also differ in literal clock bytes,
  so both one-use live samples were skipped; one IronKV sample additionally
  retains a memory-observer exit refusal. The strict gate stays failed and
  the prototype stays archived, with no default integration. See the
  [qualification record](validation/solver-snapshots.md).
- **rc.5 omits verifier stdout after dependency-store seeding** ([#184](https://github.com/Chelis-Lang/sail/issues/184)).
  A second new project reuses vstd but omits its `verification results::`
  line; plain stock on a fresh target prints it. Verdicts, exit codes and
  stderr matched in the reproduction. The source correction is merged and
  passed both-platform checks, full native fast/standard conformance and
  required PR checks. The original draft asset is unchanged;
  [evidence](validation/dependency-store-output.md).
- **Native-linker changes can be missed by dependency-store seeding** ([#187](https://github.com/Chelis-Lang/sail/issues/187)).
  On source `0a7c1d0d` plus stdout fix `2f7102c4` (macOS 26.5, Apple M3,
  pinned fork `73dea021`), replacing private `cc` bytes at unchanged `PATH`
  after a successful donor made fresh stock exit 101 with a linker error,
  while store-seeded Sail exited 0 without that diagnostic. Three native Linux
  rounds reproduced the mismatch on `3c0498c3`. The
  [ADR 0053 containment](validation/dependency-store-fail-closed.md) disables
  v1 consumption and publication; its native ordinary, FAST and STANDARD checks
  pass at their recorded sources. The revised nine-phase layout also passes
  its native gate, solver, full local union, independent managed entries and
  PR subset. Containment is merged, with all required CI checks green;
  the original draft asset is unchanged.
  Raw macOS archive SHA-256
  `7bea96ebf3bd10a292ec6508bb7322a8a799fbc25ef024a89cf256d90783f51c`.
  The unmerged #187 replacement has a separate APFS mmap guard blocker;
  the #166/#189 metadata correction fixes neither defect. Recovering the lost
  first-run speed with a sound replacement remains required work.
- **Draft rc.5 can hide source inputs through output-directory exclusions (#189).**
  Adding an alternative module under a configured Cargo target or Sail cache
  can reuse an old proof although fresh compilation rejects it with E0761.
  The source correction uses owned metadata targets and refuses managed
  reuse when Sail cache and package roots physically overlap. Native
  full fast/standard and the separate joined main checks pass; the original
  draft asset is unchanged. Cache/package overlap in either direction remains
  an unsupported setup, taking SAIL3112 fallback without Current proof state.
  See the [validation record](validation/cargo-target-context.md).

- **Same-name unscheduled-script cleaning: qualified correction** (#174).
  The held PR #197 candidate memoizes only successful Current scheduling under
  identical full inputs. Older Mac/native checks and the 0.94 s to 0.20 s fixture
  remain historical; its later native E2E ordering failure has unknown cause.
  The joined implementation passes Mac759/Clippy and full native qualification,
  including all four FAST/STANDARD arms, final guards and complete capture.
  Its current affected fixture improves 0.896 → 0.174 s (5.14×) across all nine
  measured calls; PR #226 is merged after delivery CI and review passed
  ([record](validation/unscheduled-script-cleaning.md)). The first Vest cohort
  lost detailed rows; the second retains complete facts showing no speed/CPU
  gain, while its capture-parent deadline failure stays failed. Source edits and
  unknown selections, including `--workspace`, still take conservative cleaning.
  Cargo 1.99 also cleans both versions for an exact package/version ID
  ([reproduction](../corpus/repros/cargo-clean-package-versions/README.md)).

- **Vest repair is about 1.8× M6, below the 2× figure.** The owner accepted
  M7's speed as measured (ADR 0038 amendment 2026-10-13). On the `13a1176c`
  runtime, three native rounds, every sample valid and Current: HumanEval
  2.14×, Vest 1.82× (each Vest cycle about 28 s against 51 s). L18's cheap
  items cannot close the gap without new mechanism: rustc incremental is
  already on, and Sail's whole overhead is smaller than the gap.
  [Gate 7](validation/m7.md#gate-7-frozen-repair-performance); L22 and L23
  (rescue lane report, tag `archive/rescue-m7-98f6292`).
- **`sail verify` costs a little on small projects and unchanged runs.**
  Managed verification is the only `sail verify` (ADR 0046). On a native
  c7i.4xlarge (rc.5, [performance](performance.md)), a small project's edit
  takes 1.17 s against plain Verus's 0.88 s, an unchanged re-run 0.36 s
  against 0.12 s (vest: 0.97 s against 0.22 s), and a cold run 1–2 s more.
  Solver-heavy edits are faster than plain Verus (vest 27.8 s against
  42.1 s). Measured before rc.5, they were 1.4 s slower than the removed
  pass-through (26.4 s in the same rotation): about 0.8 s of receipt and
  input checks and 0.3 s of Verus's `--output-json --time`. Where the time
  goes is in
  [ADR 0047](decisions/0047-attribution-logs-on-request.md#measurements).
- **Some setups get no proof reuse.** Tests, benches, examples,
  `--target-dir`, Verus `--log` options, `--no-verify`, Cargo `--config`
  overrides and loader variables such as `LD_LIBRARY_PATH` are verified as
  plain `cargo verus verify` verifies them, with one `note[SAIL3112]` line,
  but nothing is reused or recorded. Jobs, `watch` and MCP verify them the
  same way and mark the result `no_reuse`, never `current`
  ([workspace verification](workspace-verification.md#setups-verified-without-proof-reuse)).
- **Fresh projects verify vstd again while the native-artifact store is
  quarantined.** Current main disables cross-project compiled-dependency reuse
  under [ADR 0053](decisions/0053-disable-unqualified-native-store.md), because
  seeding can hide a native compiler failure. A fresh small project's median
  is 32.57 s in three native Linux rounds, versus seeded v1's 1.28 s and
  fresh stock's 34.36 s. Same-project proof receipts and exact solver caching
  remain available. The draft rc.5 archive still contains the older store;
  its approximately 1.4 s second-project result is historical, not current-main
  performance. Sound first-run recovery remains open in
  [#187](https://github.com/Chelis-Lang/sail/issues/187), and shipped verified
  vstd in [#161](https://github.com/Chelis-Lang/sail/issues/161).
- **The deep stock oracle can report unseen JSON output-order variants as Different.** The
  `output_json` tests print `func-details` in stock Verus's hash order
  ([VI-0008](verus-issues/VI-0008-func-details-hash-order.md)); with 3–6
  orders in 11 plain runs, a Sail-side run often shows an unseen one. On
  rc.3, six of eight rounds reported one Different, each with identical
  stdout and stderr that is a line permutation of plain's. rc.5's one round had 0 Different and 4 Unstable `output_json` tests. The arm runs the
  stock source build, which Sail no longer runs (ADR 0043). Release-candidate
  finding 11. The new [deep fork gate](deep-verus-tests.md) compares the fork
  with and without the shim exactly, and retains the original stock-with-and-
  without-shim control with ADR 0042's diagnostic-message comparison.
  JSON output from stock is not normalized. The selected `cc7bbce4` fork
  source suite passes 4,709 Same (58 ignored), with all 4,651 nonignored plain
  tests passing. This is the runtime-local fork gate, not a claim that the
  separate stock control is now green; stock-versus-fork equivalence remains
  covered by corpus FAST, STANDARD and DEEP.
  The original native `c0e7e0f5` stock control remains failed: 4,691 Same,
  3 Unstable, 1 Different, 0 infrastructure errors; its Different JSON proof-note
  output matched none of 11 plain runs. See the
  [deep-tier record](validation/deep-tier.md).
- **Default and engine runs each verify vstd once in their own target.** That
  was about 30 s on HumanEval for the first default `sail verify` and again
  for the first engine run (`sail verify --engine interactive` or
  `repair-check`), measured while default mode still ran stock Verus. Both
  now run Sail's Verus (ADR 0043); whether they can share a target is not
  yet measured. Repair functions share
  one target, so a new function no longer verifies vstd again.
  L19 measured the separate stock and engine targets and L23 the shared
  repair target (rescue lane report, tag `archive/rescue-m7-98f6292`).
- **An engine verify after a repair re-verifies the selected crate.** A repair
  rebuilds the selected crate in the shared target, so a later
  `sail verify --engine interactive` with the same arguments and unchanged
  source verifies that crate again (HumanEval 6.2 s instead of 0.6 s, Vest
  27.5 s instead of 1.8 s). Why the rebuilt bytes differ was not
  investigated. L23 (rescue lane report, tag `archive/rescue-m7-98f6292`).
- **Intermittent shared HTTP deadline failures.** The 250 ms
  lookup/coalescing fixtures have failed in many complete gates and passed in
  others. Cause unknown; the deadlines are unchanged.
  [Gate 3](validation/m7.md#gate-3-workspace-gate-solver-suite-and-native-ci), [#76](https://github.com/Chelis-Lang/sail/issues/76).
- **Verifier wait under amd64 emulation.** Verification sometimes stalls under
  amd64 emulation, which is no longer used for performance or Linux
  validation (ADR 0038). [#68](https://github.com/Chelis-Lang/sail/issues/68),
  [reproduction](../corpus/repros/linux-emulation-verus-stall/).
- **Release-candidate wording gaps (low).** A failing managed summary leads
  with `0 verified` before `… passed by Verus's totals`; after a Rust compile
  error the managed count line reads `0 verified, 0 failed, 0 unknown`; and
  the README's Install assumes `curl` and apt package lists, which a minimal
  container lacks. Findings 5, 8 and 9 of the
  [release-candidate record](validation/release-candidate.md#rc1-findings).

## Resolved defects

- **Unbound cache admission (#342).** Solver cache connections require a
  successful single `HELLO` before reads or writes; failed or repeated
  negotiation retires the connection. This closes the protocol binding gap,
  not the separate trusted-writer assumption for reply correctness.

- **Repair acceptance trusted inconsistent summaries (#345).** The acceptance
  boundary now checks report invariants, confirmed verified function verdicts,
  explicit zero verifier errors and whole-target scope. The reproduced case
  supplied a failed function with unchanged successful summary counts; no
  ordinary producer creating that state or accepted false theorem was established.

- **Cargo binary target-name collision (#344).** Reports and receipts retain
  Cargo's exact target name, so `foo-bar` and `foo_bar` no longer share compiler
  observations. Failed sibling invocations remain separate and ambiguous old
  identities require fresh verification. The reproduction supplied verifier
  report fixtures to actual target aggregation; it demonstrated lost failure
  reporting, not a false overall verification exit or accepted theorem.

- **Source symlink identity (#343).** Input revisions and retained-result
  currentness bind source aliases and linked ancestors to their destinations.
  Retargeting a file or directory link cannot preserve evidence merely because
  both destination files were already hashed. Regressions exercise production
  snapshot and read-check functions, including unchanged and exact-revert
  controls. The managed end-to-end phase also retargets a successful source
  alias to an invalid proof, checks the rejection and the retained job's
  Historical label, then restores the original binding. These controls do not
  claim that the earlier defect accepted a false theorem end to end.

- **Fork JSON proof-note order (#185).** The selected `cc7bbce4` runtime
  contains producer fix `5793253c`, which serializes both arrays lexically
  without changing internal membership, verdicts or diagnostic bytes. The full
  fork suite passes, including all nine source-JSON tests. The original failing
  sample remains in [VI-0021](verus-issues/VI-0021-proof-note-json-order.md);
  [qualification](validation/deep-tier.md#fork-source-suite-gate-124).

## Product limitations

- **Repair selector inventories have a bounded source scope.** Compiler-family
  imports cover package sources and generated inputs with validated build-script
  contracts. An arbitrary external include may require a full run in a new
  selector context before Current evidence is available; changed external bytes
  still force verification. Closing #77 qualifies the original generated-input
  defect, not this unsupported external-path extension.

- Current means the files a result depended on are unchanged; it is separate
  from the verdict. In doubt a result reads back Unknown ("inputs could not be
  confirmed unchanged; re-run"), never Current. A few rare changes during a
  run are not detected; see [freshness](freshness.md) and [trust](trust.md).
- On macOS, a Cargo configuration file created and deleted again within one
  managed run is not detected (owner decision, ADR 0041 note of 2026-10-14).
- A successful focused check still needs complete Current full validation.
- Managed compiler targets are removed only by `sail cache projects prune`;
  there is no quota or automatic collection. See
  [disk use and cleanup](workspace-hygiene.md#managed-compiler-targets).
- Sail's Verus, like the stock release, is built without Verus's `singular`
  feature, so `by(integer_ring)` proofs fail in Sail as they do in the stock
  release. The repair engine falls back to ordinary verification for such
  targets (`SAIL3124`), which cannot accept a repair. See
  [Verus features the engine lacks](proof-repair.md#verus-features-the-engine-lacks).
- Sail runs only its own Verus, at the release it pins
  ([ADR 0043](decisions/0043-default-mode-runs-the-fork.md)). A project that
  needs another Verus release cannot use Sail yet. `SAIL_CARGO_VERUS` runs
  another `cargo-verus` only as a labelled development override
  (`warning[SAIL0108]`).

## Verus and Z3

[The Verus tracker](verus-issues/README.md) lists every locally recorded
upstream issue, with its reproduction and workaround. Sail preserves raw
diagnostics and refuses uncertain attribution or coverage. Z3 4.16.0 can vary
nonlinear rlimit consumption on identical input; see the
[reproduction](../corpus/repros/z3-nla-rlimit-nondeterminism/README.md) and the
[determinism policy](decisions/0009-solver-nondeterminism-and-unstable-results.md).
Seed-dependent proof discovery is available only through named opt-in
portfolios, with separate keys.

## Resolved by decision

- **A cache-warm run can print a unit's errors in another module order.**
  `debug_expand.rs` sometimes printed the root module's error before `M4`'s
  with the same messages and verdicts
  ([VI-0007](verus-issues/VI-0007-nondeterministic-error-order.md)).
  Conformance compares messages as a multiset since
  [ADR 0042](decisions/0042-diagnostics-compared-as-a-set.md), so this order
  is no longer a difference.
  [Repro](../corpus/repros/sail-cache-warm-error-order/) (kept).
- **The engine rejected `by(integer_ring)` that a Singular build proves.**
  The engine runtime, like the stock release, has no `singular` feature, and
  the Singular deep run's engine arm reported all three Singular units
  Different (engine exit 1, never a false verified). The lead decided against
  a Singular engine variant: the engine manifest records its Verus features,
  a target that needs a missing one falls back to stock verification
  (`SAIL3124`), and the engine arm counts such units as
  `engine-lacks-feature`, never same or different.
  [Reproduction](../corpus/repros/engine-without-singular/) (kept),
  [deep-tier record](validation/deep-tier.md).


## A failed Verus function can retain an unknown check

The neutral add-one reproduction requires `input < 100`, ensures the result is
`input + 1`, and changes the implementation from `input + 1` to `input`.
On the pinned native engine, the actual managed function is labeled failed
while its primary obligation records unknown with reason `(incomplete quantifiers)`,
then unsat. A model returned after the unknown answer is not a SAT witness.
The neutral contract projection conservatively returns `NotProved` and retains
the underlying unknown count and reason. Legacy report semantics are unchanged.
The compiled body edit independently violates its postcondition at input 1,
which satisfies the precondition. This is a runtime sample, not a solver
refutation or mutation-gate result. See the [native qualification record](validation/neutral-verus-lowering.md)
and the [minimal emitted-source reproduction](../corpus/repros/neutral-contract-unknown/README.md).


## Compound-field definedness in neutral lowering version one

[Issue 323](https://github.com/Chelis-Lang/sail/issues/323) records an actual
source-job proof for a guarantee whose checked runtime overflows while selecting
a field from a conditional record. The original lowerer discarded definedness
of the selected record expression. Lowering version two preserves that condition;
new workers require version two, and retained reads refuse affected version-one
projections without rewriting the original evidence. Safe historical projections
remain byte-identical. Qualification of the correction, including actual unsafe
and guarded source jobs, remains required before removing the source-job hold.
