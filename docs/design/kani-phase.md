# The Kani phase

Status: proposed design for the phase after v0.3.1 and the D.I.V.E. symbolic pickup (milestone #17). The owner approved the plan on 2026-11-27, including forking Kani for deterministic output if measurement supports it. [ADR 0180](../decisions/0180-kani-phase-scope.md) records the decision. Nothing here is implemented beyond what [the roadmap](../ROADMAP.md) lists; every expected gain below is a prediction until its spike record exists.

This document covers first-class Kani support, where it sits against other backends and solvers, and when neutral spec frontends come back. Kani on Sail is a large performance opportunity with a narrower shape than "cache Kani as Sail caches Verus": it says where the gain is, where it is not, and what is measured before building.

Sources: this repository, the D.I.V.E. repository, the `feat/v030-kani-cli` and `spike/v030-kani` branches, the pilot record on [#135](https://github.com/Chelis-Lang/sail/issues/135), Kani 0.68.0 source and issues, and the Kani ASE 2026 (arXiv 2607.01504) and verify-rust-std NFM 2026 (arXiv 2606.17374) papers.

## 1. How Kani works

`cargo kani` builds the crate with a custom rustc backend (`kani-compiler`) that turns each `#[kani::proof]` harness into its own GOTO program, already sliced to the code that harness can reach. `goto-instrument` rewrites it (contracts, loop contracts, dropping unused functions), then CBMC symbolically executes it, bit-blasts it to one propositional formula and hands that to a SAT solver (CaDiCaL by default; Kissat and MiniSat selectable; Z3, cvc5 and Bitwuzla through a different SMT encoding).

Three facts drive everything below:

- **One independent CBMC process per harness.** No shared solver session, so nothing like the Z3 session-prefix constraint (ADR 0008) that makes a Verus edit re-solve the rest of the module.
- **Results are bounded.** A harness holds up to its unwind bound, unless it uses function contracts and loop invariants, which abstract loops and give unbounded results under stated assumptions.
- **No cross-run result cache exists.** Cargo skips recompiling unchanged crates; CBMC runs again for every harness on every invocation. Upstream has no plan for one.

## 2. Why Kani next

| Candidate | Case for | Case against |
|---|---|---|
| **Kani** | Ordinary Rust with loops, calls and `match` (Oyster's neutral subset admitted 0 of 53 real functions). Named industrial users paying 21–69 CI minutes per PR and re-verifying everything each time. No result cache to compete with. Per-harness independence. Real verdict-integrity bugs Sail can fix (OOM reported as success, timeout reported as success, 30 of 220 results changing under load). | Bounded evidence only. One release in the last eight months, nightly-pinned. A separate user base from today's Verus users. |
| cvc5 for Verus | Development backend already merged. | No consumer: Z3-dialect queries parse on cvc5 0 of 200, certificates 0 of 192. Verus upstream has to move first. |
| Creusot | Unbounded proofs on ordinary Rust; see [Creusot after Kani](creusot-after-kani.md). | Why3 already replays sessions and runs a prover portfolio. |
| Verus mutation testing | Mostly built; serves current users. | Not a new audience. Gets much cheaper once per-harness keys exist, so it benefits from going after Kani's reuse work. |

Kani is the only candidate where the plain tool does the full work every time, real users feel it, and the structure of the tool (independent harnesses, deterministic completed verdicts) makes exact reuse cheap to argue sound.

## 3. Where the performance is

The pilot number (18.5 s cold, 0.11 s warm on `arrayvec`) is weaker evidence than it looks: its baseline ran nine separate `cargo kani` processes, its key was the whole crate (any edit misses everything), and 85% of the cold cost was the build. The honest baseline is one `cargo kani -j N` with a warm `target/`.

**Cost structure.** On heavy harnesses CBMC is about 91% of post-build time (Kani #5043, ten std harnesses), and inside CBMC the SAT solver ranges from 1% to 61%: symbolic execution and bit-blasting often dominate. On small crates the build dominates. So the gain is in *skipping CBMC entirely for harnesses whose program did not change*, and it scales with harness weight.

**Reuse ladder**, in build order:

| Layer | Key | Skips | Wins on | Expected |
|---|---|---|---|---|
| **B0. Batch executor** | none | nothing | prerequisite: one shared build, then per-harness runs | parity with `cargo kani -j N` |
| **L1. Closure receipts** | authenticated source closure of the harness's package, lockfile, config, tool payloads, argv, bounds | everything, including the build | unchanged reruns, reverts, untouched workspace crates, CI on docs-only PRs | 10–100× on those; 1.0× on an edit inside a single crate |
| **L2. CBMC-boundary shim** | exact bytes of the GOTO file CBMC reads, argv, CBMC and solver binary identity | `goto-instrument` and CBMC | an edit in a crate where most harnesses do not reach the changed code; mutation campaigns | 2–5× on CBMC-bound projects; ≤1.5× where the build dominates |
| **Signed CI tier** | L1 and L2 entries, ed25519-signed (ADR 0017) | as above, across machines | PR CI | CI reruns in seconds; PRs pay only for what they touch |
| **Supervision and scheduling** | n/a | n/a | large suites on memory-bound runners | about 20% makespan (largest-first, #5043) and zero load-dependent flips |

L2 is the product. It is the Verus shim pattern again: a small authenticated shim stands in for `cbmc` and `goto-instrument` in the staged bundle, replays exact output bytes on a hit and execs the real binary on a miss or on any error. Parity holds by construction, and no source-closure authentication is needed because the compile still runs.

**What blocks L2 today, and the fork.** Kani names constant allocations after rustc's `AllocId`, which depends on creation order (Kani #4821). Four whole-crate builds of the same source produced four different GOTO programs, and an edit anywhere can rename symbols in harnesses whose code did not change. Exact-byte keys stay sound but almost never hit. The owner approved forking Kani as Verus was forked (ADR 0039 rules: stock Kani stays the conformance oracle, fork output is a defect-fix only). The fix is small: name allocations by content hash. It also removes a 1.6× solver-time variance upstream reports from the same cause. The fork is dropped when a pinned stock release carries an equivalent fix; whether to offer the fix upstream is the owner's decision, and the problem is recorded locally first.

**One class of miss stays.** GOTO embeds file, line and column, and those appear in user-visible check locations. An edit that shifts lines legitimately changes every reached function below it in that file. Ignoring locations would be normalization; accept the miss and measure how much it costs on real history.

**What is not worth building now:**

- **A whole-formula CNF cache.** Kani's default reachability checks make a passing harness's formula satisfiable and solved goal by goal, so one cached SAT answer cannot reproduce per-check statuses, and it skips only the solver. Dropped unless a spike shows otherwise.
- **Solver racing as a default.** Completed per-check statuses are solver-independent only among SAT solvers on the identical formula. The SMT backends use a different encoding with open CBMC bugs where backends disagree; the pilot's "FAILED on a passing harness" with Z3, cvc5 and Bitwuzla was almost certainly a missing binary reported as a verdict. Racing also multiplies memory, which is the binding constraint. The 2–200× figures are mostly MiniSat against CaDiCaL from 2023, and CaDiCaL is now the default. Racing becomes an opt-in labelled mode later, if a spike shows ≥1.3×.
- **Compile-time reuse.** ADR 0170 and 0052 already showed frontend reuse does not pay. Use Cargo's own fingerprints.

**Honest headline by workload:**

| Workload | Expectation |
|---|---|
| Small crate, workstation edit loop | 0.8–1.2×. Must be held to "never slower". |
| Unchanged rerun, revert, docs-only PR | 10–100× |
| PR touching one module of a mid-size project (s2n-quic, Firecracker class) | 2–5× with L2 and the fork; about 1× on the edited crate with L1 alone |
| Mutation over Kani harnesses | Best case: most mutants never reach most harnesses |
| verify-rust-std scale | Unknown: 16,748 harnesses and about 5 runner-hours of autoharness per run make it the largest prize, but it runs on its own Kani fork and build time matters more there |

## 4. Corrections the plan depends on

- The qualified executor on `feat/v030-kani-cli` cold-builds a fresh target per harness and forces `--unwind N`. Both must change before any comparison with `cargo kani -j`.
- ADR 0128's decoder refuses contracts, stubbing, loop contracts and non-CaDiCaL solvers. Firecracker, Hifitime, zerocopy and verify-rust-std depend on those, and s2n-quic pins Kissat. **Running** those harness kinds has to be admitted, each with its own evidence qualifiers. That is separate from adopting contracts as a Sail front end, which stays closed.
- Supervision that reports "no claim" where direct Kani wrongly prints success is a deliberate, downgrade-only divergence and gets registered in `docs/divergences.md`.

## 5. Correctness contract for the Kani backend

- **Oracle:** direct stock Kani at the same pins, same argv, environment and project bytes, run at least twice.
- **Parity:** every property record (id, class, description, location, status), counts, failed-check list, backend configuration and stream lines are exact. A field may differ only if two direct runs differ on it; each such field is named individually (ADR 0156's eleven are the starting set).
- **Outcomes:** `bounded_success` and `counterexample` carry tier Bounded; `bound_insufficient` completes with no tier; timeout, out-of-memory, crash, cancelled, unsupported and build failure are no-claim and never cached. "Verified" never appears.
- **Qualifiers by harness kind:** bounds with their source; stubs as assumptions; `proof_for_contract` lists assumed contracts; loop contracts record invariant abstraction and whether termination was checked; autoharness records generated origin and single monomorphization.
- **Success needs positive evidence:** observed CBMC exit status, no signal, no cgroup OOM event, complete output, reconciled counts, Sail's own deadline not fired, process group empty afterwards.
- **Cache eligibility:** integrity-clean completed outcomes only. L1 additionally requires the full input closure authenticated before the run and unchanged after it; crates whose build scripts or proc macros read outside the closure fall back to L2.
- **Default mode** uses exactly the solver direct Kani would select, one process.
- **One bounded-evidence shape** in `sail-api` shared with D.I.V.E.'s symbolic path (`bounded-safe`, k-induction): subject, outcome, tier, strength (bounded or inductive), bounds, assumptions, witness and how it was validated, tool identity, integrity, provenance.

## 6. Spike wave

Each spike is a few hours of agent time on native EC2, writes one record under `docs/validation/`, and has a numeric pass rule and a drop rule. All run on stock Kani 0.68.0 unless stated.

| # | Question | Pass | If it fails |
|---|---|---|---|
| S1 | Honest baseline: one `cargo kani -j8`, cold and warm target, against the rebased executor in batch | Batch within 5% of direct | Redesign the executor around one shared codegen before anything else |
| S2 | Cost split per harness (build, codegen, `goto-instrument`, symex, SAT, peak RSS) on `arrayvec`, lading, s2n-quic-core, Firecracker | Post-build share ≥50% of `-j8` wall on two of three real projects | L2 is cancelled; scope becomes CI receipts plus integrity |
| S3 | GOTO determinism: 10 rebuilds, two paths, two hosts; stock, then with the content-hash allocation patch | Patched: ≥99% of harnesses byte-identical across rebuilds, verdicts and checks identical to stock | No L2; L1 only |
| S4 | Hit rate on real history: replay the last 60 commits of s2n-quic and Firecracker with the patched compiler; fraction of harnesses whose CBMC input changed per commit; count line-shift-only misses | Median commit invalidates ≤30% of harness CBMC time; projected ≥2× over `cargo kani -j8` | Below 1.3× projected: L2 is not built and Kani ships as an uncached experimental backend |
| S5 | Admission: share of corpus harnesses that run under today's decoder, then with contracts, stubbing and pinned SAT solvers admitted as run modes | ≥60% admitted and conformant on two heavy projects | Widen first; no headline release below 30% |
| S6 | Integrity: reproduce Kani #4905, #5170 and #5043 at 0.68, then under Sail supervision with largest-first and a memory gate at `-j16` | Zero flips where direct flips at least once; wall no worse | Ship plain parity, no scheduling claim |
| S7 | SAT solver choice: CaDiCaL, Kissat, MiniSat on every harness ≥5 s, binaries verified present | 100% status agreement and virtual-best ≥1.3× | No solver work this phase; any disagreement is a soundness alarm recorded locally |
| S8 | verify-rust-std sample: 500 harnesses, cost split and rebuild determinism on its Kani pin | Informational: sizes the largest prize | n/a |
| S9 | Kani on macOS arm64: nine `arrayvec` pairs | 9 of 9 Same | Linux-only, labelled |
| S10 | Reach on Oyster's 53 real functions with autoharness at a 60 s budget | ≥27 reach the bounded tier | Form B (section 10) stays closed |

**One owner checkpoint** follows the wave: go on L2 and the fork (including where the fork lives and whether the fix is offered upstream), the headline claim, Mac in or out, and whether mutation co-ships. If S2 or S4 fails, the recommendation at the checkpoint is to ship Kani as a qualified uncached backend with integrity supervision and put the release's weight on mutation instead.

## 7. Measurement infrastructure

Kani work is memory-bound throughput (harnesses up to about 10 GB) with a small amount of timed A/B. Build on Sail's `scripts/perf-host.sh` lifecycle (tags, TTL self-termination, tested teardown, `list` must be empty) and graft on what D.I.V.E.'s runs proved:

- **Detached jobs:** `systemd-run` units with a completion marker, so runs survive disconnects (Sail's `run` is a blocking ssh today).
- **Per-harness isolation:** a cgroup per job (`MemoryMax` about 12 GiB, swap off), `timeout --kill-after`, exit code and `/usr/bin/time` files; tolerate jobs that die without output.
- **Fleet:** N lanes at once, spot with on-demand fallback, shards split by longest prior processing time, vCPU quota check, fleet-wide `list` and `down`. D.I.V.E. ran up to ten nodes this way.
- **Resume:** idempotent per-harness result keys (the D.I.V.E. gate node's cached `.rc` pattern) so a lost spot node or an expired TTL re-runs only unfinished harnesses. TTL is set per job with heartbeat `extend`.
- **One build copied everywhere,** verified by sha256. D.I.V.E. found separate builds were not byte-identical; Sail's authenticated Kani bundle already covers the tool side.
- **Results:** nodes write tarballs straight to the private evidence bucket through an instance profile; the repository keeps compact Markdown records citing sha256 (ADR 0038).
- **Two pools:** r7i or m7i throughput nodes for history replays and corpus runs; a small quiet pool of bare-metal `c7i.metal-24xl` (SMT and turbo off, fixed frequency, NUMA-bound, one job at a time, alternating arm order, steal and busy-core check before each sample) for the timed rows that back a release claim.
- **Regions and cost:** prefer regions other than us-east-1; add a budget alarm and spot-interruption handling, which neither repository has.

This fleet tooling is the first workstream after the checkpoint, and the history-replay and mutation lanes for Verus can use it too.

## 8. Workstreams

Each runs in its own worktree under `.claude/worktrees/`, lands by PR, and ships unadvertised until its gate passes (ADR 0174's rule).

| WS | Scope | Needs | Gate |
|---|---|---|---|
| W0 | Rebase and land `feat/v030-kani-cli` (executor, stage, CLI, ADRs 0152–0156); remove forced `--unwind`; authenticated PATH. **Can start now.** | — | Nine direct pairs Same; default Verus conformance unchanged |
| W1 | Bounded-evidence types in `sail-api`, backend and tier in the ledger. **Can start now** so the D.I.V.E. symbolic pickup lands on them. | — | Round-trips Kani and D.I.V.E. symbolic outcomes |
| W2 | Fleet tooling (section 7) | — | A 60-commit replay survives a killed node and resumes |
| W3 | Kani corpus (`corpus/kani-corpus.toml`), `KaniRunRecord`, per-harness and per-check conformance beside `crates/sail-conformance`, fault-injection arm with fake `cbmc`, `sail-bench` cold, warm, edit, history-replay and CI-rerun arms | W0 | Fast tier Same over three rounds |
| W4 | Batch execution on a shared target, `kani list` discovery, harness kinds as run modes, supervision and scheduler | W0, S1, S5, S6 | S1 threshold in product; injected OOM and timeout never yield success |
| W5 | Kani fork with deterministic allocation names in a repository the owner creates for it; pinned fork runtime as a release asset like the Verus engine | S3 | Stock-versus-fork conformance on the corpus; determinism record |
| W6 | L2 CBMC-boundary shim and exact-bytes store | W3, W4, W5 | S4's projection reproduced by product code on two projects, zero differences |
| W7 | L1 input-closure authentication and `kani-v1` receipts (reusing `sail-verus/src/managed/` closure code and the ADR 0175 receipt store) | W3, W4 | Warm unchanged ≤1 s; a miss costs ≤5% |
| W8 | Signed CI tier and GitHub Action for Kani | W6, W7 | Unchanged-PR rerun ≥20× on a fresh runner; tampered entry refused |
| W9 | MCP tools, health for Kani (cost, peak memory, near-timeout, near-bound), consumer skill, `docs/kani.md` | W4 | Agent-loop bench row |
| W10 | Release: performance matrix on the quiet pool, roadmap rewrite, changelog, known issues, Mac pins or a Linux-only statement | all | No row slower than stock unless listed as a failed gate |

Corpus: fast tier `arrayvec`, `x86_64`, lading, zerocopy; standard s2n-quic, Firecracker (pins Kani 0.67, so a secondary pin), Hifitime; deep a verify-rust-std partition and gale (the one project with both Verus and Kani proofs).

## 9. Releases and the longer roadmap

| Release | Content |
|---|---|
| v0.3.1 (in flight) | Temporal receipts and edit sessions |
| Symbolic pickup (milestone #17) | D.I.V.E. symbolic and symbolic-path on the shared bounded-evidence types |
| **v0.4.0** | Kani headline if S2 and S4 pass: "edit a function and Sail re-checks only the harnesses that reach it; same results as `cargo kani`; an out-of-memory or timeout is never reported as success". Other v0.4.0 pillars co-ship only if already qualified. |
| v0.4.x | Signed CI tier if it trails, SAT solver memory, Mac if deferred |
| v0.5.0 | Mutation over Kani harnesses, SMT solvers through CBMC, the next backend |

**After Kani, in order:**

1. **SAT solver identity** (Kissat, CaDiCaL, MiniSat) inside the Kani key, with remembered-best and opt-in racing if S7 passes.
2. **Bitwuzla, then cvc5, through CBMC's SMT path.** Needed for quantified harnesses with symbolic bounds; each is its own named backend with its own conformance. This is where cvc5 gets a real consumer, and it reuses the cvc5 runtime work already merged.
3. **Concurrency: Loom.** Not the next backend; a spike-gated client of the Kani executor. See [Loom after Kani](loom-after-kani.md).
4. **Creusot.** After cvc5 is a qualified multi-solver backend, driven through `why3find` first. See [Creusot after Kani](creusot-after-kani.md).
5. **ESBMC on the same GOTO** as an independent second opinion once Kani's `-Z esbmc` lands; a disagreement is an alarm, never blended.
6. **Certificates** (DRAT or LRAT for cached UNSAT, Alethe for SMT): one short spike each; no product work unless CBMC exposes proofs.
7. **cvc5 for Verus:** when Verus upstream's cvc5 support passes the fast corpus.
8. **Flux, Crux-MIR:** shim-shaped at the SMT boundary, demand-led. **Prusti, Aeneas/Lean:** not scheduled.

## 10. Neutral spec and constraint frontends

Oyster failed on reach (0 of 53 functions) and on building a negotiated protocol before any user outcome. The lesson holds: no new source syntax, no validator, no shadow crates.

- **The neutral layer goes below the frontends.** `sail-api` gains `Input` kinds beside `Smt` and `Temporal`, with one job, evidence, receipt and ledger contract. Raw SMT-LIB stays the constraint-level entry point. GOTO becomes a second neutral artifact once two engines consume it (CBMC and ESBMC).
- **A source-level form returns only as Form B:** a thin `requires`/`ensures` attribute that each backend expands natively. S10 measures the bar already written down (at least half of 53 real functions reach the bounded tier with only the attribute). If it passes, a one-week pilot follows v0.4.0.
- **Sequence:** two real frontends first (Verus and Kani), then the shared evidence contract proves itself across them and D.I.V.E., then Form B if S10 passes.

## 11. Decisions to record

ADR numbers 0152–0156 are taken by the branch stack; new ones start at 0180.

| ADR | Decision |
|---|---|
| 0180 | Kani phase scope and the spike-wave checkpoint outcome; amends ADR 0128's cache ban and no-fork clause |
| 0181 | Parity definition against direct Kani and the outcome taxonomy |
| 0182 | Integrity supervision as a registered downgrade-only divergence |
| 0183 | Kani conformance harness, fault arm, corpus tiers and secondary Kani pins |
| 0184 | Harness kinds as run modes with qualifiers; Form B stays closed |
| 0185 | Bounded-evidence types shared with D.I.V.E. symbolic |
| 0186 | Batch execution, selection independence, scheduler in default mode |
| 0187 | Kani fork for deterministic output, under ADR 0039's rules |
| 0188 | CBMC-boundary shim and exact-bytes store |
| 0189 | Input-closure authentication and `kani-v1` receipts |
| 0190 | Signed shared and CI tier for Kani entries |
| 0191 | Fleet measurement method for Kani |
| later | Opt-in solver race; SMT-through-CBMC backends; Loom ([record](loom-after-kani.md)); Creusot ([record](creusot-after-kani.md)) |

## Critical files

- `crates/sail-kani/src/{lib,decode}.rs`; on `feat/v030-kani-cli`: `execution.rs`, `tools.rs`, `tools/stage.rs`, `crates/sail-cli/src/kani_cli.rs`, `docs/kani.md`
- `crates/sail-runtime/src/temporal.rs` and `temporal/receipts.rs` (backend, worker and receipt templates)
- `crates/sail-verus/src/managed/` (`build_inputs_sources.rs`, `dependency_snapshot.rs`, `freshness.rs`, `receipts.rs`) for closure authentication
- `crates/sail-cache/src/lib.rs` (`DiskCache`, `put_scoped`), `crates/sail-daemon/src/shared.rs` (signed tier), `crates/sail-daemon/src/jobs/schedule.rs`
- `crates/sail-conformance/src/lib.rs` (`Verdict`, `classify`), `crates/sail-bench/src/{cold,warm,edit}.rs`
- `crates/sail-api/src/output.rs` (`EvidenceTierName`), `crates/sail-api/src/runtime.rs` (`Input`)
- `xtask/src/layering.rs` (new edges from `sail-kani`)
- `scripts/perf-host.sh`; the D.I.V.E. repository's `scripts/aws/` and `scripts/perf/scale/` as the job-model reference
- `docs/decisions/0128-kani-bounded-observations.md`, `docs/ROADMAP.md`, `docs/divergences.md`

## Verification

- **W0:** the nine direct-versus-Sail pairs on a native Linux lane; `ci/local.sh conformance fast pr` shows default Verus unchanged.
- **Every spike:** a record under `docs/validation/` with host type, region, pins, result against its pass rule, and sha256 of the raw archive.
- **Every reuse layer:** conformance on every benchmark round; a speedup that coincides with any verdict difference is a bug. Fault arm proves no-claim outcomes are never stored.
- **Release:** the performance matrix on the quiet pool, alternating arms, against `cargo kani -j N` with a warm target.
- **End of every session:** `scripts/perf-host.sh list` is empty.
