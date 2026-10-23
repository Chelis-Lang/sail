# Realistic edit-loop suite (#284, ADR 0063)

**Status:** corrected implementation and native gates pass. Full qualification
of the frozen installed Linux/macOS candidate is running; no final release
edit-loop claim is made yet.

Generator version 2 covers all nine classes. Each catalogue is qualified by
three stock runs per mutant, with root-only logging, pinned Cargo 1.99.1 and
registry vstd setup. Qualification is platform-specific:

| Project | Linux kept | macOS kept |
|---|---:|---:|
| HumanEval | 52 | 52 |
| IronKV | 48 | 48 |
| Vest | 52 | 52 |
| CapybaraKV | 53 | 53 |

The runner measures every qualified mutant in three rotated rounds, using one
observed daemon per round across all mutants and both original-source reverts.
Function comparisons preserve repeated entries. False verified, timeouts,
unexpected diagnostics, changed daemon identity and failed revert provenance
remain failures. The approved VI-0003 exception matches only the reproduced
function, assertion and conservative verdict direction. Returned source must
recover its own results; solver hits do not imply retained-proof restoration.

## Completed frozen candidate cohorts

Native Linux c7i.8xlarge/us-east-1, frozen source `5636fa3e`, installed CLI
`3c5b4267ab5c1a401dcdbe2be3bdc685baf4a1de9bd8b6553657e315b10f796f`,
runtime f8589ed4 on upstream 7d82994:

| Project | Mutants × rounds | Matching comparisons | Full loop speedup by round |
|---|---:|---:|---|
| HumanEval | 52 × 3 | 165/165 | 2.67×, 2.70×, 2.72× |
| IronKV | 48 × 3 | 153/153 | 1.32×, 1.32×, 1.32× |
| CapybaraKV | 53 × 3 | 168/168 | 1.93×, 1.92×, 1.92× |
| Vest | 52 × 3 | 165/165 | 1.12×, 1.12×, 1.12× |

No false verified result, infrastructure failure or unstable result occurs in
these completed cohorts. Every round keeps one daemon; each revert recovers
its own results. Reverts use the solver cache, with no retained-proof restore.
All 651 Linux comparisons match. Vest retains the same default-limit proof
failure in both arms; it measures the failed-proof edit loop. Mac and the
other corrected cohorts remain pending. Later performance changes require
cohorts bound to their own installed binary; these results cannot clear another artifact's
release matrix.

The combined baseline matrix accepts all four cohorts: 31/31 rows have valid
functional comparisons. Six IronKV edits are individually slower despite its
aggregate gain ([#330](https://github.com/Chelis-Lang/sail/issues/330)); these
are rechecked below.

Raw reports, headers, samples and logs in the S3 evidence bucket:

- `experiments/2026-10-21/human-eval-final-3c.tgz`, SHA-256
  `598355353ebd6557fc34c1ef9700bd82f47022108ce03597f8f373ce4635891d`.
- `experiments/2026-10-21/ironkv-final-3c.tgz`, SHA-256
  `c7aef01b16195abd7f8788cb49721e8812becc26428b9a03a0991ceed679103f`.
- `experiments/2026-10-21/capybara-final-3c.tgz`, SHA-256
  `2e65545ff43835351a813d8da36eb96f813032e8b60e07b788f697d49c94144a`.
- `experiments/2026-10-21/vest-final-3c.tgz`, SHA-256
  `0d0a7d916f1ee5c605f2ca37cb7c3d2ee4c0f1d8b0b4b83bb96e81aed244f690`.

## Corrected IronKV candidate

The corrected source `36d90083`, installed CLI SHA-256
`4e0a31dbd78ee69dbfb14230f2f52100d42a7f8534a3faeaea5885c98c244df4`,
passes all 153 comparisons on native c7i.8xlarge/us-east-1 with the same
runtime, catalogue and three rotated rounds. Full-loop gains are 1.42×,
1.43× and 1.44×. The six larger baseline losses no longer reproduce. Only
`statement-001` and `logic-002` have slower medians, by 0.083 s and 0.064 s;
their paired rounds are mixed. No baseline or mutant was removed (#330).

Raw header, samples and report:
`experiments/2026-10-22/ironkv-corrected.tgz`, SHA-256
`227758fc58d103abdb1e920f5eb4d5bdc0db45ec4cea812bb30e81bda4e7b288`.
The other corrected cohorts and combined Mac suite remain pending.

## Completed installed controls

- **CapybaraKV correction:** runtime f8589ed4 on upstream 7d82994, native
  c7i.8xlarge/us-east-1, CLI `3e00b59f…`. All 168 comparisons across 53
  mutations and three rounds pass, with no false verified. Edit totals are
  1.93× faster than stock in every round (about 1,272 → 659 s); cold medians
  56.7 → 55.7 s. Ordinary/forced reverts use the solver cache, not retained
  proofs. This qualifies the rebuild correction, not the later frozen binary.
  [Correction PR](https://github.com/Chelis-Lang/sail/pull/321).
- **HumanEval smoke:** cold, mutation and both revert comparisons pass with
  one daemon. Its `path` identifiers fall outside retained source admission,
  so its cache hits do not qualify instant revert.
- **Vest preliminary complete round:** all 52 mutations and both reverts
  match stock. The run was stopped after that round to restart with the frozen
  installed candidate. Its default-limit baseline has stock's known VI-0032
  proof failure. These are failed-proof edit loops; the passing resource-100
  profile and shipped/retained coexistence are separate acceptance controls.

Composed native workspace, solver, full e2e including extra, PR/full-fast and
standard conformance pass. Standard includes all four 159-unit corpus arms
and 18 HumanEval mutants with both reverts. The composed Mac host gate passes
946 tests, 41 skipped, Clippy clean. Final candidate measurements remain pending.

## Indexed earlier attempts

- 2026-10-20, branch on `006ee5d0`: first oracle kept successful Cargo targets
  fresh instead of re-verifying; corrected with targeted cleaning per stock run.
- 2026-10-20: first runner compared cold-only dependency documents during a
  revert; corrected to compare the requested package documents.
- 2026-10-20: the original 411 comparisons used source-patched vstd, dependency
  logging and stripped timing notes. Their tables are unqualified for release
  claims. Retained raw archive SHA-256
  `b026525d607bb6f65980d88ff021159a59c0baba58ae23254966189b6f25b005`.
- 2026-10-20: a set-based failed-function comparison misreported repeated
  function names; rechecking retained outputs as multisets finds no false
  verified. Unrelated bit-vector cases remain refused.
- 2026-10-21, CLI `99bada10`: CapybaraKV followed its old nightly while stock
  used 1.99.1 (#312). Both arms now bind the same toolchain explicitly; the
  interrupted CapybaraKV and Vest attempts remain diagnostic only.

Raw completed controls in the S3 evidence bucket:
`experiments/2026-10-21/capybara-unit-plan.tgz`, SHA-256
`8f3bba24e00b68074d68c588bdc949ff9adf39fb8b472d4ce06b468b49eb03a9`;
`experiments/2026-10-21/vest-first-round.tgz`, SHA-256
`2b4a77362a492dd941fb406d91c4a507b402c5a958eda7aaa0368c7bf88e0c8b`.
