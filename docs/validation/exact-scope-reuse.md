# Unchanged partial verification

**Status:** implemented. The corrected combined consumer verifies Anvil once,
then starts zero verifiers on its first unchanged call. Complete release
performance acceptance remains open; this record does not claim cold parity.

## Behavior and input coverage

Exact-scope reports bind the ordered verification selectors and all ordinary
source, runtime, environment and artifact checks. A partial result stays
partial; it cannot authorize a whole-target proof. Changed selectors, failed
edits, missing receipts, changed artifacts and restored-mtime writes refuse
reuse. Whole verified dependencies retain their own provenance.

Within one call, receipt checks can project selected revisions from a complete
source observation. The exact fresh-read SHA-256 encoding, foreign dependency
binding and original mutation stamp are preserved. An entire batch is accepted
only after checking that original stamp. Post-run reuse also requires the same
complete observed namespace. New includes, uncovered aliases, mutations,
incomplete observations and retries require fresh reads. No source hashes are
retained between calls; compiler identities and solver keys are unchanged.

Compiler declarations can spell an already observed file as `src/../file`.
Mutation stamps identify the resolved file once and separately stamp actual
symlink ancestors. Source bindings remain exact: `alias/../file` is resolved
through the filesystem, never by deleting `..` text. A newly discovered alias
or file still changes the observation. Intentionally absent Cargo configuration
paths retain their missing-path stamps.

### First recheck after native-header discovery

Explicit native include hints, such as `OPENSSL_DIR/include`, preobserve headers
that build scripts would otherwise discover only after compilation. Actual
script declarations and the normal content/mutation checks remain authoritative.
Relative or missing hints add no coverage. See [freshness](../freshness.md).

## Corrected combined consumer

Source `cda42861`, runtime `7d9d5410` on upstream `7d82994`, Cargo/Rust 1.99.1.
Native Linux: EC2 c7i.8xlarge, us-east-1. Mac: EC2 mac-m4.metal, us-east-1c,
macOS 27.0.

| Installed Anvil call | Seconds | Currentness | Verifier invocations |
|---|---:|---|---:|
| Cold | 191.21 | Current | 3 |
| First unchanged | 1.81 | Current | 0 |

The selected scope is `--lib -- --verify-module vdeployment_controller`.
These calls establish removal of repeated verification, not cold performance
parity against stock. The requested root remains partial; two target reports
are reused on the unchanged call.

The complete native workspace gate passes 992 tests; solver tests pass 42/42.
Full E2E including extra, PR conformance and full FAST pass. Mac clippy and all
981 host tests pass. STANDARD passes all four arms: 159 Same and 13,166 functions per arm,
with zero differences, instability, infrastructure errors or skipped projects.
Its 18 mutation plus cold/revert/forced-revert controls also pass (21 Same).
The corrected final Linux/Mac matrices remain pending.

The new regression discovers both plain and symlink-dependent `../`
declarations, then retargets between already-hashed good and bad files and
restores the alias. Logical identity restores on exact revert; the within-run
mutation stamp remains changed. Separate controls retain absent configuration
roots and detect restored-content/restored-mtime mutations. No comparison,
conformance exception or release threshold changed.

Raw control: `experiments/2026-10-23/anvil-declared-spelling-control.tgz` in the
S3 evidence bucket, SHA-256
`ba2ff90e196fd21184e9cff1047a7866602ff396dec40637dffc97d08f645800`.

Native gate logs and reports: `experiments/2026-10-23/declared-spelling-native-gates.tgz`,
SHA-256 `33d93b6863135fd6c42393e33aaae4ac147608cb59cb8aae3cfcdd9cca5d998e`.

## Earlier component measurements and attempts

- `486e46ff`, 2026-10-21, native Linux: exact-scope reuse passed; unchanged
  calls 2.05/1.89 seconds, zero verifiers, against stock 0.287 seconds. Full
  native gates and Mac host controls passed. Raw `experiments/2026-10-21/anvil-scope-control.tgz`,
  SHA-256 `c2e59a81c4855735dd59ae7e18957d9e76b0a13d304f3b3668460ddb44dac288`.
- `1908b51c`, 2026-10-21, installed attempt failed the first-recheck goal:
  39.23/39.01 seconds. Cargo package values and `OUT_DIR` declarations were
  not accepted; the correction binds those values to actual Cargo units.
- `e581205c`, 2026-10-21, native-header discovery correction passed. Mac cold
  105.79 seconds, first unchanged 2.41, second unchanged 2.15; all Current,
  unchanged calls start zero verifiers. Native workspace, solver, full E2E,
  PR/full FAST and STANDARD passed. Raw `experiments/2026-10-21/anvil-first-recheck-gates.tgz`,
  SHA-256 `993581d78e83a5904d710d7b6075f87b775b9da91ec5b38cdcd947876501cc4a`.
- Original source-observation component, 2026-10-23, nine rotated Mac pairs:
  unchanged 2.088 to 1.494 seconds; three cold pairs 102.353 versus 102.103
  seconds overlap. Every call Current, unchanged calls start zero verifiers.
  This predates the Tokio update and source-binding integration and qualifies
  neither the current release binary nor cold parity. CLI hashes:
  baseline `cf55a904f130baff1dc876636d7d8ec38e532041519ac678265a6fc06b13f5fc`,
  candidate `87a5309c1b52a7e1eb22ef23163032479a4cf2cf9ac26c33d9acbea7bf86daa8`.
- `e627a4a0`, 2026-10-23, combined attempt regressed first unchanged to 61.94
  seconds/three verifiers. Trace found 166 newly declared `..` spellings but
  no changed file digests, source bindings or existing path metadata.
- `2c7f5164`, 2026-10-23, initial correction refused release recording with
  Unknown inputs because absent configuration roots could not canonicalize.
  `cda42861` preserves those missing-path stamps and adds a regression test.
