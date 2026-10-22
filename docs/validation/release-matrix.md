# Combined release matrix

**Status:** baseline measured; final corrected candidate is still being qualified.
The release is not accepted while known cold losses remain.

The frozen combined source is `5636fa3e`, runtime f8589ed4 based on Verus
7d82994, Rust/Cargo 1.99.1. Three rotated rounds use the installed archive,
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
