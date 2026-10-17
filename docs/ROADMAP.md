# Roadmap

The project is built in waves. Each milestone ends with evidence: green gates,
conformance results and, where relevant, benchmark data. Work widens only after
the previous milestone is shown to work.

## Initial session (foundation): done, 2026-10-08

See `docs/reports/2026-10-08-bootstrap.md` for the evidence.

- **M0: Workspace and guardrails.** The crate skeleton from the charter, the fast
  CI tier, lints, the AI-authorship guard (hook, CI and agent config), docs, and
  ADRs 0001–0002. *Exit:* both CI jobs green on `main`, the guard rejects an AI
  trailer locally and in CI, then switch to PR mode.
- **M1: Survey and integration decision.** `docs/survey/` covers Verus↔Z3
  internals for the pinned release, prior art, corpus candidates and solver
  behaviour. The pinned Verus is installed via `cargo xtask fetch-verus`. *Exit:*
  an ADR on the integration point, backed by an empirical transcript.
- **M2: Faithful pass-through.**
  - `sail-smtlib`: a byte-faithful parser and printer.
  - `sail-session`: assertion stack, options and the logical state at each
    `check-sat`.
  - `sail-z3` (`sail-shim`): forwards every byte to the pinned Z3, with an
    observation-only tap and transcript capture.

  *Exit:* a toy crate verifies identically with and without the shim.
- **M3: Corpus, conformance, baseline and profile.**
  - Pinned corpus.
  - `sail-conformance` diffs plain Verus against Verus-with-shim.
  - `sail-bench` baseline and generated `docs/benchmarks.md`.
  - Query corpus from shim transcripts, with a profile of solver time per query
    and function and of seed stability.

  *Exit:* zero verdict or diagnostic differences in pass-through mode.
- **M4: Report.** Survey findings, the integration evidence, baseline and
  profile, conformance results, and open questions.

## Initial product: complete, 2026-10-09

Milestone M5 and `docs/reports/2026-10-09-initial-product.md` record the
completion decisions and full local acceptance. The owner kept the unknown
serving cap at 50%; the later research below remains deferred.

## M6: Live workspace verification and team caching (implemented)

This milestone implements a complete local developer and agent
workflow. Ordinary single-developer use needs no credentials, server setup,
account, or container. One verification worker runs by default.

1. **Managed workspace verification** ([#64](https://github.com/Chelis-Lang/sail/issues/64)).
   Always invoke Cargo; retain complete successful per-target proof receipts in
   Sail-owned artifacts. Bind them to the exact verification context and actual
   artifacts. Return complete reports when Cargo reuses verified targets, and
   refresh missing or incompatible evidence.
2. **Persistent jobs, watch and MCP** ([#65](https://github.com/Chelis-Lang/sail/issues/65)).
   Submit, inspect, follow and cancel verification across sessions. Watch the
   workspace and local path dependencies; distinguish current results from
   historical or superseded work. Result lookups stay responsive during runs.
3. **Optional HTTP team cache** ([#66](https://github.com/Chelis-Lang/sail/issues/66)).
   Read and publish signed SMT entries over authenticated HTTPS, with an
   optional lightweight self-hosted service. Reader signing trust remains
   independent of transport authentication; remote failures cost misses.

Acceptance is retained in `docs/reports/2026-10-10-live-workspace.md`: complete
unchanged reports with zero verifier invocations, native CLI/MCP and recovery
checks, exact same-name library/binary solver inputs, full local corpus coverage,
and three-round secondary-platform resource measurements. Easy synthetic
fixtures show freshness overhead, not a general wall-time gain. Follow-ups are
safe freshness-cost reduction ([#69](https://github.com/Chelis-Lang/sail/issues/69))
and an unresolved emulated verifier wait
([#68](https://github.com/Chelis-Lang/sail/issues/68)). No deadline or check was
relaxed to classify a failed attempt as success.

Exit evidence: unchanged eligible workspaces invoke no verifier and return
complete reports; edits and verification-context changes invalidate correctly;
job cancellation/restart/currentness work across CLI and MCP; a fresh remote
reader consumes verified signed replies and silence certificates. Run full
local gates and affected conformance, with three-round performance and resource
measurements. Long tests stay local-only; every required CI job remains below
ten minutes. Persistent Verus internals, solver pools, normalization, stack-only
keys and new SMT backends remain deferred.

## M7: Interactive proof engine and repair workflow (in progress)

The approved contract is [ADR 0027](decisions/0027-interactive-proof-engine-and-repair-workflow.md).

- Optional pinned source engine: retain one compiler callback for a focused
  check followed by independent full validation; reuse conservative prepared
  lowering without retaining compiler state across source revisions.
- CLI/MCP repair checks, failure evidence, saved named baselines and comparison.
- A separate live proof-event journal with revision-aware projection and
  cancellation/reconnect handling.
- Shared complete input observation and responsive job reads, addressing #69.

Exit: the engine's full results and diagnostics match plain Verus on the local corpus
and affected source tests; established reads below 200 ms p95 during active
verification; and at least **2× median improvement on each** of frozen
human-eval-verus and Vest repair catalogues, measured on a native x86-64
Linux host ([ADR 0038](decisions/0038-m7-rescue.md)). Three rounds and spread
are required; every sample is recorded and judged afterwards. No successful focus is accepted before
complete current full validation. M7 remains incomplete until these gates pass.

### Current state

| Gate | Status | Best evidence |
|---|---|---|
| Engine build and qualification | Pass (source checks) | Fork `73dea021` (ADR 0039, no prepared lowering; adds the VI-0017 and VI-0007 fixes to `13a1176c`; VI-0005 fix dropped): stock fixtures and cancel/restart on Linux and macOS, fork CI library and `sail_vi*` tests, L43 ([fork-default record](validation/fork-default.md#runtime-qualification)); `13a1176c`: L22 (rescue lane report, tag `archive/rescue-m7-98f6292`) |
| Engine runtime delivery | Pass | `engine-runtime-73dea021-linux` and `-macos` pinned (schema 2, fork commit); a fresh CI image installs and validates the Linux one |
| Workspace gate, solver, native CI | Pass | `1fbaa57` 677/677 + 41/41 (emulated); native CI `213852a` all jobs; native-host gate 677/677 at `68a22c7` after the `Queue::offer` fixture fix, L2 (rescue lane report, tag `archive/rescue-m7-98f6292`) |
| Repair CLI/MCP workflow | Pass | `387ba80` original fixture, 90.588 s, unchanged 120/30/5 s limits |
| Responsive reads (p95 < 200 ms) | Pass (native) | `scripts/reader-gate.sh`: Status p95 9.8 ms, Result p95 25.7 ms with active verification, L7c (rescue lane report, tag `archive/rescue-m7-98f6292`) |
| Default and cache conformance | Pass (native) | `d187e49` on EC2 with runtime `13a1176c`: PR subset 33/33, fast 147/147 and standard 159/159 Same in pass-through, fill, warm and the engine arm, 0 Unstable, L22 (rescue lane report, tag `archive/rescue-m7-98f6292`). The intermittent cache-warm module order on `debug_expand.rs` is no longer a difference ([ADR 0042](decisions/0042-diagnostics-compared-as-a-set.md); [repro](../corpus/repros/sail-cache-warm-error-order/)) |
| Deep tier (release check) | Pass | `98f6292` on EC2: corpus deep tier 159/159 Same in pass-through, fill, warm and engine; `rust_verify_test` 4,695 tests, 0 Different in each mode (3–5 `output_json` Unstable, VI-0008); Singular projects Same in default mode; their engine arm counts 3 units `engine-lacks-feature` (stock fallback, `SAIL3124`; L26 `8b40111`, tested as `56415d5` with identical code before rebasing onto #85) ([deep-tier record](validation/deep-tier.md)) |
| Frozen repair performance (≥2× each) | Accepted at the measured speedup (ADR 0038 amendment 2026-10-13) | Three native rounds on runtime `13a1176c` (`d187e49`), every sample valid: HumanEval 2.14× (64.7 s / 30.3 s), Vest 1.82× (246.6 s / 135.1 s) ([benchmarks](benchmarks.md); L22 rescue lane report, tag `archive/rescue-m7-98f6292`); L23 measured 2.11× and 1.80× on the same protocol |

The source engine, repair CLI/MCP, resumable proof journals, explanations,
immutable baselines and comparisons are implemented. Since
[ADR 0043](decisions/0043-default-mode-runs-the-fork.md) every verification,
default mode included, runs Sail's Verus from the fork; stock Verus is the
conformance oracle. Start with [the repair walkthrough](proof-repair-walkthrough.md),
[the MCP guide](mcp.md) or [the SDK contract](result-client.md). Every gate
above has native x86-64 evidence; the speed gate is accepted at the measured
speedup rather than met at 2× on Vest (ADR 0038 amendment). M7's acceptance as
a milestone awaits the lead's review of this evidence. [M7 validation](validation/m7.md)
gives each gate's evidence and its indexed failed attempts, and
[known issues](known-issues.md) lists the open defects behind them.

## Open-source release (planned)

Sail stays private until the owner orchestrates the open-source rollout;
nothing is published to crates.io before then. It is then released as open
source under the MIT License, with macOS and Linux binaries on GitHub releases.
The owner has kept rc.5 as a draft and has not authorized 0.1.0 publication
or a repository-visibility change. Draft rc.5's native-artifact store is now
quarantined on main after a compiler-input mismatch; its historical first-run
speed gain is not current-main behavior. Sound first-run recovery and the
remaining core/performance acceptance gates must stay visible before a later
candidate is considered ([question 29](open-questions.md)).
Item numbers refer to [the roadmap review](reports/2026-10-13-roadmap-review.md).

| In the release | Status |
|---|---|
| A1: MCP works in Claude Code | Done (#89) |
| A2: agent-sized MCP replies | Done (#96) |
| A3: honest counts | Done (#98) |
| A4: macOS managed reuse | Done (#99) |
| A5: install and first run, targeting release binaries | Done (#95; the README installs from the release archive) |
| A6: papercuts | Done (#90, #91) |
| macOS and Linux release builds of Sail with the engine runtime | Done (#100; `scripts/release.sh` makes draft releases) |
| Engine runtimes from the public `Chelis-Lang/verus` fork releases | Done (#94, #100) |
| Performance-claims page backed by reproducible `sail-bench` runs on Linux and macOS | Done (#105, [performance](performance.md)) |
| Disk cleanup for managed targets (review B3) | Done (#97) |
| One `sail verify`: managed verification by default, pass-through removed ([ADR 0046](decisions/0046-managed-verification-by-default.md)) | Implemented; required gates and the standard tier pass on Linux; awaiting review |
| `LICENSE` (MIT) and `CONTRIBUTING.md` | Done |
| `sail new`: a ready-to-verify project, bundled `cargo-verus` kept off `PATH` (question 28) | Implemented ([#148](https://github.com/Chelis-Lang/sail/issues/148)): `sail new` runs Sail's Verus's `cargo verus new`; e2e checks it matches stock and verifies. Clean-install acceptance follows the managed-by-default and pre-verified vstd work |
| Output schema v2: establishment and evidence-tier axes (question 22) | Implemented ([#149](https://github.com/Chelis-Lang/sail/issues/149), ADR 0044): `establishment`, `evidence_tier` and ITF counterexample values in `--output json`, the project envelope and MCP; gate, solver, e2e and PR conformance pass on native Linux; macOS host checks pass |
| Pre-verified vstd for new projects and worktrees (owner direction 2026-10-15; review B1) | V1 is quarantined under [ADR 0053](decisions/0053-disable-unqualified-native-store.md) after the native compiler-input mismatch (#187). Three-round containment cost: fresh demo 32.57 s versus seeded 1.28 s and stock 34.36 s; [quarantine merged; native integration and required CI pass](validation/dependency-store-fail-closed.md). Historical v1 gains remain in the [original record](validation/dependency-store.md). A sound fast replacement and first-run speed recovery are required; no prebuilt archive support is selected. |
| Release-candidate validation: required gates, standard and deep tiers, and a repeat walkthrough on clean Linux and macOS, including Claude Code | Done: draft `v0.1.0-rc.5` (`c3687990`: managed by default, pre-verified vstd from the dependency store, `sail new`, schema v2; Sail's Verus, fork `73dea021`) passed the gate, e2e, every conformance tier with 0 Different, one `deep-rvt` round with 0 Different, both smoke tests and the README walkthrough on Linux and macOS; the repair benchmark showed no regression, but its outcome check needs updating for schema v2 (#179) ([release candidate](validation/release-candidate.md), [clean macOS](validation/clean-macos.md), rc.1); the owner reviews it and decides when to publish |

Out of the release: B2 (single-file
`verus file.rs` projects; the release documents that Sail needs a Cargo
project), B4 (receipts beyond the latest revision) and every section D item.

## Later waves (from the charter's initial scope)

1. **Caching.**
   - **Done:** key v2; a disk tier, a 256 MiB memory tier and write-behind
     stores (ADR 0011, 0015); serving far-from-limit `unknown` answers
     (ADR 0009); effort recording; fault injection; a shared tier of
     signed entries in a directory store (ADR 0017); optional authenticated
     HTTP(S) reading and publication with a self-hosted server (ADR 0025).
   - **Planned:** warm, exact-prefix solver sessions for edit-loop cache
     misses (ADR 0038, direction item 2).
   - **Deferred:** the opt-in stack-key mode (ADR 0008 update).
2. **CLI.**
   - **Done:** `sail verify`, `sail watch`, `sail serve`, `sail status`,
     `sail cache`, and the MCP server `sail mcp` (ADR 0016).
   - **Done:** `sail bench` delegates to the benchmark binary.
3. **Agent ergonomics.**
   - **Done:** function attribution, the ledger and `--output json` schema
     (ADR 0014; version 2, ADR 0044).
   - **Done:** fragility measurement: `sail fragility`, `sail-bench
     fragility` and the fragility census (ADR 0018).
   - **Done:** exact function scores in JSON/MCP and sequential,
     complete-run `accept-first-proof` portfolios (ADRs 0018, 0020).
     Every winning attempt independently proves the requested scope.
     Racing is deferred; seed successes are never assembled across runs.
4. **Benchmarks.**
   - **Done:** cold, warm, edit, agent-loop and history-replay workloads, all
     local-only (ADR 0013).
5. **Corpus.**
   - **Done:** fast and standard tiers, plus a secondary Verus pin and
     `docs/compatibility.md`.
   - **Done:** the exclusive, strict `rust_verify_test` deep suite and
     optional Singular source-build validation (ADRs 0019, 0021).
6. **CI.**
   - **Done:** a container image, with six fast per-PR checks only
     (ADR 0013).

### Measured: warm solver pools would not pay off (2026-10-08)

The charter lists warm solver pools. We measured their value before building
them, on a vstd run on macOS arm64:

| Run | Wall time | Rust front end | Verus verify phase (CPU, all threads) | Solver (all threads) |
|---|---:|---:|---:|---:|
| Plain Verus | 18.9 s | 5.7 s | 57.9 s | 13.0 s |
| sail, warm | 17.1 s | 5.6 s | 50.3 s | 3.5 s |

- **What's left after a warm cache is Verus, not the solver.** Most of the
  remaining time is Verus's own work: the Rust front end, VIR, and AIR and
  SMT generation.
- **The Linux fast-tier warm run** started a solver for only 26 of 1,526
  sessions.

Across a whole warm run, then, pools could only shorten a small
solver-side remainder. Edit loops agree.

- **Rescue L11.** It measured warm sessions per missed bucket, on the cycle's
  critical path, L11 (rescue lane report, tag `archive/rescue-m7-98f6292`).
  - A cycle misses 1–2 buckets. Setup costs 21–82 ms per bucket at the
    median.
  - Exact-prefix sessions would save at most 9–19 ms per human-eval-verus
    cycle (about 0.2%) and 107 ms per vest cycle (0.2%).
  - History-tolerant sessions would save about the same.
- **Proposal.** ADR 0040 (proposed) defers both behind the levers below.

The levers that matter now:
- **The warm Verus frontend** (the charter roadmap item).
  - The [larger-project profile](reports/2026-10-16-warm-frontend.md) is recorded.
    A read-only pruning candidate passes a bounded native A/B query comparison;
    its overall edit gain is inconclusive and runtime qualification is pending.
    The raw memo key misses ordinary edits,
    so persistent reuse remains unimplemented (proposed ADR 0052).
    A disabled observer on combined `8a373f81` builds, but the first native
    capture refuses truncated SST. The single bounded capacity revision
    completes the selected captures; complete-key and diagnostic boundaries
    remain unqualified. Lossless analysis finds packed span-position changes
    across edits and an unchanged IronKV external-function order reversal.
    A dependency-patcher no-op means original project macros were used;
    corrected full-source/actual-resolution project captures await native
    execution. The Sail research harness passes its native workspace gate and
    all command/capture/analysis controls; this does not qualify frontend reuse.
  - The earlier M8.0 gate failed on the reference host. Unchanged-bucket work is
    9–12% of the median edit cycle (gate: 25%), so the hermetic bucket memo
    (M8.1) does not proceed.
  - With 7 Verus threads it is 18–27%.
  - Cheaper items come first: rustc incremental on the EXEC pass (9.6% /
    12.6%), Cargo bypass (≤9% / 7%) and Sail overhead (11% / 7%).
    Rustc incremental is already on, for plain `cargo verus` and Sail alike;
    a separate cache for Verus's verify pass measured no gain, so that item
    is closed (L23 rescue lane report, tag `archive/rescue-m7-98f6292`).
  - Repair functions share one Cargo target, so a new function's first
    repair no longer verifies vstd (HumanEval 40 s → 6.5–8.9 s, Vest 61 s
    → 30 s; L23).
  - These figures come from the M8.0 stage profile, L18 (rescue lane report, tag `archive/rescue-m7-98f6292`).
  - The two vstd re-verification defects it found are resolved. The passing
    run after a failure no longer verifies vstd again (32 s → 5.4 s on
    HumanEval, 50 s → 24.4 s on Vest). The engine defect came from the
    dropped VI-0005 fork fix and does not occur at the current pin
    (L19 rescue lane report, tag `archive/rescue-m7-98f6292`).
- **Determinism (VI-0001, VI-0015),** to raise hit rates. In edit loops a
  reordered bucket costs about 2.7 s per affected human-eval-verus cycle.
  The fork's fixes (pinned since `13a1176c`, including `af212563` for
  `cargo verus`; `73dea021` adds VI-0017 and VI-0007) now reach default mode
  too ([ADR 0043](decisions/0043-default-mode-runs-the-fork.md),
  [record](validation/fork-default.md),
  [determinism record](validation/determinism.md));
- **Fill cost on slow filesystems:** write-behind stores in the daemon.

## After 0.1.0

Planned for releases after 0.1.0. The pre-verified vstd and managed-by-default
work in flight for 0.1.0 is not listed here; nor are `sail new` and output
schema v2, which the owner moved into 0.1.0 (#148, #149).

**Order (owner direction, 2026-10-15).** "We're doing the core verus and perf
features first and then backburnering the growth to multibackends and prep for
oyster and such." This supersedes the parallel tracks of
[question 27](open-questions.md): M10 waits until M11 completes.

1. **M11: Verus core and performance** (milestone *M11: Verus core and
   performance*; ordered backlog in
   [#168](https://github.com/Chelis-Lang/sail/issues/168)). Sail's reason to
   exist for today's users, judged against the repeated walkthrough
   ([roadmap review](reports/2026-10-13-roadmap-review.md) sections B and D)
   and the [real-project measurements](reports/2026-10-15-real-project-performance.md).
   In pickup order:
   1. The verismo soundness check, #160.
   2. A miss costs only the changed check, #158 (the largest edit-loop lever).
      Fork/live-prefix investigation is recorded in the
      [report](reports/2026-10-16-solver-snapshots.md). Strict raw acceptance
      fails. The named timing-only prototype preserves its earlier resource
      refusals; a separate sole-wait4 owner passes both fixed solver traces.
      Preparation remains real work and Anvil saves no total CPU. Actual edit
      workflow/default acceptance is unbuilt and unselected; #158 stays open.
   3. vstd pre-verified in the release archive, #161.
   4. Remaining managed overhead against the removed pass-through, #156:
      cheaper checks qualify against rc.5 on unchanged vest/demo runs
      (1.01/0.39 s to 0.54/0.21 s); the final vest edit gain is
      inconclusive. See the [validation record](validation/managed-overhead.md).
      A cheaper per-function report in the fork remains a separate lever.
   5. Cache robustness to incidental edits: stable crate-wide numbering in
      the fork, [#118](https://github.com/Chelis-Lang/sail/issues/118); any
      key change is an ADR under charter rules 5 and 6. The only crate-wide
      counter found is the closure-type number (VI-0019); its fork fix
      (`8d03ab54`, [ADR 0049](decisions/0049-stable-closure-names.md),
      accepted with the matching runtime pin) is independently reviewed and measured in isolation; the combined runtime
      now has a qualified full crate-qualified runtime selected by the matching
      pin change, with a measured 27.6% helper-edit gain and 1.2% body-edit
      regression against the previous runtime. Its
      [qualification record](validation/closure-full-identity.md) records
      completed runtime gates and exact platform/scope limits. The three-project
      [incidental-edit catalogue](validation/incidental-edits.md) is prepared;
      its final-runtime measurements and exact solver-work counts are pending.
   6. B4, instant revert: receipts beyond the latest revision, #163.
   7. The warm Verus frontend (M8), #162: the M8.0 gate failed and the
      hermetic bucket memo did not proceed, but the 5–38 s all-hits floor on
      real projects caps every other lever. A narrow Cargo-selection correction
      measures the first repair transition at 67.87 to 32.73 seconds on the same
      cc7 runtime; later warm repair remains about 33 seconds. Current-source
      STANDARD/final qualification is pending
      ([record](validation/managed-selection-family.md));
      this does not accept M8 or close #162.
   8. Jobs, watch and MCP fall back like `sail verify` (`SAIL3112`), #165:
      implemented; native gates and full fast/standard pass; required PR
      checks are enforced separately ([validation](validation/fallback-parity.md)).
   9. B2, single-file `verus file.rs` projects, #164.
   10. deep-rvt against the fork, #124: exact fork and retained stock
       control arms implemented; native acceptance pending
       ([guide](deep-verus-tests.md)).
   11. Reliability: #102, #103, #112, #76, #77, #78, #166, #167.

   B1 (a verified vstd shared across projects and worktrees, beyond the
   0.1.0 pre-verified vstd) is not yet ordered.
2. **M10: Verification runtime (backburnered until M11 completes)**
   ([scope](scope/verification-runtime.md), milestone *M10: Verification
   runtime*, umbrella [#146](https://github.com/Chelis-Lang/sail/issues/146)).
   One cached runtime and one ledger behind annotated Rust, raw SMT-LIB and
   TLA+ jobs (charter appendix entry 5). When it starts, the interface
   specifications (#127, #128, #129) come first, then the SMT-LIB service
   (#137), since Oyster depends on it first; alongside it fan out the
   verifier backends (#130–#136), certificates (#138), TLA+ jobs through
   D.I.V.E. (#139), the Verus-aware mutator (#140) and library mode (#144);
   then the mutation gate (#141), cross-backend selection (#142) and the
   assurance view (#143). Dafny reach is an evaluation (#145). Every backend
   passes conformance before any acceleration, the Verus path keeps its
   performance and parity, and a backend is forked only with a measured gain
   proven before Sail ships any use of the fork.

The charter's original roadmap items map onto this as follows:

| Charter roadmap item | Where it is now |
|---|---|
| Multiple SMT backends (cvc5, portable queries) | SMT-LIB service, #137; cross-backend portfolios, #142 |
| Alethe and Carcara certificates | Proof certificates, #138 |
| Other verifiers | The M10 backends (#130–#136); Dafny as an evaluation, #145 |
| A warm Verus frontend | M11 (M8), #162 |
| A Rust pre-solver (M9) | Closed: measured no-go for edit-loop latency (ADR 0038) |
| Unsat-core pruning, instantiation replay | Designed for, not scheduled |
| Warm solver sessions, stack-key mode | Not built now (ADR 0040); deferred (ADR 0008 update) |
