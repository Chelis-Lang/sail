# Real-project performance, 2026-10-15

A tester reported about 1.2× from `sail verify` with a warm cache on a large
Verus project. [Performance](../performance.md) claims 1.3–1.5× on vest, but
those edits are lemma edits in one small module. This report measures five
pinned real projects with realistic edits. It explains why real projects gain
less and how high the gain could go, and ranks the levers that would raise
it. It is an investigation only: no product code changed. Lane L40.

## Summary

- **Edits that touch one module:** 1.2–2.3× (typical, `sail verify`, warm
  cache). **Edits to a widely used spec function:** 1.0–1.6×, about 1.0× when
  its users are expensive. **Unchanged
  re-verification:** 1.45–3.2×. This is the ceiling: even when every query
  hits the cache, Cargo, rustc and Verus's own work remain.
- **Main cause, not previously documented: a miss costs the whole module.**
  Verus runs one Z3 process per module (a *bucket*). A cache miss starts Z3
  and replays the bucket's entire earlier input, including every earlier
  `check-sat`, which Z3 solves again. So an edit to a module's last function
  costs as much as an edit to its first. In every project, `body-late` and
  `body-early` take the same time. On anvil, a one-function edit costs
  104 s of plain's 124 s (1.19×), against 38 s when nothing misses (3.2×).
  The cache's hits before the changed byte save Verus waiting on replies;
  they do not save Z3's work.
- **Widely used spec changes invalidate many buckets.** That costs
  everything where the spec function's users are expensive (ironkv 1.01×,
  capybarakv 1.07×, anvil 1.00×) and little where they are cheap (verismo
  1.45×, vest 1.60×). This happens even when the edit only renames a
  parameter.
- **Inserting an item renumbers crate-wide IDs:** the anonymous-closure type
  counters. Unrelated modules then miss from their first check. On vest, a
  new lemma in `der_ord.rs` re-solves `cbor::format`, the most expensive
  module: 1.14× instead of 1.6×.
- **Stock nondeterminism (VI-0001) misses 1–9 buckets on unchanged reruns**
  (ironkv, capybarakv, vest, verismo; never anvil). Every observed case is
  the `arrow_*` declaration order, which precedes the bucket's first check.
  The fork's Verus produced identical SMT on every rerun of all five
  projects. Its timing effect is 0–15% on most runs and up to about 40% on
  an unlucky run (capybarakv: one rerun of 20.3 s against 12.3 s).
- **`sail verify --incremental` was slower than plain** on vest, capybarakv
  and verismo (0.5–0.7×). Run from the package directory without `-p`,
  every managed run recompiled the Verus macro crates and verified vstd
  again. This still happens on `origin/main` `49487ef7`, and also with
  vest's own crates.io vstd. From vest's workspace root with
  `-p vest_lib`, an unchanged managed run takes 1.5 s and an edit 31 s
  (issue #157). A tester using managed mode from a package directory could
  see this alone.
- **Comments and pure line shifts cost nothing.** They change only the `;;`
  source-position comments in the SMT log, and no query misses (0 extra
  misses in every project).

## Setup

| | |
|---|---|
| Hosts | 2 × EC2 c7i.4xlarge (16 vCPU, 30 GiB), us-east-1, Ubuntu 24.04, `scripts/perf-host.sh up l40` and `l40b`, one timed run at a time per host, nothing else on the host |
| Sail | `63e8dfb4` (this branch's base), release build of `sail-cli`, `sail-shim`, `sail-conformance` |
| Stock Verus | 0.2026.10.04.426d8b0 (`cargo xtask fetch-verus`), Z3 4.16.0 |
| Fork Verus | engine runtime `engine-runtime-13a1176c-linux` (fork commit `13a1176c`, archive sha256 `a28e212e…7c2`, as pinned in `engine/verus/artifact-linux.json`), used as a plain `cargo verus`/`verus` |
| Projects | corpus pins in `corpus/corpus.toml`, fetched by `sail-conformance fetch`; each copy's Verus dependencies patched to the pinned Verus source exactly as conformance does (`sail_verus::patch`) |
| Date | 2026-10-15 |

Projects (plain warm edit time in brackets): ironkv `ironsht` (10 s),
capybarakv (29 s), verismo `source/verismo`, which also verifies its
dependency `verismo_tspec` (36 s), vest `vest_lib` (45 s), and anvil
`--lib --verify-module vdeployment_controller` (124 s). Page-table and
node-replication were left out: page-table takes 14–46 min per run and fails
on rlimit with the pinned release (VI-0002), and node-replication verifies in
8 s with too little solver work to matter.

**Quietness.** Each host ran a queue that executed timed runs strictly one
at a time, with no builds, fetches, syncs or other projects in parallel.
Setup and SMT-log comparisons ran before or after the timed queue. Before
every run, the harness pauses 5 s and records the 1-minute load average and
the number of other running processes. Medians per project were 1.8–4.5;
these averages still include the previous run's own 15-thread Verus work.
At most 2 other processes were running at any run's start. Earlier attempts
that overlapped with other work, or hit harness bugs, were stopped within
minutes and their output deleted. No number here comes from them.

**Edits** (`scripts/perf-real.sh`, one edit at a time from the pristine
source; the targets are listed in the script):

| Edit | What it does |
|---|---|
| noop1–3, noopend | a comment appended at the end of the edited file: no query change (the all-hits floor, and unchanged reruns) |
| body-early, body-late | `assert(true);` at the start of a function early or late in a large module |
| comment-top | a comment line inserted at line 1 of that file (every later line shifts) |
| lemma-mid | a new small `proof fn` inserted mid-file |
| spec-wide | the first parameter of a widely used `open spec fn` renamed (a pure refactoring) |
| fail, fix | `assert(1 + 1 == 3);` then `assert(1 + 1 == 2);` in a proof function (new bytes) |
| catalog (vest) | the frozen catalogue edit used by `docs/performance.md` |
| upstream-body, upstream-spec (verismo) | the same kinds of edit in the dependency crate `verismo_tspec` |

Modes: plain `cargo verus verify` (`focus` for anvil), `sail verify`,
`sail verify --incremental`, and both Sail and plain with the fork's Verus.
Three rounds per project, two for anvil (124 s per plain run), each round
starting from a fresh copy and an empty cache. Every run passes
`--time-expanded`. Cache counters come from `sail status --json` before and
after each run, plus the shim's per-session provenance
(`SAIL_PROVENANCE_DIR`). Exit codes agreed across modes in every round and
phase of every project. In verismo, `assert(1 + 1 == 3)` verified, in
`proof_open_wf` and in `osmem_find`, with both stock and fork, so verismo has
no `fail` result below.

The cause is in verismo, not in Verus or Sail (issue #160). verismo_tspec's
trusted `#[verifier(external_body)]` broadcast axiom
`axiom_size_from_cast_bytes` states
`size_of::<T>() == T::spec_size_def()`, and `impl SpecSize for char` gives
1, while vstd's `layout_of_primitives` states `size_of::<char>() == 4`.
Wherever a function's context keeps both, `false` is provable: in
`security::mem`, `assert(false)` in a `proof fn` with no precondition
verifies. A 30-line file with only that axiom and impl reproduces it with
stock Verus 0.2026.10.04. With `[char, 4]` instead, `assert(false)` fails
again in `security::mem`, one `verismo_tspec` proof
(`proof_field_set_constant`) fails, and with that proof admitted, verismo
reports 1,118 verified and 165 errors instead of 1,283 and 0. Of 17
`assert(1 + 1 == 3)` insertions, one per file that has a top-level
`proof fn`, 13 verified. So verismo's measurements stay valid as timings,
but most of its queries are discharged from a contradiction.
`scripts/perf-real.sh` now puts verismo's `fail` and `fix` in
`lemma_seq_to_multiset_swap` (`tspec_e/array/sort.rs`), where the failing
assert fails; the `fix` figures below were measured in `osmem_find`.

## Results

Wall seconds, median of rounds. Ratio is plain divided by Sail. "Missed"
counts the Z3 sessions (buckets) that started a solver; "puts" counts the
replies solved and stored.

### Speed per edit (`sail verify`, stock Verus)

| Edit | ironkv | capybarakv | verismo | vest | anvil |
|---|---|---|---|---|---|
| plain (typical edit) | 9.9 | 28.8 | 36.0 | 45.2 | 124 |
| noop (all hits) | 5.6 (1.78×) | 12.7 (2.24×) | 24.6 (1.45×) | 27.9 (1.61×) | 38.2 (3.25×) |
| comment-top | 5.6 (1.77×) | 12.6 (2.29×) | 24.5 (1.49×) | 27.6 (1.65×) | 38.4 (3.23×) |
| body-early | 7.5 (1.33×) | 13.3 (2.25×) | 28.1 (1.27×) | 28.2 (1.61×) | 103.5 (1.19×) |
| body-late | 7.4 (1.34×) | 13.1 (2.22×) | 28.5 (1.29×) | 28.0 (1.61×) | 104.1 (1.19×) |
| lemma-mid | 7.7 (1.29×) | 13.1 (2.14×) | 28.3 (1.27×) | 39.8 (1.14×) | 104.3 (1.20×) |
| spec-wide | 9.9 (1.01×) | 26.8 (1.07×) | 24.7 (1.45×) | 28.4 (1.60×) | 124.3 (1.00×) |
| fail | 7.6 (1.16×) | 11.1 (2.33×) | — | 24.8 (1.63×) | 31.3 (3.64×) |
| fix | 7.2 (1.39×) | 13.4 (2.14×) | 27.7 (1.33×) | 28.0 (1.62×) | 64.1 (1.95×) |
| upstream crate edit | | | 44.2–45.5 (1.29–1.32×; plain 58.5) | | |
| cold | 0.97× | 1.01× | 0.99× | 0.99× | 1.01× |

Fork Verus under Sail (`fork sail`) matches these within the spread, except
on unchanged reruns, where stock sometimes misses. ironkv's noop is 4.8–4.9 s
on the fork against 5.5–5.7 s stock. On capybarakv, stock's first noop took
13.2–21.1 s (8 buckets missed) against 12.1–13.4 s on the fork. Plain
fork runs took the same time as plain stock runs (ratio 1.00–1.06×), so the
fork's frontend costs nothing.

`sail verify --incremental` medians:

| | ironkv | capybarakv | verismo | vest |
|---|---|---|---|---|
| noop | 6.0 (1.64×) | 44.9 (0.64×) | 69.5 (0.51×) | 62.8 (0.72×) |
| body edit | 7.9 (1.25×) | 45.0 (0.66×) | 74.3 (0.48×) | 62.9 (0.72×) |

On capybarakv, verismo and vest, every `--incremental` run recompiled about
18 crates: `proc-macro2`, `quote`, `verus_syn`, vstd, and the Verus macro
crates. It verified vstd again (daemon hits 26,000–53,000 per run against
6,000–33,000 for `sail verify`) and printed `Unknown evidence … 2 verifier
runs`. On ironkv, a single package with no local path dependencies, it
printed `Current evidence … 1 target reports reused`.

The harness runs each project from its package directory without `-p`.
That condition triggers the problem, not the patched Verus dependencies. A
check on `origin/main` `49487ef7` used the engine runtime pinned there and
the unmodified vest, with crates.io vstd. From `vest/vest_lib`, cold took
83 s, then two unchanged runs took 61 s each and an edit 62 s, with vstd
compiled and verified every time. From `vest/` with `-p vest_lib`, cold
took 83 s, the unchanged runs 1.5 s and 1.6 s with no verifier run, and the
edit 31 s. Filed as issue #157.

Every incremental cold run passed, so issue #123 (a failing first managed
run, fixed in #125) was not the trigger here. It can also add to a
tester's numbers in managed mode.

### Where the time goes

Verus's own `--time-expanded` (median, plain, typical edit; solver wall
estimated as verify-crate wall × smt-run ÷ verify-time, both summed over
threads), against the all-hits run under Sail:

| | ironkv | capybarakv | verismo | vest | anvil |
|---|---:|---:|---:|---:|---:|
| plain wall | 9.9 | 28.8 | 36.0 | 45.2 | 124 |
| rustc (Verus `rust-time`) | 1.6 | 4.3 | 8.0 | 7.5 | 10.9 |
| VIR | 1.3 | 3.4 | 5.8 | 6.7 | 14.7 |
| verify-crate | 6.0 | 19.1 | 19.4 | 27.2 | 91.5 |
| estimated solver wall | 4.2 | 14.2 | 7.1 | 11.2 | 85.7 |
| Amdahl ceiling, plain ÷ (plain − solver) | 1.74× | 1.99× | 1.25× | 1.33× | 3.24× |
| measured all-hits floor (Sail noop) | 5.6 | 12.7 | 24.6 | 27.9 | 38.2 |
| verify-crate with all hits (fork) | 0.9 | 3.0 | 8.0 | 9.9 | 6.0 |

Even with 0 misses, verify-crate keeps 1–10 s. That is Verus building
AIR, SST and SMT for every function and waiting on the shim. The
remainder of the floor is rustc (twice: verification and the EXEC pass),
VIR and Cargo. On verismo and vest, the measured floor beats the estimated
ceiling: Sail also removes Verus's per-check overheads, not only Z3's time.

### What misses, and what it costs

From the timed runs (fork Verus, so no nondeterminism) and from SMT logs
(`scripts/perf-real.sh det`, `;;` comment lines removed, edit compared with
an unchanged run):

| Edit | Buckets changed (checks after the first difference / bucket checks) | Extra time over noop under Sail |
|---|---|---|
| body-early / body-late, ironkv `host_impl_v` | 1 (12/24, 1/24) | +2.5 s / +2.5 s |
| body-early / body-late, vest `asn1::der_ord` | 1 (147/148, 3/148) | +0.5 s / +0.5 s (cheap module) |
| body-early / body-late, capybarakv `kv2::rwkv_v` | 1 (29/42, 1/42) | +1.0 s / +0.8 s |
| body-early / body-late, anvil `resource_match` | 1 (1/9, 2/9) | +66 s / +66 s |
| comment-top, all projects | 0 (only `;;` comments differ) | 0 |
| lemma-mid, vest | 3: `der_ord` (1/149), `cbor::format` (25/25), `combinators::recursive::exec` (6/6) | +12.6 s |
| spec-wide, ironkv / capybarakv / vest / verismo / anvil | 10 / 26 / 8 / 14 / 39 of 35 / 111 / 298 / 183 / 45 | +4.8 / +14 / +1.2 / +0.3 / +87 s |
| upstream-body, verismo (`verismo_tspec`) | 0 in `verismo`, all of `verismo_tspec` re-run by Verus | +21 s (two crates' frontend) |

The anvil row shows the replay cost. The changed function is the
`resource_match` bucket's first check (1 of 9 checks re-solved in
`body-early`, 2 in `body-late`), yet both edits cost 66 s. Z3 solves the
bucket's earlier checks again while it replays the prefix (`sail-shim`
`start_solver` writes the whole input so far). The ironkv rows show the
same: an edit to the bucket's last check costs the same 2.5 s as one in
the middle.

ironkv has one bucket that misses on every run, stock and fork, with
identical input (same SHA-256). One reply is `unknown`, which the shim never
serves (ADR 0009 policy), so that bucket re-solves 141 replies each time.
Its cost was not separated from the rest of ironkv's 4.8 s floor.

## Hypotheses

| # | Hypothesis | Verdict | Measured cost |
|---|---|---|---|
| 1 | Amdahl ceiling: Cargo, rustc and the Verus frontend remain | **holds** | all-hits ceiling 1.45× (verismo), 1.6× (vest), 1.8× (ironkv), 2.3× (capybarakv), 3.2× (anvil) |
| 2 | Prefix-bound keys: the edited bucket hits up to the first changed byte | **holds for keys, wrong for cost** | the hits happen, but a miss replays and re-solves the whole bucket prefix; edit position does not matter; one bucket costs 0.5–66 s |
| 3 | Line shifts change query bytes | **no** for comments and blank lines (only `;;` comments change; 0 misses); **yes** for inserted items | crate-wide `anonymous_closure%N` numbering shifts; vest lemma-mid +12.6 s |
| 4 | VI-0001/VI-0015 nondeterminism | **holds for VI-0001 on stock; VI-0015 not seen** | 0–9 buckets per unchanged rerun; 0–0.8 s typical, up to 8 s on capybarakv; 0 on the fork |
| 5 | Sail overhead at large query counts | **small** | cold within ±3% of plain (0.97–1.01×); `--incremental` run from a package directory without `-p` is far worse (0.5–0.7×, issue #157) |
| 6 | Upstream-crate edits re-run downstream crates | **holds** | verismo: upstream edit 58 s plain, 44–46 s Sail; the downstream crate's frontend (about 20 s) is paid with every hit |

## Nondeterminism seen

For the determinism-audit lane. SMT logs from 3 unchanged runs per project
were compared (`;;` comment lines removed), plus every edit run:

- **Stock:** every difference was VI-0001, the order of
  `fuel%…arrow_<field>` declarations. Examples: ironkv
  `environment_t.impl&%0.arrow_r` against `arrow_s`; capybarakv
  `kv2.keys.inv_v.impl&%1.arrow_k` against `arrow_rm`, and `testkv_v`
  `impl&%323.arrow_f0` against `arrow_f3`; vest `asn1.integer.impl&%7.arrow_v`
  against `arrow_raw`; verismo `arch.memop.impl&%0.arrow_2` against
  `arrow_0`. Each appeared before the bucket's first check. VI-0015
  (trigger choice) did not appear in these projects.
- **Fork (`13a1176c`):** 0 differing files in every unchanged rerun of all
  five projects (35 to 806 SMT files each), and 0 extra misses in the timed
  fork runs beyond ironkv's `unknown` bucket. No remaining fork
  nondeterminism was observed.
- **Not nondeterminism, but cache-hostile:** inserting an item renumbers
  `TYPE%anonymous_closure%N` crate-wide. This is a stable counter, the same
  on every run, but it changes every later closure's ID in other modules.
  The vest diff (fork, `cbor__format.smt2`, first difference before check 0
  of 25):

  ```
    (declare-fun TYPE%tuple%5. (Dcr Type Dcr Type Dcr Type Dcr Type Dcr Type) Type)
  - (declare-fun TYPE%anonymous_closure%8848. (Dcr Type Dcr Type Dcr Type) Type)
  - (declare-fun TYPE%anonymous_closure%8866. (Dcr Type Dcr Type) Type)
  + (declare-fun TYPE%anonymous_closure%8849. (Dcr Type Dcr Type Dcr Type) Type)
  + (declare-fun TYPE%anonymous_closure%8867. (Dcr Type Dcr Type) Type)
  ```

## Comment lines and line shifts (vest `der_ord`, capybarakv `rwkv`)

Fork runs, a comment line inserted at line 1 of the file. The SMT logs
differ only in `;;` comment lines (296 such lines in vest's `der_ord`
bucket, 84 in capybarakv's `rwkv_v`). There are 0 differing non-comment
lines out of 97,079 and 52,134. The first difference:

```
;; Function-Def vest_lib::asn1::der_ord::DerOrd::der_leq
- ;; vest_lib/src/asn1/der_ord.rs:131:5: 131:57 (#0)
+ ;; vest_lib/src/asn1/der_ord.rs:132:5: 132:57 (#0)
  (push)
   (get-info :all-statistics)
```

```
;; Function-Def capybarakv::kv2::rwkv_v::NoopPerm::apply
- ;; src/kv2/rwkv_v.rs:1268:11: 1268:166 (#0)
+ ;; src/kv2/rwkv_v.rs:1269:11: 1269:166 (#0)
  (push)
   (get-info :all-statistics)
```

**Classification:** source positions in comments only. No span-derived
names or labels, counters or ordering changed. In the timed runs this edit
had 0 extra misses in every project, so these comments do not reach the
cache keys. A line shift caused by a new *item*, rather than a comment,
does change bytes, through the closure counter above.

## Smaller buckets: `-V spinoff-all`

Stock Verus can give every function its own Z3 process. The edit loops were
re-run with `-V spinoff-all` for every mode: ironkv for 3 rounds, and
capybarakv and vest for 2 rounds each (plain against fork Sail). Wall seconds,
median:

| | plain, default | plain, spinoff-all | fork Sail, default | fork Sail, spinoff-all |
|---|---:|---:|---:|---:|
| ironkv noop / body-early / body-late | 9.9 | 12.4 | 4.8 / 7.4 / 7.3 | 5.5 / 5.5 / 5.6 |
| capybarakv noop / body-early | 28.6 / 29.9 | 36.1 / 36.4 | 12.2 / 13.2 | 20.2 / 20.1 |
| vest noop / body-early / lemma-mid | 45.0 / 45.4 / 45.5 | 89.6 / 89.7 / 90.0 | 27.7 / 28.3 / 40.3 | 48.8 / 48.6 / 62.2 |
| cold (ironkv, capybarakv, vest) | 46, 66, 84 | 65, 91, 144 | 48, 66, 86 | 68, 94, 152 |

With one function per process, an edit costs no more than the all-hits
floor (ironkv body edits 5.5 s, the same as noop). But each process
re-declares its module's context, so the floor itself rises. It rises a
little on ironkv and by 1.7–1.8× on capybarakv and vest. Plain Verus also
gets 1.25–2× slower. In absolute terms, spinoff-all only helps ironkv's body
edits (7.4 s to 5.5 s). It also changes a verdict: under spinoff-all,
ironkv's `lemma_serialization_is_not_a_prefix_of` (`marshal_v.rs:930`)
exceeds its rlimit, in plain and Sail alike. Stock nondeterminism also gets
worse with per-function processes: 21–45 missed sessions per unchanged
ironkv run, against 2 with the fork. The lever is to bound replay cost
without paying a full context per function: for example, split only large
or expensive buckets, or keep a fork-side checkpoint of each bucket's
context.

## Levers, ranked by estimated gain on these projects

1. **Make a miss cost only the changed check, not the bucket.** This is the
   largest lever on solver-heavy projects. Today anvil's one-function edit
   costs 104 s, against a 38 s floor. Bringing body edits down to the
   all-hits floor would make them 2.7× faster on anvil (104 s → 38 s), 1.5×
   on ironkv (7.3 s → 4.8 s) and 1.15× on verismo (28.3 s → 24.6 s). Options:
   - smaller buckets. Stock's `-V spinoff-all`, which gives every function
     its own Z3 process, is not that lever as it stands (see below): edits
     cost no more than the floor, but the floor itself rises;
   - an opt-in history-tolerant mode: v1 (stack-only) keys, with replay
     that sends the declarations and the current stack but skips earlier
     checks.

   *Correctness:* both change the solver history relative to stock Verus.
   ADR 0008 measured identical answers, but `rlimit-count` and model bytes
   differ. Under the charter this must be a named opt-in mode with its own
   ADR, a `docs/divergences.md` entry and separate keys. Alternatively, now
   that the fork becomes the default, the fork's own bucketing becomes the
   reference, and its conformance must be re-established.
2. **Fix `--incremental` rebuilding and re-verifying vstd** when it runs
   from a package directory without `-p` (issue #157). This would make
   managed runs 2.2–3.4× faster on vest, capybarakv and verismo (63 s, 45 s
   and 70 s against 28 s, 13 s and 25 s with `sail verify`). *Correctness:* none; build reuse only.
3. **The warm Verus frontend (M8).** The all-hits floor is 5–38 s, mostly
   rustc, VIR, and verify-crate's non-solver work. Halving it would roughly
   double the all-hits ratio (vest 1.6× → 3.2×, verismo 1.45× → 2.9×).
   *Correctness:* a cached frontend must be invalidated exactly. This is
   the hardest of these levers (roadmap item M8; its M8.0 gate failed on
   the reference host, see `docs/ROADMAP.md`).
4. **Stable crate-wide numbering in the fork:** per-module or
   content-derived closure IDs. These would stop item insertions from
   invalidating unrelated buckets (vest lemma-mid 40 s → about 29 s,
   1.14× → 1.6×). *Correctness:* naming only. It must produce output stock
   Verus can produce, or be validated like other fork fixes (ADR 0039).
5. **Default to the fork (VI-0001 fixed).** This removes 0–9 spurious bucket
   misses per run: 0–15% typical, up to 40% on an unlucky run, and it
   removes run-to-run variance. *Correctness:* already planned. The fork
   chooses an order stock can produce, and conformance is required.
6. **`unknown` replies.** One ironkv bucket re-solves 141 replies on every
   run (cost not separated). ADR 0009 policy deliberately re-solves
   unknowns; keep it.

Stack-only keys alone, without history-tolerant replay, would not help.
The hits already happen; the cost is Z3 redoing the bucket's prefix.

## Proposed wording for `docs/performance.md`

For the lead. In claim 2's table cell, replace "vest: 1.3–1.5× (`sail verify`,
about 24 s against 34–38 s)" with:

> vest catalogue edit: 1.3–1.6×. Real projects: 1.2–2.3× for an edit
> confined to one module, about 1.0× for a change to a widely used spec
> function (see the real-project report)

Under "Where Sail does not help", add:

> - **Edits in large modules and widely used definitions.** Verus checks
>   each module in one solver process. When an edit changes any query in a
>   module, the solver re-runs that module's earlier queries as well, so the
>   cost of an edit is the cost of its whole module, wherever the edit is.
>   A change to a spec function used across the crate re-runs most modules,
>   and Sail is then about as fast as plain Verus. Adding a new function or
>   closure can also re-run unrelated modules that use closures. Comments
>   and blank lines cost nothing.
> - **`--incremental` from a package directory.** When `--incremental` runs
>   without `-p` from a package directory inside a workspace, or in a
>   package with local path dependencies, each managed run currently
>   rebuilds Verus's macro crates and verifies vstd again. That is slower
>   than plain Verus (issue #157). Run it from the workspace root with
>   `-p PACKAGE`.

The 1.3–1.5× figure was measured with crates.io vstd and no `--time`. Here,
with vstd patched to the pinned source and `--time-expanded`, vest measured
45 s plain and 28 s with Sail.

## Raw evidence

Raw logs are kept outside the repository, in `l40-real-project.tgz` (sha256
`0e697b2e07001a7b4f572386267aab94b3dbb3db1facc1f0ed48097e0bfe5450`). For
every run it holds `results.tsv`, `table.md`, `header.txt`, each run's log
and provenance, and every `det` table. It also holds:

- the first-difference contexts, with and without `;;` comment lines
  (`det/diffs*/`);
- the `-V spinoff-all` runs (`*-runx/`);
- the local queue logs (`local/`).

The SMT logs themselves are not kept. The issue #157 check on the main
branch is recorded in the issue only: its logs were not packed before the
hosts were terminated.

Reproduce: `cargo build --release -p sail-cli -p sail-shim -p sail-conformance`,
`cargo xtask fetch-verus`, then
`scripts/perf-real.sh run PROJECT 3` and `scripts/perf-real.sh det PROJECT 3`
(set `SAIL_PERF_FORK` to the engine runtime's
`verus/source/target-verus/release` for the fork arm). Run `scripts/perf-real.sh`
with no arguments for usage.
