# Ordered input projection

**Status:** native workspace, solver, full E2E, PR/full fast and standard
conformance pass; Mac host, paired controls and all eight installed mutation
cohorts pass. Both actual archive walkthroughs and the focused current Mac
comparisons pass functional checks. The remaining setup overhead is open.

The combined crate tree is `a8140a02446d41d39d93ec96dccb07a9d99b3baf`,
based on main `3b0bac19`, with runtime `36d65c3f`.

The shared projection validates the same selected namespace and aliases. Internal
revision comparisons hash that selection directly instead of materializing and
then discarding full snapshot maps. Full-snapshot consumers retain their maps.
Ordered namespace unions and digest iteration avoid repeated tree searches.
Fresh content reads, original bracketing mutation stamps and compiler identity
checks remain the authority; logical revision bytes are retained.

The fresh-read comparison covers both the first and later disjoint package
ranges. Native Linux passes Clippy and all 458 sail-verus tests; the combined
Mac candidate passes Clippy and all 1,056 host tests. All four PR arms pass
33 units / 311 functions; full fast passes 147 / 3,451
and standard passes 159 / 13,166, with no differences, instability or
infrastructure errors. Standard also passes its 21 bounded mutation/control
comparisons. Native workspace has 1,067 tests, solver 42; full E2E includes
all ten phases and local extra. Both actual archives pass installation, shipped first-use proof, unchanged
zero-verifier reuse, passing/failing edits, return and the real MCP client with
18 tools. The Mac walkthrough moves the source SDK aside; its bundled runtime
remains usable. Rust 1.99.1 with rust-src is already installed for these checks.

## Installed mutation controls

Both normal factory installations complete one full functional round of each
catalogue, with one daemon across all mutants and both ordinary and forced
returns to the original. All 217 comparisons per platform match; there are no
false verified results, unexpected oracle outcomes or infrastructure failures.
Mac's refreshed three-run stock oracles retain every selected case and outcome.

| Catalogue | Linux comparisons | Mac comparisons |
|---|---:|---:|
| HumanEval | 55 | 55 |
| IronKV | 51 | 51 |
| Vest | 55 | 55 |
| CapybaraKV | 56 | 56 |

This is functional breadth, not a new three-round performance campaign.
Vest's default-limit cohort preserves its known failing original; the passing
paired and cold controls below use limit 100. The exported Linux archive's
application and engine file hashes match the mutation installation; its
dependency proof was independently recorded by the normal factory.

## Paired unchanged calls

Native hosts with no competing test/build jobs: EC2 c7i.8xlarge, us-east-1,
and mac-m4.metal, us-east-1c,
macOS 27.0. Verifier Rust/Cargo 1.99.1, fork runtime `36d65c3f`. Baseline Sail is
`237c7d8d`; the candidate adds the input/runner patch, plus the separately
qualified watch correction on Mac. The frozen code patch and raw results are
identified in the evidence below. Both variants use stripped release binaries,
the same workspace and separate proof contexts. Each establishes its own proof
before eleven alternating unchanged-call pairs. Every measured call is Current
with zero verifier invocations and the same function outcomes.

| Workload | Baseline median | Candidate median | Scope |
|---|---:|---:|---|
| Linux Anvil unchanged | 1.713 s | 1.647 s | About 4% reduction |
| Mac Anvil unchanged | 1.508 s | 1.457 s | About 3% reduction |
| Mac Vest unchanged | 0.802 s | 0.797 s | Essentially unchanged; passing limit-100 workload |

Anvil selects `vdeployment_controller` with
`--output json --lib -- --verify-module vdeployment_controller`; it remains
an exact partial scope. Vest uses `-p vest_lib -- --rlimit 100`.

These paired controls use one warmed project and Cargo home. They are not
substitutes for the separately frozen full release matrices or their real
mutation-loop performance results.

## Focused current-main Mac cold baseline

Six rotated rounds on the same Mac, using a normal factory installation
of `237c7d8d`: all 48 samples succeed and all four selected rows have valid
functional comparisons. The missing mutation row in the general matrix report
is outside this focused diagnosis; no full matrix acceptance is claimed.

| Workload | Stock median [range] | Sail median [range] |
|---|---:|---:|
| Anvil cold | 99.628 s [99.488–99.927] | 99.957 s [99.882–100.259] |
| Vest cold | 39.057 s [38.685–39.239] | 38.185 s [37.903–38.376] |

All six Anvil pairs add 0.19–0.54 s; all six Vest pairs save 0.35–1.27 s.
Vest cold improves on this baseline; the small Anvil cold gap remains open.
These cold numbers precede the projection change. Recording a loss does not
accept it. The two unchanged rows also remain slower than stock.

## Selected-candidate Mac follow-up

Three rotated rounds on the combined candidate above: all 24 samples succeed
and all four selected rows have valid functional comparisons. These use the
normal factory installation and the same passing limits/scopes as the baseline.

| Workload | Stock median [range] | Sail median [range] |
|---|---:|---:|
| Anvil cold | 99.665 s [99.563–99.917] | 100.039 s [99.702–100.195] |
| Vest cold | 39.116 s [39.059–39.471] | 38.029 s [37.651–38.039] |
| Anvil unchanged | 0.281 s [0.280–0.288] | 1.778 s [1.772–1.791] |
| Vest unchanged | 0.258 s [0.255–0.263] | 1.016 s [1.012–1.038] |

Anvil's three cold pairs still add 0.139–0.374 s; Vest's save 1.077–1.442 s.
Anvil cold and both unchanged rows remain marked for performance review.
The omitted mutation-performance row is outside this focused run; the new
one-round functional cohorts do not substitute for a three-round matrix.
No performance waiver or new release follows from these checks.

## Attempts and limits

- The first Linux archive walkthrough inspected a nonexistent provenance field
  and incorrectly required local-receipt reuse for the shipped proof. Actual
  JSON already reported release dependency-build provenance and one verifier
  call. The corrected walkthrough checks that documented provenance and passes.
- Initial profiling attempts changed the benchmark environment and re-established
  proofs; their timings are excluded from the unchanged comparison.
- The first, smaller ordered-set prototype passed workspace, solver, full E2E
  and PR conformance. Its full-fast run was interrupted when superseded by the
  revision-only candidate; it is not qualified by an incomplete run.
- The Mac factory refused to overwrite the first trial's proof evidence. That
  evidence was retained separately and a fresh factory run passes; the refusal
  is not counted as successful staging.

The baseline CPU sample has no lost samples. File hashing, filesystem calls
and repeated path comparisons remain costs. This change does not resolve all
unchanged or tiny-edit overhead and grants no performance waiver.

Timing inputs, code patches, reports and controls are in the evidence bucket:

- `experiments/2026-10-25/sail-next-setup-linux.tgz`, SHA-256
  `5184955c0ff684a53ba2db08400c68e20faca4c4765045c256fa3805304be862`.
- `experiments/2026-10-25/sail-next-setup-mac.tgz`, SHA-256
  `156f5f40413a7c5b97e29de469883f78413f873db9ae9b5d51e8131da71b12a2`.

Native gate and Linux install logs: `experiments/2026-10-25/sail-next-native-gates-and-install.tgz`,
SHA-256 `ecbc4373a5723c804e0f9e47fa0fb3acb5465dad746f8d4e2df23576978c31c7`.

Current Mac controls and actual install logs:
`experiments/2026-10-25/sail-next-mac-current-and-install.tgz`, SHA-256
`881dbf91f76780fe2c6b44cdbbfe552fbf732fc8cd62dedbdaafae30ad3d8115`.
The private review index lists the eight cohort objects and both checksummed
preview archives: `staging/2026-10-25/remaining-sprint/qualification.json`,
SHA-256 `c6c812e3089aa0a50eed47f3fce3a8abe370dee70f807dfb7c1c5d2063069575`.
