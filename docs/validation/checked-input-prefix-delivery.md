# Input-prefix and delivery qualification

The complete input-prefix component, ordinary failure controls, native Linux
runtime suites and Mac host gate pass. The bounded production-shim cost run
passes all 400 timed sessions. Final raw-evidence review and current-head CI
remain merge gates; the physical origin and whole-service theorem remain open.

## Proof scope

| Check | Actual result |
|---|---|
| Complete `prefix` positive, `14d37ed8` | 182 verified, zero errors; complete 264-function inventory, including all 17 original witness/factory/bridge functions |
| Ordinary controls, `092315fa` | All 37 pass: each actual whole-root call reports 181 verified, one error and the complete inventory |
| Current program correspondence | All 316 complete generated programs/dependencies match the audited `092315fa` inputs: 278 inherited cases and 38 prefix cases |
| Abstract models | All 35 complete programs match the previously audited actual `838706f0` executions, including 31 ordinary negatives; reused, not rerun here |

The prefix calls use the pinned Stock Verus, default 120-second timeout,
`--num-threads 1`, `--no-cheating` and complete roots. No timeout, resource or
VIR refusal counts as an ordinary failure. The manifest locks the actual
182/264 discovery. Both original descriptors that generate the same feed
mutation remain registered and were separately executed: 38 registered prefix
cases produce 37 unique complete programs.

The two checked slice-length assertions establish the bounds of the actual
Rust slice. Contracts, executable bodies, loop boundaries, controls and limits
remain unchanged. The actual factory and shared operations compile in the
Stock driver. SHA, owned-pipe adapters and surrounding native callers are
reviewed and source-bound; they are not thereby Stock-proved.

## Native execution

Sail uses Rust 1.99.0. Direct proofs use Stock
`0.2026.10.18.7d82994` with proof Rust 1.99.1. Native verification uses the
`47fb3be6` fork and pinned Z3 4.16.0; the Linux engine manifest is
`d6096d5bd4952df54f8fdd0fdc157a1bb610298318166495604ccb7201dbe44b`.

The actual Linux source is `7c1bb7c5`, based on main `0057eba6`, on native
EC2 `i-0af9ec6d569b12d3b`, `c7i.4xlarge`, `us-east-1`. Normal commands run
sequentially in their unchanged pinned CI images, with complete canonical
source and validated runtime/image checks before and after every stage.

| Gate | Actual result at `7c1bb7c5` |
|---|---|
| `ci/local.sh gate` | PASS: 76 source-checker tests; 1,239 workspace tests, 54 skipped; fmt, strict Clippy, doctests, rustdoc, layering, deny and generated benchmark check; 218.76 s |
| `ci/local.sh solver` | PASS: all 51 pinned-Z3 tests; 19.05 s |
| `ci/local.sh e2e` | PASS: full normal phases, including local extra and real MCP/repair checks; 1,049.40 s |
| `ci/local.sh conformance fast` | PASS: four arms, each 147 same/147 units and 3,451 functions; 146 engine-focused units; 514.35 s |
| `ci/local.sh conformance standard` | PASS: four arms, each 159 same/159 units and 13,166 functions; 158 engine-focused units; normal 18 mutants plus cold/revert/revert-forced controls, all 21 rows match |
| Native arm64 Mac `ci/run.sh host` | PASS: strict Clippy across all targets, 1,228 tests, 54 skipped; 271.59 s |

All corpus arms report zero unstable, different, infrastructure-error or
skipped-project results. Fast and standard each report resource
counter drift in 11 and 17 units respectively; per-function verdict and
diagnostic comparisons pass. The mutation sample remains the normal
`--per-class 2 --rounds 1` selection, without project filters or reduced scope.

The Mac gate uses the normal caller Cargo/kache setup, one Cargo job, serial
tests and `nice -n 19`. Its source/tool/configuration checks pass, including
fresh path-bound dependency records. All seven explicitly required new
prefix/delivery tests and all six layering graph tests pass; the complete
suite executes 14 additional tests relative to its command/cache parent.
The Mac command does not run doctests, Stock proofs, pinned-Z3 tests, corpus
conformance or performance measurements.

Main subsequently adds #596's source-sync tooling, one Linux gate test and
Docker package support. The final restack preserves the actual production,
proof, Cargo, xtask and pin bytes from `7c1bb7c5`, and composes that tooling's
separate qualification with current-head CI. It is not a fresh execution of
every native suite on the restacked commit.

## Bounded added cost

Two normal isolated release builds compare main `0057eba6` with `7c1bb7c5`
in the same qualified native Linux CI image. Each timed call executes the
actual `sail-z3 -smt2 -in` with the pinned Z3 and an owned `sail serve` daemon.
The inputs are the existing 58,932-byte Verus transcript and the existing
74-byte `check-sat-using`/`check-sat` fixture. No managed unchanged-call shortcut
can replace these actual shim executions.

Five blocks alternate the first arm, with ten paired sessions per block:
50 sessions per arm, fixture and cache population, or 400 timed calls total.
All 400 pass; 200 untimed warm fills are retained separately. Cold means a
fresh daemon and query cache per sample. Warm means an untimed fill on that
same live daemon before the timed call. Binary/input guards keep those files
resident; this is not a machine-cold or project-startup benchmark.

| Fixture / population | Baseline p50 | Candidate p50 | p50 ratio | p95 ratio | Median paired ratio |
|---|---:|---:|---:|---:|---:|
| Transcript / cold | 95.641 ms | 94.883 ms | 0.992 | 1.001 | 0.997 |
| Transcript / warm | 36.230 ms | 36.603 ms | 1.010 | 1.006 | 1.011 |
| Tactic / cold | 71.095 ms | 70.896 ms | 0.997 | 1.002 | 0.999 |
| Tactic / warm | 1.517 ms | 1.451 ms | 0.957 | 0.987 | 0.929 |

Ratios are candidate/baseline. P50 is the median, p95 the nearest-rank 95th
percentile, and paired ratios match block/session indices. These are observed
component-process samples, not an isolated leaf cost or whole-project speedup.
The warm transcript starts a real SMT session in all 50 samples per arm;
the warm tactic starts none and serves cached replies. Those populations are
kept distinct. All cold samples start a real SMT session and have no served
cache replies.

Wall time includes process launch and timeout bookkeeping. Daemon startup,
readiness, fills, queued-write drain, validation and serialization are outside
the timer. Child CPU is recorded separately and excludes daemon CPU. Every
sample retains raw stdout/stderr/exit, actual-PID provenance, original input
SHA, daemon identity/status, cache headers/payloads and paired v2 key sets.
The original transcript comparator checks both complete response counts and
all non-statistics replies; only its existing `:rlimit-count` comparison is
used for statistics. An additional check rejects incomplete trailing output
on both streams. The original repository helpers remain unfixed in
[#611](https://github.com/Chelis-Lang/sail/issues/611).

## Evidence and limits

| Packet | SHA-256 |
|---|---|
| Actual positive plus 37 controls | `16ffb6a1f34ece38c0bffa54deb742e71f59f05f11cb1f42a3992b5eca9f80fb` |
| Native Linux and all cost samples | `41c229d206909325af95d698d13c020f26bc51bfafae4d9c6b768e3437d773cd` |
| Mac host gate | `4d3b846cff4040a1833356658ad40c3054b5283c6d6aa4dd64d39416007e6aed` |

The private keys are respectively
`experiments/2026-10-29/acceptance-prefix-092315fa-proofs-closed.tgz`,
`experiments/2026-10-29/acceptance-prefix-7c1-native-closed.tgz` and
`experiments/2026-10-29/acceptance-prefix-7c1-macos.tgz`.
The proof packet is 22,668,910 bytes; the native packet is 792,959,391 bytes
with 99,455 indexed payloads; the Mac packet is 15,451,902 bytes with 1,415
indexed payloads. Whole archives and every indexed payload are read back and
checked. All three private uploads have fresh full-download SHA checks. Independent
native/cost raw review remains the final evidence-review gate.

The packets retain canonical Git bundles, actual generated proof inputs,
complete function inventories, raw output, source/runtime maps, selected
binaries, Cargo fingerprints, managed/repair/neutral-service evidence and
cost-session data. Public Stock/Rust runtime payloads are excluded: recorded
full runtime/tool maps and validation guards are retained rather than claiming
an independent binary rehash from absent payloads. Passing output-loop and
legacy temporary projects deleted by the unchanged harness are not
reconstructed. Mac temporary child payloads not printed by its tests are not
claimed retained. Mutation evidence retains the original runner's 21 outcome
rows and setup record; its per-mutation stock/Sail stdout/stderr streams are
not retained, so no independent raw diagnostic recomparison is claimed for
those rows. Direct conformance and all cost-session raw streams are retained.
Link targets and nonregular-file types are recorded without dereferencing them.

The SHA, pipe, launched-image and writer/protected-filesystem boundaries remain
explicit. This qualification establishes no universal solver-result-origin,
compiler-input closure or complete-service theorem. See
[ADR 0089](../decisions/0089-checked-input-prefix-delivery.md) and the owning
[acceptance design](../design/evidence-acceptance-core.md).

## Indexed earlier attempts

| Source | Date | Attempt |
|---|---|---|
| `369fb802` | 2026-10-29 | Whole positive FAIL: 181 verified/one error, complete 264-function inventory; missing slice-length proof guidance, without resource/VIR refusal. Closed raw SHA `05bc1173a802854b94a70c451cec7ff996f9526749cab9d2320c23cfccc03e19`. |
| `2fb8c6b8` | 2026-10-29 | Controller setup refuses scoped Git ownership before any gate; corrected capture retains that attempt. Workspace command then exits zero, but the source guard refuses Cargo's dependency-order rewrite. No completed native qualification credit. |
| `7c1bb7c5` cost v2 | 2026-10-29 | Docker 125 before Cargo or timing: the read-only checkout lacked the nested target mountpoint. Zero timed sessions. The next harness also corrects the declared optional `Info.bundled` omission before timing; no product or check change. |

Superseded waiting cost packages/queues are unexecuted preparation, not failed
measurements. The positive `14d37ed8`, ordinary controls `092315fa` and actual
native/cost `7c1bb7c5` retain their original execution identities.
