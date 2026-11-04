# Sail 0.2.0 release qualification

Status: Sail 0.2.0 is published after explicit owner approval on 2026-11-04.
Tag `v0.2.0` binds protected merge
`3d7782bf70bf54efbfe0378eac8fbdf7b2276c20` ([PR #661](https://github.com/Chelis-Lang/sail/pull/661)).
The two exact qualified archives were attested and independently verified before
[GitHub publication](https://github.com/Chelis-Lang/sail/releases/tag/v0.2.0).
All eighteen stable crate archives match the approved merged-source packages.
The earlier registry prerelease retains its separate immutable source.

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
provenance binds the protected `3d7782bf` merge; earlier execution is never
relabeled as a new run. All eighteen packages were regenerated and verified
from that clean merge before owner approval, then independently matched to
the published registry bytes.

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
| Final publication | Owner approved 2026-11-04; GitHub stable release and all eighteen matching stable crates published |

The runtime is the already-qualified pinned A0e fork. Its source suites,
determinism, exact binary-input controls and full installed feature controls
retain their actual source/runtime scopes in
[runtime qualification](binary-compiler-inputs.md),
[feature qualification](release-expansion.md) and
[crate qualification](crates-io.md). This version-only preparation does not
change the solver algorithms or verifier. Fresh native gates, package and
archive controls qualify its changed assembly rather than claiming earlier
executables were built with version `0.2.0`.

## Exact published archives

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
packages were regenerated and checked separately. No publication credential was used.

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

## Publication and installed consumers

Owner approval on 2026-11-04 covers the exact source and archive/crate identities
presented in PR #661. The [attestation run](https://github.com/Chelis-Lang/sail/actions/runs/41829722273)
passes at the tagged source; both draft archives were downloaded again,
matched against their approved SHA-256 values and independently verified with
`gh attestation verify` before publication. The
[crate workflow](https://github.com/Chelis-Lang/sail/actions/runs/41830064285)
then passes on the same immutable source. Every public `.crate` archive and its
clean VCS identity is independently checked against the eighteen approved
package hashes in PR #661.

Final merged-source package capture SHA-256:
`872b62553d634b6b9472a0d198a12e23682758052b71c3b3c39adb2b9be5ac68`.
Its individual crate hashes bind publication bytes; the outer tar identifies
only the retained package capture.

Actual public stable consumption passes on native c7i.4xlarge / us-east-1 /
Ubuntu 24.04 and mac-m4.metal / us-east-1c / macOS 27.0, using Rust/Cargo
1.99.1 and rust-src. Each installs `sail-solve 0.2.0 --locked` outside the
checkout from an empty Cargo package cache, without registry or GitHub tokens.
The downloaded root crate matches the approved hash and clean `3d7782bf` VCS
identity. Fresh public runtime installation, reuse with broken download proxies,
normal-discovery pass/fail/restore and zero-verifier unchanged calls all pass.
Both platforms also pass the complete standalone CLI/jobs/watch/MCP driver,
focused repair, generated-input/background preparation and actual cache-budget
controls. They install the archive through public `--version 0.2.0` discovery
and pass strict release smoke, including first-use shipped-proof admission and
repair. Anonymous installers check checksums and report their documented lack
of signed-in `gh`; the identical archive bytes were independently attested and
verified on the controller before publication.

| Stable consumer capture | Regular members including seals | SHA-256 |
|---|---:|---|
| Linux | 404 | `b3e564c0ab747aa40b01b31ee13090db84de9f2a2486bc6e6ed64535a40153d7` |
| macOS | 404 | `779f5f1df5d4203afb91bea5b69352aee2c0fd5ec74d8e1246e69b6600bc7000` |

Every raw file's size/hash seal is verified after transfer. These are actual
installed-consumer controls, not a new whole-call performance measurement or
an installation-time claim. Existing rc.1 consumers and prepublication archive
smoke retain their own sources. docs.rs builds all eighteen stable library
targets successfully; all 54 inspected crate/build/rustdoc pages are captured.
The publication/registry/docs.rs provenance capture has SHA-256
`f028411fbb983fb8b302f0b2ba877a0bf99895ff41b3eb8340553761c90b5fe5`.

Retained rollout attempts, all 2026-11-04 / `3d7782bf`:

- Attestation dispatch by raw commit ref: GitHub rejects that ref; dispatch by
  the exact annotated `v0.2.0` tag passes at the same commit.
- Mac Terminal launcher and unprivileged system-domain launchctl submission:
  setup refuses before consumer execution; a system LaunchDaemon under the
  normal user completes the unchanged controls.
- Mac capture through SSH's Python 3.9, then an absent Python 3.12 executable:
  collection refuses after all consumer controls passed; the installed Homebrew
  Python collects the same raw files and every seal verifies. No test or product
  input changes.


Known first/full-path, tiny-operation and Mac Anvil-cold limits remain in the
[release notes](../release-notes/0.2.0.md). Publication does not establish
universal speedups or enable the unselected digest experiments.
