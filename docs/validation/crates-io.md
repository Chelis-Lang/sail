# Crates.io distribution qualification

Status: the owner authorized initial `sail-solve 0.2.0-rc.1` publication and
crate publishing for future releases (ADR 0098). All eighteen source packages
and actual installed commands pass native Linux and macOS qualification.
The protected implementation PR #656 is merged; immutable tag
`v0.2.0-rc.1` selects merged source `81c41be8`. All eighteen packages verify
from that exact final source. Actual upload was attempted and crates.io
requires the publishing account to verify its email. Initial publication and
anonymous registry installations on both platforms remain pending. This does not close the broader release
performance gate.

## Scope and current gates

Source `ef6d53da`, pinned A0e runtime, c7i.4xlarge / us-east-1 / Ubuntu 24.04:

| Gate | Actual result |
|---|---|
| Source packages | Cargo verifies all eighteen extracted packages together with `--locked`; archive count, clean Git provenance and source commit are checked |
| Package resources | All 38 package-local copies match their authoritative repository bytes |
| Installed commands | Release `sail` and `sail-solve-z3` copied alone outside the checkout; actual fresh public runtime download and full status validation pass |
| Existing installation | A second engine install with broken HTTP proxies succeeds using the validated existing runtime |
| Normal user setup | The literal README arithmetic example runs outside the checkout with only `SAIL_CACHE_DIR`, no runtime-selection override: passing/failing human output equals stock, all passing JSON reports are Current and complete, settled/replay and restored calls use zero verifier invocations |
| Standalone workflow | Existing complete driver passes: 68 recorded calls plus asserted watch and MCP flows, covering genuine verification, unchanged reuse, failure/fix/undo, jobs and included binary inputs |
| Workspace and solver | Clippy, 1,324 workspace tests, docs, layering, policy and 51 solver controls pass |
| End to end and fast conformance | Every local E2E phase including `extra` passes; all four fast arms report 147 Same / 3,451 functions, zero disagreements, instability, infrastructure errors or skipped projects |
| Refinement and private extension | All 316 direct Stock actual-body/refinement cases pass; private extension Clippy and all 22 tests pass; original executable/proof guards remain exact |
| macOS | Same production source, separate physical source/target, native mac-m4.metal / us-east-1c / macOS 26.7: Clippy and all 1,313 tests pass; both installed binaries, fresh runtime download, the normal quickstart and complete standalone driver pass |
| Registry consumer | Pending actual upload: anonymous `cargo install --locked` from an empty package cache outside the checkout on Linux and macOS |

The package/install capture has 171 regular members, each checked against its
size and SHA-256 seal; archive SHA-256
`b0b395e3e019896a734699f238364d4c6d11cb7e689afb504a9dbb6ab5af1eff`.
It binds every `.crate` provenance commit and the installed binary/runtime
identities. No publication credential is used by this gate.

The independent full native gate capture contains 4,797 sealed members,
including all 316 refinement receipts, E2E output and conformance artifacts,
the gate logs, and a second successful installed-command execution. Archive
SHA-256 `e2f6894948ffee8bcbaae1dab46514a0d6a8ed502e45696ae6d55be6ea975db9`.
Its package step retained a Docker Git-ownership refusal; the independent
package gate above verifies the same committed source with trust scoped to
the mounted `/work` checkout.

Normal-discovery capture: 38 sealed members, 11 recorded calls, archive
SHA-256 `8a8f62b4d005acce04bed5721598db93fdded9318a7302b39a9ec5874e021564`.
The fixed first-JSON / settled / replay calls record 1 / 0 / 0 verifier
invocations; the invalid source records 1 and restoring the proof records 0.
Every call and its output is retained. This tests normal setup and settled
reuse, without declaring first-unchanged or general timing parity.

Mac capture: 888 members, including 554 regular files sealed after readback;
archive SHA-256
`bd9d6f6dad18a0d0351ba0c0aeb2a892a6ed894204fde63e8a7dded305440c34`.
The normal quickstart records 11 calls with no runtime-selection override; the
complete existing standalone driver records 67 platform-specific calls plus
its watch/MCP assertions. The installed runtime has the exact pinned Mac
manifest `6bdbf6c1e51757212463ac0f491e07e7d9cacb20920264738d5bcf596f262173`.
The Linux driver records 68 calls; no platform case was omitted to equate counts.

Later amendments change only documentation, publication-workflow caching and
the publisher's explicit null stdin for preparation subprocesses. Production
Rust, manifests, dependency locks, package resources and runtime pins remain
byte-identical to the qualified source above. Final Cargo verification and
anonymous registry installation bind the final merged commit independently.

The exact solver algorithms, verdict/diagnostic contract, third-party lock
versions and runtime pins are unchanged from `911977aa`. Its complete standard
conformance and native source/runtime acceptance retain their original scope;
fresh package, E2E and actual companion-shim controls cover the changed paths.
Cargo installations establish their own proofs; these controls make no claim
to the archive channel's shipped-vstd acceleration or new timing acceptance.
The separate ongoing CapybaraKV timing run also exposes the explanation-note
cardinality defect [#655](https://github.com/Chelis-Lang/sail/issues/655),
retained as a failed harness classification with matching verification output.
Its proposed correction and performance interpretation do not authorize or
replace this package gate's actual installation and verification controls.

## Retained attempts

- `501f9bdb` with working packaging changes, 2026-11-02: initial native gate
  refused the changed Cargo assembly metadata. Reviewed exact metadata hashes
  and package-shape checks were updated; executable/proof/import/module digests
  and API declaration stencils remain unchanged. All 78 checker controls and
  the complete native Stock refinement run now pass.
- `ef6d53da`, 2026-11-03: first package-only attempt failed before Cargo because
  Docker could not create a target mountpoint under a read-only checkout.
  Corrected host setup only; the next attempt above passes. Failed capture has
  ten sealed members; archive SHA-256
  `6939714eb400fd2f56b54b501b57d59ba8ffc62dffc48ec2c668a0bab37247e0`.
- `ef6d53da`, 2026-11-03: the full-gate lane's package preparation refused
  root-owned Docker Git access to `/work` before Cargo ran. Source unchanged;
  independent packaging with checkout-scoped Git trust passes above. Failure
  log remains in the full native capture.

- `81c41be8`, 2026-11-03: all eighteen final-source packages verify; actual
  upload returns HTTP 400 requiring a verified publishing-account email.
  No successful registry publication is claimed. Retained redacted upload log
  SHA-256 `82fa8897c92379c4c50f9ee3dbf52c5ed63af6459c2b63735d83b721755a661f`.

## Publication procedure

Publish only the final clean merged source and preserve its immutable version
tag. The publisher verifies packages before loading a token, resumes only
identical non-yanked registry checksums, and refuses conflicting bytes.
Future published releases invoke the same publisher with a release-only
repository secret. See [releasing](../releasing.md#cratesio-publication) and
[installation](../install.md#cargo-installation).

## Fixed release source

Source `a252bd69` binds release-event publication to its fixed event SHA and
manual retry to an explicitly supplied qualified commit. The published tag
must resolve to that commit, including annotated tags, before checkout.
Checkout and workspace version are checked again before source commands or
the upload credential. All eighteen publication controls pass, including the
actual inline workflow code, moved tags, malformed/missing commits, published
release state and checkout/version disagreement. A read-only preflight against
actual published `v0.1.0` accepts its qualified commit and rejects a different
commit; it uploads nothing and reads no Cargo token.

The complete native workspace gate passes on c7i.4xlarge / us-east-1 /
Ubuntu 24.04. Its full log, source and both successful exit markers are sealed
and checked after transfer; archive SHA-256 `9438133e477a82ed706b867bbb4fce0702a525822c18e3f0d48c409d782a4dc6`.
Production Rust, package resources, manifests, locks and runtime pins are
unchanged, so the earlier Linux/Mac product results retain their recorded scope.
The initial crate prerelease remains bound to `81c41be8`; its tag is unchanged.
