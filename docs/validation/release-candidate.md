# Release-candidate validation

**Current status:** `0.1.0-rc.7` passes every functional qualification gate
on native Linux and macOS. One installer defect found during qualification
(#591) is fixed on main after the candidate commit (#593). The candidate is a
GitHub draft with attested archives; nothing is published. Performance is in
[performance](../performance.md). The owner accepted its slower rows for
0.1.0 as known issues.

Candidate source `8797ccd4` (on main), fork runtime `47fb3be6` (Verus
0.2026.10.18.7d82994, Rust 1.99.1), CI image `sail-ci:587662778c7e8f88-core`.
Its Rust sources equal `1415ff47`; the later commits change only docs, README
and third-party notices. All runs are from 2026-10-28.

| Asset | Bytes | SHA-256 |
|---|---:|---|
| `sail-0.1.0-rc.7-x86_64-unknown-linux-gnu.tar.gz` | 56,853,649 | `d950044aa1a7a5b865f52d441ac4e539119b7ebead7b8a5e828486ab4a7a773b` |
| `sail-0.1.0-rc.7-aarch64-apple-darwin.tar.gz` | 58,217,800 | `59cce7c3c5342c3f16bf052a9ed1870d8579d3f8b2e90a4579326ca1af10da21` |

Bundled engine manifests: Linux `d6096d5b…e44b`, macOS `ea6853a8…9996`.
Draft release `v0.1.0-rc.7` targets `8797ccd4`. Workflow run 40762232461 of
`attest-release.yml` attested both archives, and
`gh attestation verify --repo Chelis-Lang/sail` passes for each.

## Gates

Linux gates ran on native EC2 c7i.8xlarge, us-east-1, one lane per row
(ADR 0038). macOS ran on EC2 mac-m4.metal, us-east-1c, macOS 27.0 (26A428).

| Gate | Result |
|---|---|
| `ci/local.sh gate` | Pass: `1209 tests run: 1209 passed, 54 skipped`; fmt, clippy, doctests, rustdoc, `layering ok (20 crates, 3 extension crates)`, deny, benchmarks page |
| `ci/local.sh solver` | Pass: `51 tests run: 51 passed, 1212 skipped`, fault injection included |
| `ci/local.sh e2e`, every phase plus `extra` | Pass: identical to `cargo verus verify` (exit 101; 20,247 cache entries); managed acceptance; `claude-code: listed 18 sail tools on protocol 2026-07-28`; real repair CLI/MCP acceptance |
| `ci/local.sh conformance fast pr` | Pass: every arm 33 same, 0 unstable, 0 different, 0 infra errors; 311 functions; engine 32 focused |
| `ci/local.sh conformance fast` | Pass: every arm 147 same, 0 unstable, 0 different, 0 infra errors; 3,451 functions; engine 146 focused |
| `ci/local.sh conformance standard` | Pass: every arm 159 same, 0 unstable, 0 different, 0 infra errors; 13,166 functions; anvil same in every arm; HumanEval mutants 21 of 21 same, 0 false verified |
| `ci/local.sh conformance deep` | Pass: every arm 159 same, 0 unstable, 0 different, 0 infra errors; 13,166 functions; the two Singular projects skipped (binary releases do not support Singular). Four one-round mutation cohorts below |
| `ci/local.sh deep-rvt` and `deep-rvt cache` | Pass: every fork arm (direct, cache-fill, cache-warm) 4,902 tests, 4,902 same, 0 unstable, 0 different; all 4,839 nonignored tests pass. Stock control: 0 different; 3 and 4 unstable `output_json` tests (allowed, ADR 0042) and the two known `cargo::cargo_new_*` source-build failures (VI-0018) |
| `scripts/verus-determinism.py --runs 3` | Pass: `320 of 320 deterministic over 3 runs` (fast tier direct inputs) |
| `ci/run.sh host` on macOS | Pass: clippy clean; `1198 tests run: 1198 passed, 54 skipped` |
| Release build and packaging | Pass on both platforms: `THIRD-PARTY-NOTICES is current`; self-check selected the bundled engine; proof record and verify passed; `sail --version` prints `sail 0.1.0-rc.7` |
| `scripts/release-smoke.sh` on each archive | Pass on both: bundled runtime selected, release proof served, edited verification, repair accepted through the bundled engine |
| Install and README walkthrough | Pass on both with empty Rust homes and `install.py --version 0.1.0-rc.7 --install-toolchain`: attestation verified, `sail new`, first verify, edit, revert and two unchanged runs. Linux under umask `002` refused the shipped proof (#591, below). MCP `tools/list` reports 18 tools and version `0.1.0-rc.7`, checked with direct JSON-RPC (the real Claude Code client listed 18 tools in the e2e gate) |

Deep-tier mutation cohorts, one round each (Comparisons, Same, Unstable,
Different, False verified, Infra):

| Cohort | Mutants | Result | Loop speedup |
|---|---:|---|---:|
| CapybaraKV | 53 | 56, 56, 0, 0, 0, 0 | 1.99× |
| HumanEval | 52 | 55, 55, 0, 0, 0, 0 | 2.78× |
| IronKV | 48 | 51, 51, 0, 0, 0, 0 | 1.42× |
| Vest | 52 | 55, 55, 0, 0, 0, 0; one accepted divergence (VI-0003) | 1.13× |

Every ordinary and forced revert got the original's own answers, with one
daemon per round. These speedups come from a host that ran only the deep tier.

## Findings

- [#591](https://github.com/Chelis-Lang/sail/issues/591): `scripts/install.py`
  created directories with the caller's umask. Under Ubuntu's default `002`
  they were 0775 against the indexed 0755, so the shipped vstd proof was
  refused with a `SAIL3138` note that blamed the toolchain or Verus, and the
  first Linux verify took 29.5 s instead of serving the proof. Verdicts were
  correct. macOS (umask 022), `release-smoke.sh` and plain `tar` extraction
  were unaffected. Fixed on main by #593 (`f7970dea`), after the candidate
  commit; the archives do not contain the installer.
- macOS Vest revert outliers: two of six performance rounds missed the fast
  revert path (about 12 s instead of 1.1 s, still faster than stock's 22 s).
  Not diagnosed; see [performance](../performance.md).

## Evidence

Raw logs are kept outside the repository, in the evidence bucket under
`experiments/2026-10-28/rc7/`:

| Archive | Contents | SHA-256 |
|---|---|---|
| `rcgate-logs.tgz` | gate, solver, e2e, and the failed first attempt | `3edeba31f712a10b9cf31cdcacbb16f6990a63baf9ca32a3d1e2fc0dfd363148` |
| `rcconf-logs.tgz` | fast pr, fast and standard conformance | `4cef2765f1f946fd3364fb0a586d7fb1dfc9d7eb993ef49655ed7013f46a2dfa` |
| `rcdeep-evidence.tgz` | deep conformance and mutation cohorts | `a6e912b69f1076a685d398ef9caafff4983d2d63bd5c366d715f4fd95eb7fcf6` |
| `rcrvt-raw.tar.gz` | deep-rvt, deep-rvt cache, determinism | `20c08bdec96a47bdee5abe6cabe7688b0a9264e2d418de0c7f48bc9c7aac4220` |
| `rc7-logs.tgz` | release build, packaging, attestation, smoke, walkthroughs, macOS host gate | `d54678e21173cc290ae95e188dc1639ec962edf93d4c1ae9ae13b22d6c6b6125` |

Limits: the deep tier's exit code was not captured (every step ran under
`set -euo pipefail` and every arm reported). The determinism solver captures
were not kept. The macOS host gate reused a build of `1415ff47`, whose Rust
sources equal the candidate's.

## Failed or interrupted attempts

- `8797ccd4`, 2026-10-28, Linux gate attempt 1: one test failed
  (`test_complete_runner_rejects_build_script_created_during_stock_calls`,
  `fatal: not a git repository`). Cause: the host tree was synced from a
  worktree whose `.git` pointer file names a local gitdir. Rerun with real
  history at the candidate commit passed.

## Earlier candidates

None of these qualify rc.7. Their records are in this file's git history.

- rc.6, `cda42861`, 2026-10-23: functional qualification passed on both
  platforms ([matrix](release-matrix.md)); performance not accepted. Private
  draft, superseded.
- rc.6 first build, fork `f8589ed4`, 2026-10-23: macOS HumanEval mutation
  cohort had three functional disagreements (#337). Superseded.
- rc.5, `c3687990`, 2026-10-15: macOS asset omitted vstd's stdout summary
  after dependency-store seeding (#184); input-coverage limit #187. Draft,
  never accepted.
- rc.4, `5d99671e`, 2026-10-15: revalidation of #125. Superseded.
- rc.3, `8bd4aed2`, 2026-10-15: first candidate on Sail's own Verus (ADR
  0043). Superseded.
- rc.2, `8f427e81`, 2026-10-14: revalidation of rc.1's findings. Draft deleted.
- rc.1, `7782b136`, 2026-10-14: first candidate; walkthrough findings fixed
  in rc.2.
