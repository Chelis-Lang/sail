# Stable 0.2.0 candidate qualification

Status: native stable candidate gates and both exact archives are qualified.
Final source assembly and merged-commit package provenance are tracked in
[PR #661](https://github.com/Chelis-Lang/sail/pull/661). Final owner approval
remains required; no stable publication is approved. The published registry
prerelease is a separate immutable source and is not repackaged as stable.

## Source and scope

Native qualification and archive builds use
[`e6adf3c0dd9d6a663bd2520bcbca697e7c7cac89`](https://github.com/Chelis-Lang/sail/commit/e6adf3c0dd9d6a663bd2520bcbca697e7c7cac89),
based on main `3e44db15`. It changes the inherited workspace version, twenty
exact internal requirements, twenty-one root-lock package versions and sixteen
imported extension-lock versions to `0.2.0`. The extension's own version remains
`0.1.0`. Parsed lock comparison finds no changed third-party version, checksum
or dependency edge. The refinement guard changes only the exact root lock
SHA-256 to `b5830bca99063940bedd6f4f959d77e82fcd36882a24d29fc0de02292e19e510`;
all executable, proof, import, declaration and manifest guards remain intact.

Rebasing onto main `4f9191a0` also inherits #660: third-party debug symbols
are disabled in dev/test profiles. Parsed comparison requires those exact two
profile additions and identical remaining root manifest data; all other
non-Markdown files match the qualified archive source. Release profiles, Rust,
locks, embedded resources, pins and build scripts are unchanged. A fresh native
workspace/extension gate at
[`0955f4aa`](https://github.com/Chelis-Lang/sail/commit/0955f4aa5ef5cc803efe691c965b8526ae639248)
passes with the changed development assembly.
Later documentation amendments have no compiled input changes. Final source/package Git
provenance must be bound after the protected merge; earlier execution is never
relabeled as a new run. The final source packages must be regenerated and
verified from that clean merged commit before requesting publication approval.

## Current gates

| Gate | Actual result / remaining work |
|---|---|
| Local guard controls | All 78 refinement checker controls, 18 publication controls, 38 resource-copy checks, formatting and diff checks pass |
| Native Linux workspace | c7i.4xlarge / us-east-1 / Ubuntu 24.04, pinned CI image: e6 gate passes; fresh current-main `0955f4aa` Clippy, 1,325 workspace tests, remaining gate checks and all 22 private-extension tests pass |
| Direct Stock refinements | All 316 actual-body/refinement cases pass at e6: seven positive roots and 309 negative controls; prior prerelease run retains its original source scope |
| Candidate Linux solver/E2E/corpus | All 51 solver controls, every E2E phase including `extra`, all four fast arms (147 Same / 3,451 functions each) and full standard (four arms each 159 Same / 13,166 functions, all 18 mutations plus cold/both returns) pass. All four deep corpus arms pass (159 Same / 13,166 each), explicitly excluding only the two optional Singular projects. All 205 seeded mutations and 12 cold/return controls pass; both returns receive their own answers |
| macOS platform and factory | mac-m4.metal / us-east-1c / macOS 26.7: Clippy, all 1,314 host tests and native release factory pass |
| Cargo source packages | Independent native Linux lane: all eighteen extracted stable packages verify together with `--locked`; no credentials loaded or uploads attempted |
| Stable archive installation | Both exact `0.2.0` archives pass clean-PATH first-use/repair smoke and fixed three-call ordinary-Cargo JSON verification; every call is Current/complete and the final call invokes zero verifiers |
| Protected source PR | Normal six-check current-main squash merge and final-commit package results are bound in [PR #661](https://github.com/Chelis-Lang/sail/pull/661); green checks do not authorize publication |
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

## Exact staged archives

These are native `0.2.0` builds of `e6adf3c0`, with new version metadata;
they are not renamed rc.1 binaries. Both factories check the CLI/MCP version,
complete runtime inventory, source-bound vstd proofs, solver-reply indexes and
release file/mode indexes. Both fresh-install smoke runs admit the shipped
vstd proof and accept repair through the bundled engine.

| Candidate artifact | SHA-256 |
|---|---|
| `sail-0.2.0-aarch64-apple-darwin.tar.gz` | `ace0e6d1bf8addecfa025f59f35966af956de2712b327379549e3ff86126e47e` |
| `sail-0.2.0-x86_64-unknown-linux-gnu.tar.gz` | `4fce5acbb6d2a8ed4331add3e49f1af1239d86c573533dc1f0bcb73cd1a8a100` |

The smoke logs and all three ordinary replay JSON/stdout/stderr transcripts
are captured separately from the completed native corpus qualification:

| Archive validation capture | SHA-256 |
|---|---|
| linux | `e56c7c59a3d844e73ffb7fc3a108ce4b030be8c29a92c6c8c919cb38126d482f` |
| macos | `f8ccedc99a066a11f4fb49c64c2d3d7344982576211bd68d09ec83eb50933f73` |

The independent all-eighteen-package/factory capture has 27 sealed members,
SHA-256 `692b80eef6d2dae8f90814d10e628ede5075a956cd31ae4c6ddceba0714bd080`.
Its packages have the actual tested e6adf3c0 Git provenance; final merged-source
packages are regenerated separately. No publication credential was used.

The refreshed development-profile/extension gate capture includes the two
initial setup failures (Git worktree metadata absent from Docker; missing
cached private Git revision), the successful unchanged workspace gate and
successful extension run. Archive SHA-256
`28548789f2845fec7313d2c74e5a320251578269859c88350d32747ea5b99e73`.
The retry mounts the common Git directory read-only and supplies the exact
already-pinned private revision; no source or assertion changes.

The completed independent Stock refinement capture preserves all 316 receipts
and their exact source/runtime binding: 634 files, SHA-256
`18c11eca973b08baadc568053841cef4b4e75e814d094ed8c952580d908f2b2c`.
The complete native product/corpus capture has 12,921 sealed files,
SHA-256 `a8e170e3a3ba854429293fa92d7a414f1a292b9477cf150092a294e9b6bee840`. Every size/hash seal is checked after transfer.
Independent structured-report validation requires all four arms per tier,
matching runtime identity, exact fast/standard counts, no disagreements,
instability or infrastructure errors, and complete mutation/return inventories.
Fast and standard have no skipped projects. Deep records exactly the optional
`verus-integer-ring` and `verified-memory-allocator` projects, both requiring
Singular and a Singular-enabled source build (ADR 0021); this does not qualify
that separate optional configuration.

## Release boundary

The [release notes](../release-notes/0.2.0.md) describe user-visible scope and
remaining performance gaps. Before requesting owner approval, complete native
gates and raw capture, record exact archive checksums, merge the source PR and
verify final-commit source packages. After explicit approval only, create the
exact stable tag/draft, attest and independently verify these archive bytes,
and publish. The published-release workflow then publishes matching crates
from the immutable approved commit. Required attestation failure stops rollout.
