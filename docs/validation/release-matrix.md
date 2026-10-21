# Combined release matrix

**Status:** baseline measured; final corrected candidate is still being qualified.
The release is not accepted while known cold losses remain.

The frozen combined source is `5636fa3e`, runtime f8589ed4 based on Verus
7d82994, Rust/Cargo 1.99.1. Three rotated rounds use the installed archive,
registry vstd setup and stock as functional oracle. The Linux CLI SHA-256 is
`3c5b4267ab5c1a401dcdbe2be3bdc685baf4a1de9bd8b6553657e315b10f796f`.
The Mac artifact identity is recorded in its matrix header.

Both baseline matrices have 27 valid scripted scenario rows and four separate
mutation rows still missing. No scripted comparison has a functional mismatch.
The report exits nonzero for missing cohorts and slower rows. Do not combine
mutation data from another binary to clear that result.

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
