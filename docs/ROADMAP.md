# Roadmap

Current state and next work. Evidence lives in the linked validation records
and reports; this page holds only status and gates. Status words follow
AGENTS.md: proposed, built, implemented, qualified, selected, accepted.

## Goal

The first public release must be clearly faster than plain `cargo verus
verify` (stock Verus) in normal use. No scenario may be materially slower
than stock, cold runs included: at most 5% slower, or 0.1 s on sub-second
rows ([question 32](open-questions.md)).
Verdicts, diagnostics and exit codes stay identical to stock. Sail's Verus
fork is an accepted way to get speed (ADR 0039, ADR 0043). The release is a
CLI. M10 (multi-backend verification runtime, Oyster) stays backburnered
(owner direction 2026-10-15 and 2026-10-19).

Milestones: M11 is *Verus core and performance*, tracked by the GitHub
milestone and its ordered backlog
[#168](https://github.com/Chelis-Lang/sail/issues/168). The performance
program below is #168's order; new performance issues are filed under M11
and listed in #168. M10 is the multi-backend verification runtime (#146).

### Current standing against stock Verus

Ratio = stock time ÷ Sail time; above 1 means Sail is faster. Linux, native
host (dedicated EC2), three-round medians unless noted. **No row has been
measured on current main `d1499eff` or on the 0.1.0 candidate.** Each row
names the Sail build it measured; cohorts differ, so rows cannot be combined.

| Scenario | Stock | Sail (latest measured) | Ratio | Meets goal | Sail build measured | Evidence |
|---|---:|---:|---:|---|---|---|
| Cold first run, Vest | 73.93 s | 85.52 s | 0.86× | **No** (+16%) | `e0d14d75` (#225; main minus #226) | [instant revert, optional-plan screen](validation/instant-revert.md#unchanged-run-optional-plan-candidate-156) |
| Cold first run, small project | 32.22 s | 36.58 s | 0.88× | **No** (+14%) | `c3e64d21`, pre-#224, diagnostic figures from a FAIL cohort | [instant revert, ordinary public candidate](validation/instant-revert.md#ordinary-public-candidate-and-measured-scope-successor) |
| Cold first run, five real projects | unmeasured | unmeasured | gap | Unknown (likely failing: four projects use path or pinned Verus and get nothing from shipped vstd) | — | lever 1.1 |
| Fresh project (`sail new` + verify) | 34.36 s | 32.57 s | 1.05× | Unknown on current main (likely below parity after #217) | `19034f8d`, pre-#217 containment candidate (2026-10-16), original pinned runtime | [store containment](validation/dependency-store-fail-closed.md); #217 later added +11.26% here ([solver identity](validation/solver-identity.md)) |
| Fresh project, shipped vstd (Cargo 1.98 only) | 31.78 s | 14.23 s | 2.23× | Not qualified (candidate component screen only) | `356b7d0f` (runtime `e5365c05`, before the `cbbcdf44` join) | no record yet (PR #227 body); setup + new + lock + verify, nine roots, one host |
| New worktree, Vest | unmeasured | unmeasured | gap | Unknown | — | v1 history only ([store record](validation/dependency-store.md)); boundary path is Sail vs Sail ([record](validation/vstd-compile-boundary.md)) |
| Small edit | 0.70 s | 0.95 s | 0.74× | **No** (+0.24 s, sub-second) | `c3e64d21`, diagnostic FAIL cohort | [instant revert](validation/instant-revert.md#ordinary-public-candidate-and-measured-scope-successor) |
| Vest edit | 41.07 s | 30.43 s | 1.35× | Yes (requalify on `cbbcdf44`) | `e0d14d75` (#225) | [optional-plan screen](validation/instant-revert.md#unchanged-run-optional-plan-candidate-156) |
| Unchanged rerun | 0.10 s (small), 0.23 s (Vest) | 0.19 s, 0.54 s | 0.53×, 0.42× | **No** (sub-second; Sail returns a complete report, stock a Cargo no-op) | small `c3e64d21`; Vest `e0d14d75` | same two records |
| One-module edit, five real projects | 9.9–124 s | 7.4–104 s | 1.19–2.25× (Anvil lowest) | Not current product | removed pass-through mode on stock Verus, 2026-10-15; not current managed behaviour | [real-project report](reports/2026-10-15-real-project-performance.md) (three rounds, Anvil two) |
| Spec-wide edit, five real projects | 9.9–124 s | 9.9–124.3 s | 1.00–1.60× | Not current product; parity on Anvil, IronKV, CapybaraKV then | same | same |
| Revert to verified content, Vest | 41.12 s | 2.80 s | 14.7× | Yes on Linux public alloc/std profile (requalify on `cbbcdf44`) | `e0d14d75` (#225) | [instant revert](validation/instant-revert.md) |
| Revert, small project | 0.71 s | 0.92 s | 0.77× | **No** (sub-second) | `c3e64d21`, diagnostic FAIL cohort | same |
| macOS, any scenario | unmeasured | unmeasured | gap | Unknown | — | historical cold was 15–18% slower ([performance](performance.md)) |

Corpus: Vest, Anvil, IronKV, CapybaraKV and Verismo are the real-project
corpus (`corpus/`; [real-project report](reports/2026-10-15-real-project-performance.md)).
"Small project" is the `sail new` template.

The all-hits floor (an edit with every solver query cached) caps the
edit-loop speedup at 1.45× (Verismo) to 3.25× (Anvil); measured in the
2026-10-15 pass-through mode; see track 2.

Gaps: cold runs lose to stock on every measured row; the fresh-project win
is a candidate screen on a pre-join runtime; current main, the joined
candidate, the real-project cold row, new worktrees and macOS have no
current stock A/B.

## 0.1.0

Nothing is published and repository visibility is unchanged. The 0.1.0
scope is PR #227's joined candidate (owner decision 2026-10-19,
[question 29](open-questions.md)); rc.5's v1 store stays quarantined
([ADR 0053](decisions/0053-disable-unqualified-native-store.md), #187).
Nothing goes to crates.io until the owner's rollout; the release is open
source under the MIT License with macOS and Linux binaries on GitHub
releases.

**Scope** (selected for 0.1.0): PR #227, draft, built and not qualified.
Scope commit `d827c407`; the PR head is now `7f1301a0` (`d827c407` plus
follow-ups, including `eb816fb8`, which binds the Linux runtime manifest to
the engine pin).

- Built: verified vstd shipped in release archives as a compiler-boundary
  proof bundle (#161, [ADR 0048](decisions/0048-shipped-dependency-build.md)).
  Proc-macro and build-script crates still compile fresh. Sail serves the
  bundle only after *admission*: exact source, compiler, runtime,
  linked-byte and release-index match; any mismatch verifies vstd normally.
- Built: fork runtime pin `cbbcdf44` (includes fork PR #8, stable
  trait-function spellings).
- Built: *proof coexistence*, so shipped and locally verified vstd proofs
  can sit in one cache without conflict.
- Selected for 0.1.0 (not built): Sail will select the pinned Cargo 1.98.1
  for managed builds so the shipped bundle hits regardless of the user's
  default Cargo.

| Gate | Status | Evidence | Next |
|---|---|---|---|
| Archive build produces the vstd bundle (producer), joined head | Pending (joined-source qualification not recorded) | [releasing](releasing.md) on PR head: Linux producer/clean-consumer acceptance pending | Record on `7f1301a0` |
| First run on a fresh Linux machine uses the bundle | Not qualified (component pass on `356b7d0f`; last enclosing attempt FAIL) | record lands with PR #227 ([boundary record](validation/vstd-compile-boundary.md)) | Rerun with stock primed, on the joined head |
| Fresh-project first use, joined head | Not measured | — | Re-measure stock vs Sail on `7f1301a0` (`cbbcdf44` runtime) |
| Instant revert and edit rows on `cbbcdf44` | Not qualified (manifest authority names `cbbcdf44`; ordinary-reuse qualification pending) | PR-head ROADMAP | Requalify Vest edit and A/B/A |
| Vest and public-profile A/B/A screens | Pending | — | Queued native run |
| Fast and standard conformance tiers ([testing](testing.md)) | Pending on the joined head | — | Run on the joined head |
| Release-candidate validation | Pending | rc.5 precedent ([release candidate](validation/release-candidate.md)) | Gate, e2e, every conformance tier incl. deep, one deep-rvt round, clean Linux and macOS README walkthrough including Claude Code |
| #222 applicability | Unknown | #222 (boundary path can publish vstd after a native input is replaced and restored) | Confirm the protected native-input fix is in the archive path, or show the archive path is unaffected |
| Changed Rustup roots, Linux | Pending (Mac components only) | PR #227 body | One changed-root Linux consumer |
| Fresh-machine default first use with pinned Cargo | Pending | — | After the Cargo selection is built |
| Fallback-path first runs | Unmeasured | [question 30](open-questions.md) (`SAIL3112` runs reuse no receipts) | First use with `LD_LIBRARY_PATH` set and with `--target-dir`: record bundle hit/miss and wall vs stock; the miss note must fire here too |
| macOS components | Built; no Mac performance claim | PR #227 body; no record yet | — |
| macOS ordinary retained reuse (instant revert) | Not qualified: retained reuse selects only the Linux manifest (`LINUX_MANIFEST`); macOS verifies the returned revision again | [instant revert](validation/instant-revert.md) (Linux only) | A `MACOS_MANIFEST` authority with its own pin test, then the Vest A → B → A qualification on a Mac host |
| e2e scripts on macOS | Not portable: `ci/test-output.sh` and others use GNU `sed -i` and `timeout`, which macOS lacks | — | Make the scripts portable, or document GNU coreutils and sed as a macOS prerequisite |
| macOS first-use and cold A/B | Not scheduled | — | Stock vs Sail on the joined head, rotated rounds (about 1 Mac day) |
| Pinned Cargo 1.98.1 selection | Selected, in progress (#233, [question 33](open-questions.md)) | — | ADR, stock comparisons on the same toolchain, fresh-machine test. Without it a newer default Cargo misses silently (about 36 s) |
| Loud miss note | Proposed (#238) | — | One `note[SAILnnnn]` line on a bundle miss (Cargo, vstd, toolchain mismatch, fallback paths) |
| Runtime base current with upstream Verus | In progress: fork `64baccbb` on `0.2026.10.18.7d82994` (Rust 1.99.1), which fixes the reachable standard-library label difference (VI-0031: span-string remap, stock `dbg!` location); Linux runtime published and pinned, macOS runtime pending. On `64baccbb`: Linux shipped vstd bundle re-recorded, byte-identical to stock on every probe with and without `rust-src`, first use 31.3 → 16.2 s; native shape witness passes; Linux instant revert requalified (Vest cycle 150.7 → 112.7 s). The five-case admission witness passed on `9c0ca7e7` (#249) | [compatibility](compatibility.md#upgrade-to-0202610187d82994), [boundary record](validation/vstd-compile-boundary.md#verus-7d82994-bundle-on-runtime-64baccbb) | macOS runtime and bundle, admission witness on `64baccbb`, conformance tiers, `deep-rvt` |
| **Performance matrix** | **Fails (cold rows); most rows unmeasured on the candidate** | standing table | Every row, Linux and macOS, three rotated rounds, identical verdicts; no row materially slower than stock. Needs track 1 |
| Docs and CLI trim | Pending | — | [releasing](releasing.md) "no prebuilt vstd"; [performance](performance.md) "1–3 s slower" cold claim and "a warm team cache does not change this" (never measured on real projects); README toolchain advice; research flags off the user surface |
| Owner review and publication | Not started | — | — |

**Merged on main for 0.1.0** (implemented; release qualification is the
gate table above): MCP in Claude Code, agent-sized replies, honest counts,
macOS managed reuse, install and first run, release builds with the engine
runtime, fork runtime releases, the performance page, managed target
cleanup, `LICENSE`/`CONTRIBUTING.md`, `sail new`, output schema v2 (ADR
0044), managed output order and script facts
([record](validation/managed-output-order.md)), and managed verification by
default ([ADR 0046](decisions/0046-managed-verification-by-default.md);
implemented, awaiting review). Issue lists: [CHANGELOG](../CHANGELOG.md) and
the GitHub milestones. Out of 0.1.0: single-file projects (#164).

## Performance program

Ordered by release-goal blocking first (no scenario slower than stock),
then value × tractability. Track 1 is the release blocker; track 2 is the
largest edit-loop prize and the only one that moves all five projects.
"Fork" means the lever changes Sail's Verus. Prizes are measured unless
marked "estimate"; "unmeasured" means no evidence exists.
New performance issues are filed under M11 and ordered in #168.

### 1. Cold runs and pre-caching (release blocker)

Where cold time goes: a fresh small project compiles its proc-macro and
build-script crates (proc-macro2, quote, verus_syn, Verus macro crates),
then verifies vstd (2,061 checks, 366 solver sessions), then the user crate.
Cold was 0.97–1.01× on the 2026-10-15 corpus (removed pass-through mode)
and is 0.86–0.88× on the latest measured managed builds; the regression is
unattributed (lever 1.1). On Vest, overhead fixes (1.1, 1.3, 1.4, EXEC skip)
can reach parity at best; only vstd coverage for Vest's pin (1.7–1.9)
turns cold into a win, because Vest pins vstd 2026-09-06 and misses the
shipped bundle.

| # | Lever | Prize | Status | Next experiment (cost) | Fork | Issue |
|---|---|---|---|---|---|---|
| 1.1 | Cold-gap attribution, re-baseline and owner | Required for the goal: about 12 s on Vest cold, 4 s on small | Proposed | Five rotated rounds of stock, current main, and main with handshake counts and `SAIL_JOB_EVENTS` phase timings: Vest cold, small cold, fresh project, one cold pair per corpus project, plus a no-EXEC arm (0.5–1 native-host day). Hypotheses to attribute: managed-by-default (own target, receipts, input checks), solver identity (#217), retention (#224). Same session: run Vest on the boundary and archive paths and record hit or miss (0.5 day). Target: no worse than stock | No | #232 |
| 1.2 | Shipped verified vstd + pinned Cargo 1.98.1 | Fresh project 31.78 → 14.23 s vs stock (2.23×; pre-join candidate screen) | Bundle built (PR #227); Cargo selection selected, not built | 0.1.0 gate table | No | #161, PR #227 |
| 1.2b | Pinned toolchain and rust-src fetched at install, in parallel with the archive download | How the 1.2 pin arrives; unmeasured share of 14.23 s | Proposed | rust-src split is part of 1.5's profile; then fetch during install (days) | No | #233 |
| 1.3 | Read the solver binary once per run (or a content-addressed private copy) | 2–3.3 s per fresh project, 0.5–0.9 s per Vest edit | Proposed; #219 recovered 1–3% | Per-run identity reuse (1–2 days); a private copy needs an ADR on same-path replacement | No | #156 |
| 1.4 | Take instant-revert retention off the cold path | About 4.6 s on Vest cold (76.05 → 80.68 s when added) plus about 1.6 s on changed B; the unchanged-run cost is largely recovered by #225 | Proposed; no branch exists | Skip retention on a project's first run, or write it after the result (days) | No | #163 |
| 1.5 | Profile the 9.3–14.2 s residual first use | Unmeasured, likely 0–3 s; fixed waits alone could be up to about 4 s per first run; decides 1.10 | Proposed | Control: stock `cargo build --timings` of only the macro closure (never timed). Then per-phase timestamps: native compile, rust-src setup, admission hashing (662 policy files, 128 source maps), fixed 2 s waits (0.5 day). Candidate cuts: per-machine content-memoized policy-archive attestation, readiness signals in place of the fixed arrival and graph-capture waits, parallel hashing (1–3 days) | No | #234 |
| 1.6 | Overlap daemon start and solver-identity hashing with Cargo's native compile | Unmeasured; pure cold cut, needs no think time | Proposed | `SAIL_JOB_EVENTS` timeline of a fresh project to show the serial section, then overlap (1–2 days) | No | #235 |
| 1.7 | Ship vstd SMT replies as signed shared-tier entries ([ADR 0017](decisions/0017-shared-tier-with-signed-entries.md)) for the pinned and last N crates.io vstd versions | Unmeasured; covers bundle misses (other vstd such as Vest's pin, `SAIL3112` fallbacks, Cargo mismatch), where the solver cache stays on ([question 30](open-questions.md)). Two cold controls already match all 366 literal input streams ([boundary record](validation/vstd-compile-boundary.md)). Still pays the vstd frontend | Proposed | Fill a directory store from one vstd verify, seed a fresh machine, time a deliberate bundle miss (different Cargo; Vest's pinned vstd) with and without it; record hit rate and wall saved (0.5–1 day, no code) | No | #236 |
| 1.8 | Compiler-boundary local reuse as the default fallback | 39.84 → 9.34 s per new demo project (Sail vs Sail); worktree and Vest unmeasured, and Vest's vstd pin likely misses | Built behind a research flag; #222 fix built, qualification pending | Land the #222 fix; Vest hit/miss from 1.1; then qualify as default | No | #161, #222 |
| 1.9 | More vstd versions (Vest pins 2026-09-06) | About 26 s per Vest-class worktree; corpus gain about 0 today | Proposed | Can start from 1.1's Vest hit/miss: fill-once local store keyed by `Cargo.lock` checksum (about 1 week), then published bundles (about 1 week) | No | #237 |
| 1.10 | Release-built macro crates as pinned tools | Plausibly 1.5–3 s fresh project (estimate) for projects matching the runtime's pinned vstd and macros; v1's 1.28 s is the upper bound. At opt-level 3 also 0.6–2 s per warm edit (one sample) | Proposed; blocked on an owner parity decision under ADR 0053 (#187: a broken user `cc` would no longer break Sail) | After 1.5: re-run the opt-level 3 macro A/B with ≥3 cold and ≥6 warm samples per arm on Vest and IronKV (0.5 day); then serve exact-match macro units from the archive, bound by digest (4–6 days). On Mac, requires qualifying the VI-0026 stable `@rpath` install-name fix (branch `perf/vstd-macos-repro`) | Yes | #239 |
| 1.11 | Seed cold runs from a team or CI cache | Cold minus most solver time (Anvil solver about 85.7 s of 124 s), still paying cold compile and frontend; unmeasured | Infrastructure implemented (ADR 0017, 0025); never measured on real projects | One seeded fresh-clone run on Anvil and CapybaraKV (about 1 day, no code); baseline from 1.1 | No | #240 |
| 1.12 | Background prebuild at `sail new` or first sight (the ordinary managed Cargo build, run early) | Hides up to the residual first use with think time; 0 without | Proposed; independent of 1.8 | Time `sail new` → first verify with a realistic agent think-time trace (days). Background pre-verification on a miss waits for 1.8 to be default-safe | No | #241 |
| 1.13 | User CI cache recipe | Unmeasured; plausibly the all-hits floor on unchanged code (charter success criterion) | Proposed | One measured GitHub Actions example persisting the Sail cache or reading a team store (days) | No | #242 |
| 1.14 | Bulk prefetch from the team store | Unmeasured | Proposed | Only after 1.11 shows remote latency matters | No | #243 |

Also a cold contributor: EXEC-pass skip (track 2 step 2) saves about 3 s on
Vest cold as well as every edit, about a quarter of the Vest cold gap.

Not pursued: v1 unit-copy store (blocked; unsound as designed, #187, ADR
0053); proc macros at opt-level 3 built locally (cold Vest about 73 → 85 s,
one sample; resampled inside 1.10).

### 2. Frontend floor (#162)

The all-hits floor (IronKV 5.6 s to Anvil 38.2 s) caps every edit-loop
lever and is the only lever that moves all five corpus projects. Status:
the per-bucket memo key is blocked; the cheap steps are untried
([warm-frontend report](reports/2026-10-16-warm-frontend.md),
[validation](validation/warm-frontend.md)).

| Step | Prize | Status | Next experiment (cost) | Fork | Issue |
|---|---|---|---|---|---|
| 1. Re-profile frontend stages at 31 and 8 threads | Decides step 3 | Proposed | Fork timers branch `m11-8/frontend-timers` (1 day plus one native-host day) | Yes | #162 |
| 2. Batch A/B of cheap cuts: EXEC-pass skip or metadata-only (3.04 s of a Vest cycle; also a cold cut), the unmeasured pruning branches `perf/prune-raw-lookups`, `research/prune-shared-indexes` and `perf/prune-traversal-memo`, and small lowering (SCC/AIR/autospec) branches | Estimated 10–20% off the floor | Proposed | One batched screen (2–3 days) | Yes | #162 |
| 3. Position-independent VIR key and per-bucket memo ([ADR 0052](decisions/0052-warm-frontend-reuse.md), proposed) | Estimated 25–45% off the floor; Vest all-hits 1.6× → about 2.1–2.4× | Blocked: raw key matches 0/200 Vest buckets on any edit (span offsets, unstable `external_fns` order) | Commit only if step 1 shows lowering ≥25% of the floor (3–5 weeks) | Yes | #162 |
| Cargo bypass when only target sources change | ≤1.72 s (7.1%) of a Vest cycle, upper bound (rescue stage profile¹) | Untried; constrained by ADR 0041 | After step 1 | No | #245 |

Terms: VIR is Verus's intermediate representation; a bucket is the unit
Verus sends to one solver session; spinoff runs a function in its own solver
process; rlimit is the solver's resource limit. See the warm-frontend report.
Merged: the selection-family fix (#220, [record](validation/managed-selection-family.md)).

### 3. Solver work on misses (#158 and throughput)

The large solver prizes are mostly Anvil: one `resource_match` bucket (9
checks, about 66 s) is re-solved on any edit, and solver time is 85.7 s of
Anvil's 124 s cold run. Verus dispatches buckets FIFO with spinoff buckets
last; cost ordering and heavy-bucket splitting were never tried. Estimates
below come from the [real-project report](reports/2026-10-15-real-project-performance.md)
("Where the time goes" and the miss-cost table). Kill rule for the whole
track: stop if the heavy-bucket experiment recovers under 10 s.

| Lever | Prize (estimate) | Status | Next experiment (cost) | Fork | Issue |
|---|---|---|---|---|---|
| Heavy-bucket experiment: `#[verifier::spinoff_prover]` on Anvil's 1–3 heaviest functions (measurement only) | Anvil body edits 88–104 → about 44–50 s best case; 0 if one query holds the cost | Proposed; answers #158 and throughput together | Anvil cold timeline plus per-check split, then the annotation A/B (1–2 days) | No | #158 |
| Cost-ordered (longest-first) bucket dispatch and automatic spinoff of heavy functions | Anvil cold and spec-wide 0–35%, 0–5% elsewhere (jointly) | Deferred until the experiment | Order from recorded cost hints (50–100 lines in `verifier.rs`; queries stay byte-identical). Auto-spinoff changes rlimits, so an opt-in mode, ADR and divergence entry | Yes | #244 |
| Edit-triggered live-prefix preparation | Anvil edits 88–104 → about 45–55 s with ≥54 s think time; 0 without | Blocked on [question 31](open-questions.md) and scheduling ([ADR 0051](decisions/0051-exact-solver-snapshots.md), proposed) | Fallback if the experiment fails: decide question 31 (about 1 day), then 1–2 weeks | No | #158 |

[Solver-snapshot report](reports/2026-10-16-solver-snapshots.md),
[validation](validation/solver-snapshots.md).

### 4. Incidental-edit robustness (#118)

Keeps edits that change nothing logically at the floor. Merged (qualified
per record): closure identity (#221, [record](validation/closure-full-identity.md)),
spec-parameter names (#223, [record](validation/spec-parameter-names.md)).

| Item | Prize | Status | Next step (cost) | Fork | Issue |
|---|---|---|---|---|---|
| Trait function spellings | Vest empty-impl edit 45.09 → 33.96 s | Built (fork PR #8 draft; runtime `cbbcdf44` published as a prerelease); part of the 0.1.0 scope | Qualified with the joined candidate | Yes | #118 |
| 15-edit catalogue | Finds remaining leaks; mostly insurance | Prepared, never timed ([catalogue](validation/incidental-edits.md)) | One machine, 5–7 h, no new code, plus one Anvil row | No | #118 |

### 5. Managed overhead (#156)

Small-project edits and unchanged runs are sub-second but slower than stock
(standing table). Merged (qualified per record): #186, #194, #219, #225,
#226 ([managed overhead](validation/managed-overhead.md)); solver identity
(#217) added cost back ([record](validation/solver-identity.md)). Rule: skip
Sail-side items under 100 ms unless the sub-second threshold decision makes
them gating.

| Item | Prize | Status | Next experiment (cost) | Fork | Issue |
|---|---|---|---|---|---|
| Per-run solver identity | See 1.3 | Proposed | Lever 1.3 | No | #156 |
| Attribute the Vest unchanged gap (0.54 vs 0.23 s) | Up to 0.31 s per unchanged Vest run | Proposed | `SAIL_JOB_EVENTS` breakdown, tied to 1.3 and 1.4 (0.5 day) | No | #156 |
| Fork attribution sidecar | Up to 0.8 s on JSON/MCP Vest runs only | Unfunded unless MCP latency becomes a priority | — | Yes | #156 |

### 6. Instant revert breadth (#163)

Merged for the Linux public alloc/std profile (#224,
[record](validation/instant-revert.md), [guide](instant-revert.md));
requalification on `cbbcdf44` is a 0.1.0 gate. Remaining: macOS and other
dependency profiles, and lever 1.4 (its cold and cache cost, about 519 MB).

## Owner and lead decisions pending

| Decision | Who | Blocks |
|---|---|---|
| Fork base: rebase onto the current upstream Verus release before qualifying ([question 34](open-questions.md)) | Owner | 0.1.0 qualification |
| ADR for pinned Cargo selection: stock comparisons run with the same toolchain ([question 33](open-questions.md), decided) | Lead | 1.2 |
| Prebuilt macro crates under ADR 0053 parity ([question 35](open-questions.md)) | Owner | 1.10 |
| Private solver copy on same-path replacement (ADR) | Lead | 1.3 variant |
| Question 31, continuation timing oracle | Owner | Live-prefix preparation |
| ADR 0048 status: research path → release default | Lead | 0.1.0 |
| ADRs 0051, 0052 still proposed | Lead | Track 2 step 3, track 3 |
| ADR 0046 managed by default: implemented, awaiting review | Lead | 0.1.0 |
| M7 milestone acceptance | Lead review | History only |

## Measured and closed

| Idea | Evidence |
|---|---|
| Rust pre-solver (M9, a planned Rust-side pre-solving milestone) | Covers 0.1% of solve time (ADR 0038; rescue lane¹) |
| Warm solver pools | A warm fast-tier run starts a solver for 26 of 1,526 sessions |
| Warm exact-prefix sessions | Save 9–19 ms per HumanEval cycle, 107 ms per Vest cycle ([ADR 0040](decisions/0040-warm-solver-sessions.md), do not build) |
| vstd-preloaded spare process | 0.19 s on Vest (0.8%; rescue stage profile¹) |
| Separate verify-pass cache | No gain; rustc incremental already on (rescue lane¹) |
| Stack-only keys as default | Identical answers but rlimit deltas up to 17× (ADR 0008); default stays full-prefix ([question 8](open-questions.md)) |
| Stock `-V spinoff-all` | Floor 1.7–1.8× worse on CapybaraKV and Vest (a little on IronKV), cold 1.4–1.8× slower, one IronKV verdict change ([report](reports/2026-10-15-real-project-performance.md)) |
| Parked Z3 fork snapshots | Not state-identical: Z3 atfork timer state, `num-allocs` differ ([report](reports/2026-10-16-solver-snapshots.md)) |
| Fork `--time` per-function report | Within noise, 25.77 vs 25.89 s (ADR 0047) |
| Read-only pruning traversal | −0.7% Vest, +0.3% IronKV, inconclusive ([validation](validation/warm-frontend.md)) |
| CI image slimming (#198) | 9% smaller, no latency gain ([record](validation/ci-image-startup.md)) |
| `sail verify --incremental` | Recompiled about 18 crates every run (#157, fixed by #170) |

Never run and not funded: Z3 version or option sweeps (stock refuses other
Z3 versions), unsat-core pruning. Reopened: "a warm team cache does not help
cold runs" was never measured on real projects (lever 1.11).

¹ Rescue-lane reports live in tag `archive/rescue-m7-98f6292`
(`docs/reports/rescue/`), cited by ADR 0038.

## Delivered history

| Milestone | State | Evidence |
|---|---|---|
| M0–M4 foundation: workspace, survey, pass-through shim, corpus, conformance, baseline | Done 2026-10-08 | [bootstrap report](reports/2026-10-08-bootstrap.md) |
| M5 initial product | Done 2026-10-09 | [report](reports/2026-10-09-initial-product.md) |
| M6 live workspace verification, jobs/watch/MCP, HTTP team cache | Implemented and accepted locally | [report](reports/2026-10-10-live-workspace.md) |
| M7 proof engine and repair workflow | Every gate has native evidence; speed accepted at 2.14× HumanEval, 1.82× Vest vs M6 (ADR 0038 amendment); milestone acceptance awaits lead review | [M7 validation](validation/m7.md) |
| Fork as default runtime (ADR 0043) | Selected | [record](validation/fork-default.md) |
| M11 Verus core and performance | In progress; ordered backlog #168 | [milestone](https://github.com/Chelis-Lang/sail/issues/168), [managed cold start](managed-cold-start.md) |

Caching, CLI, agent ergonomics, benchmarks, corpus tiers and CI from the
charter's initial scope are done (ADRs 0009, 0011, 0013–0021, 0025).
The M11 reliability items in #168 are closed.

## Later

- **M10 verification runtime (backburnered).** One runtime and ledger for
  Rust, SMT-LIB and TLA+ jobs ([scope](scope/verification-runtime.md), #146).
  Interfaces (#127–#129), then the SMT-LIB service (#137), then backends
  (#130–#136), certificates (#138), D.I.V.E. TLA+ jobs (#139), mutation
  (#140, #141), selection (#142), assurance view (#143), library mode (#144),
  Dafny evaluation (#145). Every backend passes conformance before any
  acceleration.
- **Single-file projects** (#164).
- **CI image pull time** (#195): developer CI only, not a user claim.
- **History-tolerant serving and a named stack-only mode** (ADR 0040 point
  3, ADR 0008): opt-in divergence only; authorized in principle and
  deferred ([question 8](open-questions.md)).
- **Per-module `rust_to_vir` / GHOST typeck reuse:** the only route to
  halving the floor; uncharted compiler work, not counted on.
- **GHOST incremental build / render-once variants:** source possibly lost;
  check `archive/retired-ghost-incremental-build-source-829c328e` before
  treating them as recoverable; unmeasured.
- **Function-level speculation in `sail watch`:** provisional tier defined
  (ADR 0016) but never produced; shares the scheduling hook with live-prefix
  preparation.

| Charter roadmap item | Where it is now |
|---|---|
| Multiple SMT backends, portable queries | M10: #137, #142 |
| Alethe and Carcara certificates | M10: #138 |
| Other verifiers | M10: #130–#136; Dafny evaluation #145 |
| Warm Verus frontend | Performance program track 2, #162 |
| Rust pre-solver (M9) | Closed (ADR 0038) |
| Unsat-core pruning, instantiation replay | Not funded |
| Warm solver sessions, stack-key mode | Closed for default (ADR 0040, ADR 0008) |
