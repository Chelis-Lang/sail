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
