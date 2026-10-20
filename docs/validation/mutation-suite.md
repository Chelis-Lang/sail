# Mutation suite (#284, ADR 0063)

**Status: built and run once on native Linux, with source-patched Verus
dependencies. Soundness: no false verified in 411 comparisons and the
cross-mutant revert check passes on all four projects. One difference remains
open (#292, a 0.1.0 blocker); the VI-0003 difference is an accepted
divergence (owner, 2026-10-20). Edit loop: from 0.63× (CapybaraKV) to 2.39×
(human-eval) against stock; Vest's 1.12× is accepted as the current number.**
Next: rerun with registry vstd at the runtime's pinned version (what users
get, including the shipped vstd and instant revert) once the benchmark
harness has that mode, and after #292.

## Run

| Field | Value |
|---|---|
| Source | branch `feat/mutation-suite` on main `006ee5d0`; release build (`cargo build --release`) of `sail`, `sail-z3`, `sail-bench` |
| Sail's Verus | runtime `7d829946231d…-81186034d2e137a2` (fork `f8589ed4`) via `SAIL_VERUS_BUILD`, from `engine/verus/artifact-linux.json` |
| Oracle | stock Verus `0.2026.10.18.7d82994` (`cargo xtask fetch-verus`), `RUSTUP_TOOLCHAIN=1.99.1` |
| Host | EC2 c7i.8xlarge (32 vCPU, 61 GiB), us-east-1, native; timed runs alone on the host, one project after another |
| Corpus copies | as conformance prepares them: Verus dependencies patched to the pinned source; human-eval wrapped as a Cargo library |
| Raw evidence | `s3://chelis-sail-native-evidence-010928226848-us-east-1/experiments/2026-10-20/mutation-suite/mutation-suite-2026-10-20.tgz`, sha256 `b026525d607bb6f65980d88ff021159a59c0baba58ae23254966189b6f25b005` (samples, reports, run and oracle logs, kept outputs of every flagged step, the CapybaraKV reproduction logs) |

## Catalogues

Seed `20261020`, generator `gen-mutants 1`. Regenerating on macOS arm64 and
Linux x86-64 gave byte-identical catalogues (digests in the sidecars).
The oracle ran three stock runs per mutant, four workers per project.

| Catalogue | Files | Candidate sites | Generated | Dropped (reason) | Kept | Stock `verified` / `errors` | Timed (per class) |
|---|---:|---:|---:|---|---:|---|---:|
| human-eval | 168 | 10,699 | 54 (6 per class) | 2 (do not compile) | 52 | 13 / 39 | 36 (4) |
| ironkv | 33 | 4,483 | 53 (6; 5 rare-region sites exist) | 3 (do not compile) | 50 | 7 / 43 | 35 (4) |
| vest | 138 | 13,101 | 45 (5) | 2 (do not compile) | 43 | 0 / 43 | 27 (3) |
| capybarakv | 94 | 9,234 | 45 (5) | 2 (one does not compile, one Verus rejects before verification) | 43 | 11 / 32 | 27 (3) |

No mutant's stock outcome varied across its three runs and no run hit the
1,800 s limit. Every kept `errors` mutant fails at least one function the
original verifies (Vest's original already fails `parse_cbor_with_child`,
VI-0032, so all its mutants are `errors`). All nine classes are present in
every catalogue.

## Soundness

Every step compares Sail with the same step's stock run (ADR 0009, ADR 0042).

| Project | Comparisons | Same | Different | False verified | Revert / forced revert (3 rounds) | One daemon per round |
|---|---:|---:|---:|---:|---|---|
| human-eval | 117 | 117 | 0 | 0 | same, own answers, 3/3 each | yes |
| ironkv | 114 | 114 | 0 | 0 (12 harness flags; see below) | same, own answers, 3/3 each | yes |
| vest | 90 | 87 (+3 VI-0003, accepted) | 3 at run time, 0 on recheck | 0 (6 harness flags) | same, own answers, 3/3 each | yes |
| capybarakv | 90 | 2 | 88 | 0 | different (#292); own answers in rounds 1–2 | yes |

- **Cross-mutant check (the check PR #205 never ran).** In every round one
  daemon (one process id) served the cold run and every mutant; the restored
  original then matched stock and Sail's own cold result, both ordinarily and
  with `SAIL_MANAGED_FORCE=1`, where every check went through the solver cache
  (human-eval 4,523 of 4,523 replies served). CapybaraKV's reverts differ from
  stock only by #292's extra vstd document.
- **False verified: none.** The runner first flagged 18 IronKV and Vest steps
  that it had also judged `same`. That was a harness defect: it keyed failing
  functions into a set, and IronKV and Vest have several functions with one
  path (methods of one generic impl). The fixed comparison counts entries per
  name; `sail-bench mutants recheck` over the kept outputs gives no false
  verified in any of the 109 flagged steps.
- **Vest `vest-logic-003` (3 rounds): stock reports a failed `by (bit_vector)`
  function as `success: true`; Sail's fork reports `false`.** Diagnostics,
  error counts and exit codes are identical. This is
  [VI-0003](../verus-issues/VI-0003-bit-vector-failure-reported-as-success.md),
  fixed deliberately in the fork. The owner accepted it as a default-mode
  divergence ([divergences](../divergences.md)); the comparison now
  recognises exactly this case, and `sail-bench mutants recheck` over the
  kept outputs judges all three steps `same` with one accepted divergence
  each. It is Sail being stricter, never a false verified.
- **CapybaraKV: #292.** After every edit `sail verify` re-verifies the patched
  vstd (an extra `verification results:: 2059 verified` line and about 10 s),
  where stock verifies only the edited crate. The project crate's document,
  per-function details and diagnostics equal stock's in all 87 mutant and
  revert steps (three also print the same diagnostics in another order,
  VI-0007). Round 0's cold Sail run failed with `SAIL3103` while rustup
  installed the project's nightly toolchain. Reproduced outside the suite;
  removing `rust-toolchain.toml` does not change it.

## Edit loop

Per mutant: stock `cargo verus verify` against `sail verify`, three rotated
rounds; totals are the ratio of summed per-mutant medians, with 95%
bootstrap intervals over mutants. *Served* is the share of solver replies the
cache served.

| Project | Mutants | Stock median | Sail median | Total speedup [95% CI] | Geomean [95% CI] | Served | Per-round totals |
|---|---:|---:|---:|---|---|---:|---|
| human-eval | 36 | 16.5 s | 4.1 s | **2.39×** [1.70–4.03] | 3.69× [3.26–4.05] | 99% | 2.40×, 2.40×, 2.38× |
| ironkv | 35 | 7.8 s | 5.4 s | **1.49×** [1.36–1.63] | 1.55× [1.41–1.70] | 89% | 1.49×, 1.49×, 1.48× |
| vest | 27 | 34.9 s | 31.2 s | **1.12×** [1.11–1.12] | 1.12× [1.11–1.12] | 97% | 1.12× each |
| capybarakv | 27 | 21.9 s | 34.2 s | **0.63×** [0.61–0.65] | 0.63× [0.61–0.65] | 98% | 0.62×, 0.63×, 0.63× |

| Project | Cold (stock / Sail) | Revert (stock / Sail) |
|---|---|---|
| human-eval | 47.6 / 46.3 s | 17.1 / 4.6 s (3.76×) |
| ironkv | 39.3 / 38.0 s | 8.5 / 4.3 s (1.98×) |
| vest | 65.2 / 62.1 s | 35.0 / 31.2 s (1.12×) |
| capybarakv | 55.2 / 53.5 s | 23.9 / 34.1 s (0.70×) |

Per-class rows are in each project's `report.md` in the evidence archive.
What they show:

- **The solver cache serves nearly everything, and that is not enough on the
  larger crates.** Even where 97–99% of replies are served, a Vest mutant
  costs Sail 31 s against stock's 35 s: the frontend and compilation of
  `vest_lib` dominate. This is the edit-loop floor the ROADMAP's all-hits row
  describes, now measured over 27 realistic edits rather than one.
- **Slow failing queries cap human-eval's total.** Two mutants
  (`logic-002`, `spec-weaken-002`) take 60 s and 101 s on both sides: their
  new failing query is solved fresh. They pull the total from the 3.7×
  geomean down to 2.39× and widen its interval.
- **No step was restored.** Instant revert's qualified profile is Vest's
  public crates.io dependencies; these copies patch Verus dependencies to the
  pinned source (as conformance does), so every revert went through the
  solver cache. The suite does not measure instant revert on these layouts.
- **Rotation.** Per-round totals agree to within 1%, so mutant order and arm
  order do not move the result.

## Limits

- One native Linux host, one run of three rounds; no macOS numbers.
- Per-class rows have 3–4 mutants each; their intervals are wide where one
  mutant is slow.
- The timed subsets are the first 3–4 mutants of each class; the full
  catalogues (188 mutants) ran only through the oracle.
- human-eval is not independent evidence: Sail has been measured on it.

## Attempts

- 2026-10-20, `006ee5d0` + branch: first oracle trial with four workers
  dropped verified mutants as "not rebuilt": Cargo keeps an unchanged crate
  that last built successfully fresh even after a rewrite or `touch`. Fixed by
  `cargo clean -p` before each stock run.
- 2026-10-20: the first runner trial judged reverts against Sail's cold run
  including the dependency documents the cold run alone prints; fixed to
  compare the package's documents.
