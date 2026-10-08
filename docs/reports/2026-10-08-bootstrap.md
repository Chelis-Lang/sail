# Bootstrap report (initial session, 2026-10-08)

This is the report the charter asks for at the end of the initial session
("Initial session deliverables"). Each claim points at the document, ADR or
data behind it.

## Deliverables

| # | Charter deliverable | Status | Where |
|---|---|---|---|
| 1 | Workspace skeleton, CI tiers, lints, `AGENTS.md`, charter | Done | `Cargo.toml`, `.github/workflows/`, ADR 0001–0002 |
| 2 | Survey, confirmed Verus↔Z3 account, integration decision | Done | `docs/survey/`, ADR 0003 |
| 3 | Pinned corpus, plain-Verus baseline, query corpus and profile | Done: Linux x86-64 reference and macOS arm64 | `corpus/`, `docs/benchmarks.md`, `docs/survey/solver-behavior.md` |
| 4 | Pass-through shim and session model, passing conformance | Done: 0 differences | `crates/sail-shim`, `crates/sail-session`, ADR 0005–0006 |
| 5 | ADRs and open questions | Done; questions 1–8 answered | ADR 0001–0010, `docs/open-questions.md` |

The repository is private on Chelis-Lang. `main` is protected:
- Required checks: `No AI authorship markers` and `Workspace gate`, with
  admins included.
- Merges are squash-only.

The AI-authorship guard follows the org pattern (hook, CI job, Claude Code
settings). A negative test showed it rejects an AI trailer both locally and
in CI (ADR 0002).

## Survey findings

- **Integration point (ADR 0003).** Verus runs `$VERUS_Z3_PATH -smt2 -in`,
  and the launcher and `cargo verus` preserve the variable. A Z3-compatible
  binary named there sees every solver interaction without any change to
  Verus.
- **Protocol** (`docs/survey/verus-z3.md`):
  - Each bucket (a module or a spun-off function) gets one long-lived Z3
    process. Each nonlinear or bit-vector query gets its own short-lived
    process.
  - Commands go in synchronous batches, each ending with
    `(echo "<<DONE>>")`.
  - Each query sits inside `(push)`/`(pop)`, with statistics requests around
    it, a version check and a per-check `rlimit` (30M by default).
  - Function names never reach the pipe.
- **Prior art** (`docs/survey/prior-art.md`):
  - Z3 answers can depend on process history and on the platform.
  - Options that look output-only change the search.
  - Normalization is risky.
  - Semantic reuse belongs in opt-in modes only.
- **Corpus** (`docs/survey/corpus.md`): Verus already runs `verita`, a
  nightly cross-project runner, and our corpus starts from its list.

## Integration evidence

These runs used the pinned binaries: Verus `0.2026.10.04.426d8b0` and Z3
4.16.0.
- A logging wrapper named by `VERUS_Z3_PATH` captured a complete session:
  554 commands, 11 checks, 104 replies paired one-to-one. It is kept as a
  test fixture.
- Through `sail-z3`, Verus's stderr was byte-identical to plain Verus, and
  its JSON was identical apart from timings. The shim and direct Z3 gave
  identical replies on the recorded session; statistics replies were
  compared on `rlimit-count`, because their time and memory figures vary on
  every run.

## Pass-through conformance

`sail-conformance` compares sail with plain Verus (ADR 0006): exit code,
stderr byte for byte, `verification-results` and `func-details`, and every
function's verdict. No normalization is applied.

| Tier | Units | Functions | Same | Different | Infra errors | Platform |
|---|---:|---:|---:|---:|---:|---|
| fast | 147 | 3,450 | 147 | 0 | 0 | macOS arm64 |
| standard (includes fast) | 158 | 12,583 | 158 | 0 | 0 | macOS arm64 (anvil skipped: no OpenSSL) |
| fast subset (PR gate) | 33 | 311 | 33 | 0 | 0 | Linux x86-64 (CI) |
| fast, 3 benchmark rounds | 147 | 3,450 | 441 rounds | 0 | 0 | Linux x86-64 (CI) |

Every benchmark round also runs this comparison. In the baseline, 441 of
441 rounds were the same.

## Baseline and profile

From `docs/benchmarks.md` (generated):
- **Fast tier, cold workload, 3 rounds, macOS arm64 under development load:**
  plain Verus took 143.0 s in total (sum of unit medians), and pass-through
  took 143.8 s (+0.6%, well within the run-to-run spread).
- **Where the time goes:**
  - human-eval-verus (30 s) and vstd (26 s) account for most of the wall
    time;
  - the solver accounts for most of the work: Verus reports 129 s of solver
    time, summed across threads.
- **Linux x86-64 reference baseline (GitHub runner, 2 CPUs):** 292.8 s for
  plain Verus against 288.1 s pass-through. Pass-through has no measurable
  overhead.

From `docs/survey/solver-behavior.md`, measured on 3,645 checks (the
examples and vstd):
- **Query time:** median about 1 to 2 ms per check. The slowest 10% of checks
  take 64 to 72% of check time. Two cuckoo-hash examples account for 39% of
  the examples' check time.
- **Seeds:** with 8 seeds, 0 checks changed answer, but effort varied by up
  to 37×. No check came close to its limit.
- **History dependence:** run in a fresh process, every check gave the same
  answer. But 64 of 97 models and 72% of rlimit deltas differed. So the
  default-mode key must bind the whole process prefix (ADR 0008).
- **`rlimit` semantics:** each `(set-option :rlimit N)` gives the next check
  N more units. Exhaustion reports `canceled`.

## Cache hit rates (measured before any cache existed)

`sail-bench hits` compares traces from two runs under key v2:
- **Warm, no change (fast tier):** 95.0% of checks hit, covering 99.9% of
  solver time.
- **One-function body edit in human-eval-verus:** 94.1% of checks hit,
  covering 95.6% of solver time. Only the edited bucket re-solves.
- **The remaining misses in the warm case** come from Verus itself. It emits
  some declarations in a run-dependent order
  (`corpus/repros/verus-nondeterministic-declaration-order/`; open question
  9).

## First cache wave (ADR 0011)

A disk cache behind `sail serve`, with `sail-z3` serving replies from it.
The solver starts only on a miss, and the whole session prefix is replayed
into it.

Fast-tier conformance with the cache on, on macOS arm64 under heavy load:

| Run | Conformance | Plain Verus | sail | Ratio |
|---|---|---:|---:|---:|
| Fill (empty cache) | 147 of 147 the same | 178.6 s | 200.0 s | 1.12 |
| Warm | 147 of 147 the same | 169.1 s | 118.6 s | 0.70 |

**Warm, per unit:**
- human-eval-verus: 3.5× faster;
- vstd: 1.55× faster;
- cuckoo examples: about 1.7× faster.

In the warm runs, what remains is mostly Verus's Rust front end, which the
charter's "warm Verus frontend" roadmap item addresses.

**A cache-only pairing bug was found before merge, and fixed.** The cause was
timing-dependent: a dropped expectation shifted stored replies by one
command. The fix comes with an order-independence property test, reply shape
checks when serving and storing, and per-unit timeouts in the harness. See
ADR 0011's update.

## Surprises, captured as reproductions

- **Z3 rlimit jitter on nonlinear queries**
  (`corpus/repros/z3-nla-rlimit-nondeterminism/`). Byte-identical input gives
  `rlimit-count`s a few units apart from run to run, with the same answer, in
  plain Z3. This led to the amendment of charter rules 1 and 4, and to the
  unstable classification (ADR 0009).
- **Shim trace ordering.** The tap could record a reply before its command.
  This was fixed, and forwarding was never affected.
- **Verus's declaration order** varies between runs; see the hit-rate
  section above.

## Decisions taken

ADR 0001–0010:
- workspace and CI;
- the authorship guard;
- the integration point;
- the session model and key v1;
- the shim's fail-safe rules;
- the conformance harness;
- the benchmark method;
- prefix keys (v2);
- unstable results;
- the owner's decisions on the open questions.

## Open items for the next wave

- **Fork pins.** Archive fork pins locally, per ADR 0010 (`page-table` pins
  verita's fork).
- **Singular.** The pinned release's `rust_verify` lacks the `singular`
  feature, so `verified-memory-allocator` and `examples/integer_ring` need a
  Singular-enabled Verus build to join the corpus.
- **Linux baseline.** Commit the reference baseline from the bench workflow,
  then run the standard tier on Linux (dispatch).
- **Caching wave, next steps:**
  - record effort (the rlimit used against the limit) with each `check-sat`
    entry, and calibrate the near-limit threshold, so that cached `unknown`s
    can be served or flagged under ADR 0009;
  - warm and edit workloads in `sail-bench`, with cache-on numbers on Linux;
  - an on-demand daemon and `sail verify` (setting `VERUS_Z3_PATH`,
    `SAIL_Z3_PATH` and `SAIL_DAEMON_SOCKET` around `cargo verus`);
  - fault injection: kill the daemon or solver mid-session, and corrupt
    entries.
