# Conditional build scripts and failed requested proofs

**Status:** native gates and full CapybaraKV mutation qualification pass.
Final combined installed-product release measurements remain pending.

Source `cd6585c5`, native c7i.8xlarge in us-east-1, pinned Cargo/Rust
1.99.1, runtime f8589ed4 on upstream 7d82994:

- Workspace gate, 35 solver tests, full e2e including extra, and PR/full-fast
  conformance pass. All four standard corpus arms have 159 Same, zero
  Different, Unstable or infrastructure errors and no skipped projects.
- The existing managed fixture gains a Windows-only path dependency whose
  build script must never run. The original failing-first, independently
  verified vstd, repeated-failure, source-edit and reuse checks remain intact.
- Unit controls refuse unknown schemas, wrong or duplicate roots, invalid
  dependency indices, unexpected actual units, missing/duplicate completion
  records and malformed Cargo events. Forwarded Verus JSON remains distinct
  from Cargo's scheduling events.
- The composed Mac host gate passes 946 tests, 41 skipped, Clippy clean.

The installed correction's complete CapybaraKV run has 53 mutations across all
nine classes, three rotated rounds and one observed daemon per round. All 168
cold, mutation and revert comparisons pass, with no false verified results.
Edit totals are 1.93× faster than stock in each round (about 1,272 → 659 s).
Cold medians are 56.7 → 55.7 s; ordinary reverts 25.4 → 11.6 s. These are solver
cache reuses, not retained-proof restores. This cohort's CLI hash begins
`3e00b59f`; the final combined release uses a separate measurement cohort.

The plan comes from the current pinned Cargo request, with actual execution
records checked before Current evidence. It exempts only scripts absent from
that plan; observed scripts still require their ordinary content and environment
checks. It neither saves an absence claim across runs nor grants a proof.

Attempts retained:

- 2026-10-21, baseline `791b897d` on `4636ff20`: the first failed proof
  incorrectly reads Currentness Unknown.
- Initial correction: full e2e stops on an old assertion demanding an unchanged
  sibling script rebuild. Its replacement checks the fresh scheduling binding,
  unchanged script execution, one fresh root verifier and vstd reuse.
- `5221b2d2`: overly broad malformed-JSON detection misclassifies forwarded
  Verus output and breaks e2e extra; corrected by `cd6585c5`.

Raw gates: `experiments/2026-10-21/cargo-unit-gates.tgz`, SHA-256
`5e59a494eb33a7aa2c908db1478abde302ea81eac186e0cbe91d8ec8f38078c4`.
Mutation run: `experiments/2026-10-21/capybara-unit-plan.tgz`, SHA-256
`8f3bba24e00b68074d68c588bdc949ff9adf39fb8b472d4ce06b468b49eb03a9`.
Both are in the S3 evidence bucket.
