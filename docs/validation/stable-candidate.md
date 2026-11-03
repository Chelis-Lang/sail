# Stable 0.2.0 candidate qualification

Status: stable source prepared, not published. Final owner approval remains
required. The initial registry prerelease is already published; it is a separate
immutable source and is not being repackaged as stable.

## Source and scope

Native qualification and archive builds use `e6adf3c0dd9d6a663bd2520bcbca697e7c7cac89`,
based on main `3e44db15`. It changes the inherited workspace version, twenty
exact internal requirements, twenty-one root-lock package versions and sixteen
imported extension-lock versions to `0.2.0`. The extension's own version remains
`0.1.0`. Parsed lock comparison finds no changed third-party version, checksum
or dependency edge. The refinement guard changes only the exact root lock
SHA-256 to `b5830bca99063940bedd6f4f959d77e82fcd36882a24d29fc0de02292e19e510`;
all executable, proof, import, declaration and manifest guards remain intact.

Later package README and qualification-document amendments do not change Rust,
locks, embedded resources, pins or build scripts. Final source/package Git
provenance must be bound after the protected merge; earlier execution is never
relabeled as a new run. The final source packages must be regenerated and
verified from that clean merged commit before requesting publication approval.

## Current gates

| Gate | Actual result / remaining work |
|---|---|
| Local guard controls | All 78 refinement checker controls, 18 publication controls, 38 resource-copy checks, formatting and diff checks pass |
| Native Linux workspace | c7i.4xlarge / us-east-1 / Ubuntu 24.04, pinned CI image: full workspace gate passes |
| Direct Stock refinements | Full 316-case candidate run in progress; prior prerelease run retains its original source scope |
| Candidate Linux solver/E2E/corpus | Native solver, every E2E phase including `extra`, fast/standard/deep conformance queued after refinements |
| macOS platform and factory | mac-m4.metal / us-east-1c / macOS 26.7: Clippy, all 1,314 host tests and native release factory pass |
| Cargo source packages | Independent native Linux lane: all eighteen extracted stable packages verify together with `--locked`; no credentials loaded or uploads attempted |
| Stable archive installation | Exact native archive assembly, clean-PATH smoke and fixed three-call ordinary-Cargo JSON replay are in progress on both platforms |
| Protected source PR | Pending normal required CI and current-main squash merge |
| Final publication | Not approved; no stable tag, draft or upload created |

The runtime is the already-qualified pinned A0e fork. Its source suites,
determinism, exact binary-input controls and full installed feature controls
retain their actual source/runtime scopes in
[runtime qualification](binary-compiler-inputs.md),
[feature qualification](release-expansion.md) and
[crate qualification](crates-io.md). This version-only preparation does not
change the solver algorithms or verifier. Fresh native gates, package and
archive controls qualify its changed assembly rather than claiming earlier
executables were built with version `0.2.0`.

## Release boundary

The [release notes](../release-notes/0.2.0.md) describe user-visible scope and
remaining performance gaps. Before requesting owner approval, complete native
gates and raw capture, record exact archive checksums, merge the source PR and
verify final-commit source packages. After explicit approval only, create the
exact stable tag/draft, attest and independently verify these archive bytes,
and publish. The published-release workflow then publishes matching crates
from the immutable approved commit. Required attestation failure stops rollout.
