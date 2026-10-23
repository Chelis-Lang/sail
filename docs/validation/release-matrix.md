# Combined release matrix

**Current status:** all 31 selected comparisons are valid on each frozen
installed platform, including all four full mutation cohorts: 651 matching
comparisons per platform. Performance acceptance remains open for slower
cold and unchanged rows. This is a private review candidate, not an accepted
public release.

Source `cda42861` has the same Rust, Cargo, runtime and build inputs as merged
`e03dfa0b` (#347). Benchmark correction `fc29023a` (#354) does not change the
consumer. `release/0.1.0` freezes this source before later Oyster changes
on main. Runtime `7d9d5410` remains based on upstream `7d82994`, with
Rust/Cargo 1.99.1 and the two manifests in the [runtime record](spec-signature-runtime.md).
Both archives carry exact default/100 proofs and signed solver replies.

| Native platform | Installed CLI SHA-256 |
|---|---|
| Linux x86-64, EC2 c7i.8xlarge, us-east-1, Ubuntu 24.04 | `02646019fdcea4c3c22fab895193ec0412c1d1a789157f29dbc932b90dc3f5be` |
| macOS 27.0 arm64, EC2 mac-m4.metal, us-east-1c | `4f081dad8c04637ef98d280497717d812ada698f47f02d79377c50424d4ca391` |

| Complete mutation loop | Linux speedup | Mac speedup |
|---|---:|---:|
| HumanEval, 52 edits / 165 comparisons | 2.66× | 2.85× |
| IronKV, 48 edits / 153 comparisons | 1.46× | 1.41× |
| Vest, 52 edits / 165 comparisons | 1.11× | 1.40× |
| CapybaraKV, 53 edits / 168 comparisons | 1.94× | 2.32× |

Speedups use the ratio of the three-round median loop totals, as in the
matrix report. Completed cohorts have zero false verified, different, unstable or
infrastructure results. Every ordinary and forced return receives the
original's own answers, with one daemon per round. Vest's existing VI-0003
status exception remains explicit. Individual slower edits stay in the reports;
the aggregate wins do not grant their acceptance.

Linux completes all 27 selected scripted rows: 22 non-Vest rows, four
corrected passing Vest phases and the corrected worktree subset. Every row
has three successful samples per arm. Mac's corrected worktree
completes all three rounds (38.43 → 27.83 s), with successful verification on
both arms. Mac's other 26 scripted rows also complete all three rounds. Each
selected comparison requires the
same binary, runtime, stock verifier, corpus and lock identities.

The earlier default-limit Vest invocation failed on both arms. Those samples
remain failed and cannot qualify passing-workload performance. The main Vest
path now uses limit 100; a separate worktree path initially missed that flag
and also remains invalid in its original run. The corrected worktree subset
uses the same required limit on its baseline and timed checkout, on both arms.
These affected subsets are qualified separately; failed originals are never
dropped or rewritten. The default-limit corpus conformance case stays unchanged.

Linux first use improves 31.21 → 12.77 s. Anvil cold medians overlap
(178.89 → 178.19 s); its body/spec edits improve 155.29 → 85.94 s and
151.40 → 35.85 s. Its unchanged call starts zero verifiers but costs
1.91 s versus stock 0.32 s. Passing Vest cold improves 71.29 → 69.20 s.
Mac Anvil cold is 99.66 → 102.52 s, with nonoverlapping ranges; its unchanged
call is 0.28 → 1.82 s. Mac Vest cold is 38.21 → 38.94 s, with overlapping
ranges. Tiny-call overhead is visible on both platforms. These rows remain
open for performance review; no fixed percentage allowance is applied.

A separate pinned-Cargo diagnostic records stock 99.25 s, bare fork 98.63 s
and Sail 101.48 s on Anvil cold, all successful and Current. Sail's unchanged
call is 1.79 s with zero verifier invocations. Its setup/receipt checks
account for about 1.4 s outside the 99.92 s compiler invocation. This one-round
control attributes cost; it does not replace the release samples or qualify
a different proof profile. Raw `mac-anvil-pinned-diagnostic.tgz`, SHA-256
`ec5f91bcddb9fee4e83a3567b9cae2fe7683ec258c359feceda0f23fde8188c4`,
in the same S3 prefix below. The earlier unpinned diagnostic returned Unknown
and the text-only partial command was refused as specified; neither qualifies
performance.

Current completed raw results in the existing S3 bucket, under
`experiments/2026-10-23/`:

| Run | Archive | SHA-256 |
|---|---|---|
| Linux matrix, including invalid default-limit Vest attempt | `spelling-linux-matrix.tgz` | `eb0918c01c88f2627cb73feae5cdf00c0cde61f009afe399f5fd310c9d5d9186` |
| Linux corrected main Vest phases, original worktree still invalid | `vest-matrix-fixed.tgz` | `bc6e6efd3ccfa2762fcc6a036d1b0ac8585e95959dd248cfd4c96f5eef4d7228` |
| Linux corrected Vest worktree | `vest-worktree.tgz` | `55bd20a2cf39f910d5ca1a3d217d448edb7a845d35cc532663215ebf49f03da8` |
| Linux Vest | `spelling-linux-vest.tgz` | `6c8ac7d5d686fdde589e21734c80eeaa589e5dd38d718bbca6849aa816b4005d` |
| Mac matrix and Vest cohort; original worktree invalid | `spelling-mac-primary.tgz` | `ff5c6aaf82f08ce401d5587b391195c1ba9f8f248b43d762121da2334509fc96` |
| Linux HumanEval | `spelling-linux-human-eval.tgz` | `cafdd72296931248d5f8f8419cb8b344590a129c8bb906c4976dd39a30e3c3c2` |
| Linux IronKV | `spelling-linux-ironkv.tgz` | `02ded2193583e5d92fba287b44d2e17d73a6890c93ef508b682ca54e36058374` |
| Linux CapybaraKV | `spelling-linux-capybarakv.tgz` | `677e28b5024ad0257363fe104bc335d2c5c19779a7fe0f5f23ba6c86267251ba` |
| Mac HumanEval, IronKV and CapybaraKV | `spelling-mac-secondary.tgz` | `ca3a3da6143f9361455fe57a3bb8ea033369e9ded353007eaecbed28d337e026` |
| Mac corrected Vest worktree | `mac-vest-worktree.tgz` | `2369a859ccc131aef4a2619cae950150ca902b5a55599d59c5d502cc5ea5456f` |

The complete selected review reports are `spelling-linux-selected-matrix.md`
(SHA-256 `1ad689e86f33013979ca7a82d7608475df0778d08bdc852ae0c70ca951e78416`)
and `spelling-mac-selected-matrix.md` (SHA-256
`9b8bd133cd536e86b6be44109fa8a9863e881a5e3c832c6160dcfe7722bf1944`)
in that prefix. They retain individual slower edits and identify every original
invalid setup row alongside the selected successful reruns.

## Selected comparisons

Each row has three samples per arm. A valid comparison is not automatic
performance acceptance. Original means the full scripted run; vest/worktree
identify the corrected subsets above. Mutation rows require matching full
consumer identities and every oracle/revert check.

### Native Linux

| Project | Scenario | Stock median [range], s | Sail median [range], s | Speedup | Run | Review |
|---|---|---:|---:|---:|---|---|
| small | cold-first | 31.206 [31.052–31.292] | 12.774 [12.768–12.799] | 2.44× | original | — |
| small | fresh-project | 31.038 [31.017–31.230] | 12.482 [12.460–12.496] | 2.49× | original | — |
| small | revert | 0.736 [0.735–0.742] | 1.052 [1.051–1.054] | 0.70× | original | slower median |
| small | small-edit | 0.746 [0.741–0.747] | 1.099 [1.089–1.101] | 0.68× | original | slower median |
| small | unchanged | 0.112 [0.111–0.113] | 0.403 [0.399–0.410] | 0.28× | original | slower median |
| small | worktree | 30.943 [30.941–31.042] | 12.238 [12.145–12.244] | 2.53× | original | — |
| human-eval | mutation-loop (52 edits) | 1064.944 [1053.513–1074.860] | 400.819 [397.619–403.973] | 2.66× | mutations | slower individual edits |
| vest | cold | 71.291 [70.847–71.473] | 69.199 [69.144–69.310] | 1.03× | vest | — |
| vest | module-edit | 39.762 [39.664–39.792] | 24.066 [23.977–24.190] | 1.65× | vest | — |
| vest | mutation-loop (52 edits) | 1950.681 [1937.552–1956.188] | 1761.667 [1757.737–1780.044] | 1.11× | mutations | slower individual edits |
| vest | spec-wide | 39.772 [39.521–39.850] | 22.487 [22.453–22.592] | 1.77× | vest | — |
| vest | unchanged | 0.214 [0.212–0.214] | 0.961 [0.955–0.962] | 0.22× | vest | slower median |
| vest | worktree | 84.200 [83.562–84.798] | 59.961 [59.532–60.007] | 1.40× | worktree | — |
| ironkv | cold | 39.422 [39.323–39.451] | 20.615 [20.598–20.616] | 1.91× | original | — |
| ironkv | module-edit | 9.163 [9.134–9.175] | 7.096 [7.075–7.166] | 1.29× | original | — |
| ironkv | mutation-loop (48 edits) | 407.066 [407.003–408.696] | 279.351 [279.197–280.157] | 1.46× | mutations | slower individual edits |
| ironkv | spec-wide | 9.070 [9.066–9.104] | 4.724 [4.716–4.745] | 1.92× | original | — |
| ironkv | unchanged | 0.119 [0.117–0.119] | 0.412 [0.408–0.417] | 0.29× | original | slower median |
| capybarakv | cold | 56.564 [56.264–56.568] | 54.262 [53.862–54.281] | 1.04× | original | — |
| capybarakv | module-edit | 25.439 [25.341–25.687] | 12.783 [12.756–12.798] | 1.99× | original | — |
| capybarakv | mutation-loop (53 edits) | 1201.196 [1199.952–1201.230] | 620.244 [620.242–620.652] | 1.94× | mutations | — |
| capybarakv | spec-wide | 25.439 [25.409–25.537] | 11.406 [11.391–11.426] | 2.23× | original | — |
| capybarakv | unchanged | 0.161 [0.156–0.189] | 0.680 [0.680–0.689] | 0.24× | original | slower median |
| anvil | cold | 178.889 [175.924–187.631] | 178.186 [176.618–182.142] | 1.00× | original | — |
| anvil | module-edit | 155.289 [152.992–159.482] | 85.940 [85.535–86.597] | 1.81× | original | — |
| anvil | spec-wide | 151.399 [148.061–159.564] | 35.847 [35.735–36.040] | 4.22× | original | — |
| anvil | unchanged | 0.316 [0.315–0.325] | 1.908 [1.904–1.925] | 0.17× | original | slower median |
| verismo | cold | 81.545 [81.520–81.997] | 77.880 [77.771–77.931] | 1.05× | original | — |
| verismo | module-edit | 30.187 [30.134–30.195] | 23.634 [23.597–23.895] | 1.28× | original | — |
| verismo | spec-wide | 29.999 [29.997–30.197] | 20.756 [20.735–20.757] | 1.45× | original | — |
| verismo | unchanged | 0.170 [0.170–0.170] | 0.602 [0.601–0.605] | 0.28× | original | slower median |

### Native Mac

| Project | Scenario | Stock median [range], s | Sail median [range], s | Speedup | Run | Review |
|---|---|---:|---:|---:|---|---|
| small | cold-first | 16.356 [16.322–16.485] | 10.068 [9.986–10.457] | 1.62× | original | — |
| small | fresh-project | 16.273 [16.157–16.301] | 9.499 [9.471–9.534] | 1.71× | original | — |
| small | revert | 0.452 [0.452–0.453] | 0.850 [0.845–0.857] | 0.53× | original | slower median |
| small | small-edit | 0.454 [0.453–0.455] | 1.129 [1.115–1.132] | 0.40× | original | slower median |
| small | unchanged | 0.215 [0.215–0.217] | 0.547 [0.547–0.555] | 0.39× | original | slower median |
| small | worktree | 16.094 [16.089–16.256] | 9.277 [9.274–9.309] | 1.73× | original | — |
| human-eval | mutation-loop (52 edits) | 624.456 [623.984–624.927] | 219.166 [218.612–219.166] | 2.85× | mutations | slower individual edits |
| vest | cold | 38.206 [38.111–38.507] | 38.944 [37.934–39.098] | 0.98× | original | slower median |
| vest | module-edit | 22.140 [22.114–22.194] | 12.668 [12.403–12.777] | 1.75× | original | — |
| vest | mutation-loop (52 edits) | 1108.571 [1108.229–1108.712] | 793.300 [786.294–794.184] | 1.40× | mutations | — |
| vest | spec-wide | 22.175 [22.163–22.211] | 12.409 [12.066–12.492] | 1.79× | original | — |
| vest | unchanged | 0.259 [0.258–0.264] | 1.001 [1.000–1.010] | 0.26× | original | slower median |
| vest | worktree | 38.430 [38.379–38.573] | 27.826 [27.700–27.974] | 1.38× | worktree | — |
| ironkv | cold | 20.097 [19.947–20.209] | 13.625 [13.439–14.038] | 1.48× | original | — |
| ironkv | module-edit | 4.304 [4.297–4.329] | 3.594 [3.582–3.625] | 1.20× | original | — |
| ironkv | mutation-loop (48 edits) | 195.682 [195.518–195.683] | 138.475 [138.397–138.503] | 1.41× | mutations | slower individual edits |
| ironkv | spec-wide | 4.286 [4.263–4.316] | 2.483 [2.481–2.484] | 1.73× | original | — |
| ironkv | unchanged | 0.208 [0.208–0.224] | 0.538 [0.537–0.539] | 0.39× | original | slower median |
| capybarakv | cold | 31.073 [30.955–31.188] | 30.892 [30.620–31.268] | 1.01× | original | — |
| capybarakv | module-edit | 14.933 [14.899–14.950] | 6.240 [6.217–6.241] | 2.39× | original | — |
| capybarakv | mutation-loop (53 edits) | 768.774 [768.198–768.920] | 330.888 [330.699–330.915] | 2.32× | mutations | — |
| capybarakv | spec-wide | 14.942 [14.902–14.979] | 5.864 [5.819–5.882] | 2.55× | original | — |
| capybarakv | unchanged | 0.229 [0.225–0.241] | 0.758 [0.756–0.760] | 0.30× | original | slower median |
| anvil | cold | 99.658 [99.605–99.820] | 102.519 [101.720–103.253] | 0.97× | original | slower median |
| anvil | module-edit | 79.202 [79.165–79.210] | 42.287 [42.054–42.331] | 1.87× | original | — |
| anvil | spec-wide | 79.286 [79.076–79.344] | 15.894 [15.889–16.017] | 4.99× | original | — |
| anvil | unchanged | 0.283 [0.283–0.290] | 1.820 [1.808–1.825] | 0.16× | original | slower median |
| verismo | cold | 49.259 [48.988–49.413] | 48.533 [48.242–48.621] | 1.01× | original | — |
| verismo | module-edit | 18.931 [18.827–19.008] | 11.704 [11.634–11.731] | 1.62× | original | — |
| verismo | spec-wide | 18.948 [18.813–18.956] | 11.208 [11.179–11.221] | 1.69× | original | — |
| verismo | unchanged | 0.274 [0.269–0.276] | 0.880 [0.879–0.890] | 0.31× | original | slower median |

## Earlier signature-only baseline

The following results belong to earlier consumer binaries and the original
matching-exit benchmark rule. They do not qualify the current combined
consumer or successful-workload performance. Current rows reject matching
failed proofs as well as mismatches.

| Platform | Corrected staged CLI SHA-256 |
|---|---|
| Native Linux x86-64, EC2 c7i.8xlarge, Ubuntu 24.04 | `160ea5037436e7ef57c8fb2e8d3db50c07b41d27cc53cda61cb0ec946b2fb42b` |
| Native macOS 27.0 arm64, EC2 mac-m4.metal | `cf55a904f130baff1dc876636d7d8ec38e532041519ac678265a6fc06b13f5fc` |

Both stages contain freshly recorded exact vstd proofs and signed replies.
Each platform's complete HumanEval (165), IronKV (153), Vest (165) and
CapybaraKV (168) cohorts total **651 matching comparisons**, with zero false
verified, different, unstable or infrastructure results. One daemon serves
each complete sequence; ordinary and forced returns receive the original's
own answers. Vest includes the existing accepted VI-0003 status exception.

| Full mutation-loop speedup | Linux | Mac |
|---|---:|---:|
| HumanEval, 52 edits | 2.62× | 2.84× |
| IronKV, 48 edits | 1.47× | 1.41× |
| Vest, 52 edits | 1.13× | 1.38× |
| CapybaraKV, 53 edits | 1.93× | 2.28× |

Mac cold Anvil is 103.79 s versus stock 99.91 s; Vest is 37.64 s versus
36.77 s. Anvil unchanged is 2.39 s versus 0.28 s. Linux cold Anvil medians
are 183.92 s versus 181.34 s with overlapping sample ranges; its unchanged
call is 2.04 s versus 0.33 s. Tiny edits and other unchanged calls also retain
visible overhead. These losses are open, not accepted allowances.

Complete reports in the existing S3 evidence bucket:
`experiments/2026-10-23/signature-linux-complete-matrix.md` and
`experiments/2026-10-23/signature-mac-complete-matrix.md`. Raw matrix/cohort
reports, headers, samples and logs are in the same prefix, named
`signature-linux-matrix.tgz`, `signature-linux-{human-eval,ironkv,vest,capybarakv}.tgz`,
`signature-mac-primary.tgz` (matrix, HumanEval and IronKV) and
`signature-mac-secondary.tgz` (Vest and CapybaraKV). They preserve every
sample, setup adjustment, expected failure and individual slower mutation.

Successful public Vest edit/undo controls on both manifests restore the
original complete proof with zero verifier invocations. The failing Vest
corpus pin remains a failing project with both stock and Sail. See
[runtime qualification](spec-signature-runtime.md) and
[performance](../performance.md) for scope and remaining costs.

## Previous unaccepted fork-f8589ed4 measurements

The results below belong to the preceding binaries and do not qualify the
corrected candidate. That Linux matrix completed all 31 rows and 651 matching
mutation comparisons. Its Mac counterpart has 28/31 valid rows: HumanEval has
three functional disagreements, and IronKV/CapybaraKV did not run. Complete
reports are `experiments/2026-10-23/selected-linux-complete-matrix.md` and
`experiments/2026-10-23/selected-mac-incomplete-matrix.md`; the failed original
Mac cohort remains preserved. Anvil cold/setup losses remain tracked by #314.

**Status:** both final installed scripted matrices are complete. The Mac
HumanEval mutation cohort fails: 162 Same / three functional disagreements
([#337](https://github.com/Chelis-Lang/sail/issues/337)). Remaining mutation
cohorts and the release performance decision remain open.

The selected binaries combine the retention-budget correction (#334) and
exact raised-limit profile consumer (#335), with runtime f8589ed4 on Verus
7d82994 and Rust/Cargo 1.99.1:

| Native host | Installed CLI SHA-256 |
|---|---|
| EC2 c7i.8xlarge, us-east-1, Ubuntu 24.04 | `573cd8a37722c2b507586e5de4c461a038dcb750ac60b0d60dcc1e397ad0c71f` |
| EC2 mac-m4.metal, us-east-1c, macOS 27.0 | `8fb73ac60ac70d941e1f9c860913049edbd1739d2b0e3f0cc6281f8fbc15e33b` |

The Mac failure changes `exists_even_index`'s initial length test from zero to
one. Stock verifies the mutant; Sail rejects its postcondition in all three
rounds. A fresh single-mutant sequence also reproduces it. Bare-fork controls reproduce it without Sail or its cache. The
[spec-formal context record](../verus-issues/VI-0034-spec-formal-solver-context.md)
identifies the input-context sensitivity and the bounded Mac compiler correction;
that defect is fixed by #338; the failed cohort, catalogue and exact output
are preserved in `experiments/2026-10-23/mac-human-constant004-failure.tgz`,
SHA-256 `369de45697d6255cf9d12e30326aec32477fa1771a4ed8820bfe7fd81431c1f7`.
The comparison remains strict and the mutant remains in the catalogue.

Each platform requires all 27 scripted rows and four full three-round mutation
cohorts from its own selected binary. The Mac stock catalogues were separately
qualified with three fresh stock runs per mutant. Native full workspace,
solver, E2E, fast and standard conformance pass; the selected Mac consumer
passes its 963-test host gate and shipped-proof/undo controls. Linux README
and real Claude Code MCP pass. Final deep checks and Mac walkthrough are in
progress. See [raised profiles](rlimit-subsumption.md) for the independent
producer controls and native gate logs.

- Incomplete Mac matrix, 2026-10-22, selected CLI `8fb73ac6…15e33b`:
  23 scripted rows valid; four Verismo rows could not run because CMake and
  Ninja were absent. No release acceptance. Prerequisites installed before
  restarting the complete matrix. Raw `experiments/2026-10-22/final-mac-matrix.tgz`,
  SHA-256 `8b12f4487a313b89246ee28f2b505c988d04f83bd98005204bffe45822d3e9e9`.

## Prior qualifications

The 2026-10-22 tighter-retention-budget IronKV run has 153 matching
comparisons, zero differences, false verified results, unstable cases or
infrastructure failures. Its complete edit loop is 1.48× faster, versus
1.31× before. The six larger losses in #330 are removed; two medians retain
0.03 and 0.10 s overhead on roughly 9–10 s runs. Undo uses ordinary solver
caching (9.1 → 4.7 s). One daemon serves each complete sequence, and all
ordinary and forced returns receive the original's own answers. Raw report,
header and every sample: `experiments/2026-10-22/cost-full-iron.tgz`, SHA-256
`9dd66b6962d76eba56c3e02f36740893be75794c2b8b1b2abaae8dfa27e82650`.
The preceding 1.43× result accidentally disabled fresh-target retention and
cannot qualify the corrected product.

- Interrupted Mac matrix, 2026-10-22, source `940dbeed` (installed CLI
  `1faa7c1fc72c4129dd41313bf9f164633155f55491bf1fc96bc2949047e951e0`):
  disk exhaustion stopped the run after two rounds. This is not the required
  three-round acceptance. Preserved logs and partial samples:
  `experiments/2026-10-22/mac-qualification-interrupted.tgz`, SHA-256
  `6c9d81b21d3629dab7a3fe701eae051450532942baedc7ff5f472fda452af154`.
  Completed benchmark caches are now discarded by #333.

The tighter retention-budget candidate is based on `ff63d032`, with installed
Linux CLI SHA-256
`06cf3abe148a416afc09e42b8578f6d927320d66e6bb90f81943fdcdd32945f5`.
The focused three-round IronKV check has 12 matching comparisons. The actual
retry-mutation control passes. Successful public Vest at resource limit 100
serves the released vstd and restores its original proof on undo with zero
verifier invocations (40.05 → 1.98 s). Workspace, solver and full E2E gates
pass; full FAST passes in all four arms (147 matching units each), and full
STANDARD passes in all four arms (159 matching units, 13,166 functions each;
zero differences, unstable cases or infrastructure failures). The other three
full mutation cohorts and the matrix remain in progress. Raw gate logs:
`experiments/2026-10-22/cost-gate-results.tgz`, SHA-256
`34c36128c9d31dde6fddc0e5f0a923242dbc8fda0aaad9d0d741b05e74709ba1`.
The combined Mac consumer also preserves successful public Vest undo with
zero verifier invocations (22.58 → 1.88 s); its separately recorded exact
resource-limit profile is qualified independently. These controls do not
establish final performance acceptance.

The completed baseline used frozen source `5636fa3e`, runtime f8589ed4 based
on Verus 7d82994 and Rust/Cargo 1.99.1. Three rotated rounds use the installed archive,
registry vstd setup and stock as functional oracle. The Linux CLI SHA-256 is
`3c5b4267ab5c1a401dcdbe2be3bdc685baf4a1de9bd8b6553657e315b10f796f`.
The Mac artifact identity is recorded in its matrix header.

The completed Linux baseline has 31/31 valid rows after attaching all four
full mutation cohorts from the same binary. Its report exits 2 for slower
rows requiring review, including six individual IronKV edits (#330). The Mac
baseline has 27 valid scripted rows and four mutation rows pending. No scripted
comparison has a functional mismatch. Do not attach cohorts from another
binary to clear missing results.

Linux cold medians improve across all five real projects. Mac cold CapybaraKV,
Vest and Anvil are slower. The later early-resolution refusal, native-producer
preflight, first-recheck header inventory and fresh-target cleanup corrections
are being measured separately; the baseline does not qualify them.

Results and remaining costs are summarized in [performance](../performance.md).
The installed Linux baseline passes all six native local gates. The Mac host
gate passes; final source and final mutation acceptance remain separate.

Raw reports, headers, samples and logs in the S3 evidence bucket:

- `experiments/2026-10-21/matrix-linux-3c.tgz`, SHA-256
  `0e67449dbab981f9494480f8a3bc69664e282e8fa50a42ea4d2a8a5e0ce3552c`.
- `experiments/2026-10-21/matrix-macos-baseline.tgz`, SHA-256
  `c97285b0ae53e0c39a5fb5c6bbf973bc6169c04e2eeff175ac63e82fcd8ae191`.

The four-arm deep corpus run on this source has 159 Same, zero Different,
zero unstable and zero infrastructure errors per arm (13,166 functions). Two
explicit optional Singular projects are skipped. Its raw report and log:
`experiments/2026-10-21/deep-corpus-final.tgz`, SHA-256
`07dcbae8a56740d12fa37734e5e4c559c8751029ccb4d54b03e4895bbab586d6`.
This corpus result alone does not complete the separate full mutation tier.

- Interrupted intermediate source `8080730d`, 2026-10-21: workspace, stage,
  solver, full e2e and PR/full-fast passed; standard was stopped to qualify the
  newer corrected source. This is not a completed standard qualification.

The corrected Mac shipped/retained coexistence control (`d3f60a51`, CLI
`55fa686b012737e2c45185f8f0a22d486c7390f1304a59bdeb94e91e53d939f4`)
passes cold, edit, reverted and unchanged public Vest at resource limit 100.
The returned proof restores its original establishing run with no verifier
invocation. Cold is 39.17 → 33.74 s, edit 22.52 → 15.79 s and return
22.54 → 1.97 s (stock → Sail). This is one functional control, not a
three-round performance claim. The corrected candidate also passes the
clean README new/verify/reuse walkthrough and real Claude Code MCP listing.
Raw control, scripts, outputs and host log:
`experiments/2026-10-22/mac-proof-coexistence.tgz`, SHA-256
`a4726b493060707597e8402776cf2c0e9782c957159b94995f9344b9bb9930a9`.

- `cda42861`, 2026-10-23, Linux scripted Vest attempt: invalid as a passing
  workload; both arms failed at the default limit (VI-0032). The corrected
  invocation uses `--rlimit 100` equally on both arms; matching failed samples
  remain recorded and now fail the performance-validity check.
