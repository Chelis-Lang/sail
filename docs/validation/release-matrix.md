# Combined release matrix

**Current status:** the corrected fork-7d9d5410 candidate is undergoing full
native Linux and Mac release measurements. Each platform requires its own
27 scripted rows and four complete three-round mutation cohorts from the
same installed consumer. The release performance decision remains open.

| Platform | Corrected staged CLI SHA-256 |
|---|---|
| Native Linux x86-64, EC2 c7i.8xlarge, Ubuntu 24.04 | `160ea5037436e7ef57c8fb2e8d3db50c07b41d27cc53cda61cb0ec946b2fb42b` |
| Native macOS 27.0 arm64, EC2 mac-m4.metal | `cf55a904f130baff1dc876636d7d8ec38e532041519ac678265a6fc06b13f5fc` |

Both stages include freshly recorded exact vstd proofs and signed replies for
the rebuilt runtime. The installed Mac candidate passes all 165 comparisons in the complete
three-round HumanEval cohort, including `human-eval-constant-004`. Mac and
Linux IronKV pass all 153 comparisons each. Remaining cohorts are running. Public successful Vest edit/undo
controls pass on both platforms, including restoration of the original proof
with zero verifier invocations. Their elapsed times are diagnostic controls,
not release performance measurements. See [runtime qualification](spec-signature-runtime.md).

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
full qualification remains pending. The failed cohort, catalogue and exact output
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
