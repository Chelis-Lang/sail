# Retained-proof qualification (#163, ADR 0050)

**Status:** candidate implemented; path-resolution guard qualification is in
progress. Earlier source-inventory review and macOS host results below predate
this guard and do not qualify it. Independent review, the full macOS host gate,
native Linux gates, full fast/standard conformance and new performance
measurements remain required. This record does not qualify a release.

## Source, runtime and scope

- Source: `feat/instant-revert`, based on `c226ff7e`, with the source-inventory
  and logical-target storage-bound review fixes in this PR.
- Runtime: pinned fork `73dea02112d730e7ab52f58158012d440fb89512`, installed
  macOS artifact; Rust 1.98.1 for the integration fixture. Sail's local
  development binary was built with Rust 1.99.0.
- Environment: local macOS arm64, shared development machine. No performance
  acceptance is inferred from these correctness checks.
- Unit scope: eleven history tests, including source/directory additions, new aliases,
  symlink retargeting, same-byte writes, uncovered compiler inputs, generated
  input reverts, dependency/environment differences, corruption, provenance
  and capacity shared across compiler-identity variants. All 180 local
  `sail-verus` library tests pass; clippy for all crate targets passes.
- Full macOS host check on `d94d36b6`: clippy and all 620 enabled non-solver
  tests pass, with 35 solver tests skipped by the host tier.
- Integration scope: one tiny Cargo fixture using `verus_builtin` with
  `no-vstd`, exercising the real Sail driver and pinned fork. Its functions
  carry no SMT proof obligations; it qualifies the restoration and compiler
  diagnostic mechanism only, not proof-corpus equivalence.

## Findings and current result

The first prototype checked only positive dep-info paths. Starting from
`mod leaf;`, `leaf.rs` and a successful run, adding `leaf/mod.rs` changed none
of those recorded bytes. Sail incorrectly restored success with zero
verifier invocations and marked the result Current; direct pinned Cargo-Verus
returned exit 101 and E0761. The fix checks complete source inventories and
brackets both recording and restoration with managed mutation stamps.

The corrected fixture returns E0761/101 with one fresh verifier after the
addition. Removing the new file restores the original proof and its
establishing run. A changed source verifies freshly, and reverting it restores
the earlier proof. A build script changing a generated constant from 1 to 2
verifies freshly; regenerating 1 restores its earlier proof and provenance.
All integration steps end Current, with each failing target incomplete. A
second run also tests a new symlink module candidate pointing to an already
tracked source file; it reports E0761 instead of restoring success.

The prototype also bounded entries separately for each complete compiler
identity. Cargo-unit variants could accumulate under one proof context.
Capacity now applies to the logical target across those identities; the unit
regression retains three variants at capacity two and checks eviction and
safe lookup of both remaining variants.

Raw records are in the external `instant-revert-review-2026-10-16` evidence
directory:

| Artifact | SHA-256 |
|---|---|
| Original incorrect `new-module.stdout` | `f181d5eee78b70d7072ccfa01ad72687e0fed2b4b743125f14ef73cb3cf8445b` |
| Direct pinned-fork `direct-pinned-fork.stderr` | `775d1344e79311c8b931204aaaf11259556768dad5c1a1c0ad6b6a1bf1ee8b74` |
| Full macOS host `macos-host-d94d36b6.log` | `1ae47cf0f0263f64a334aa6b6e74e49cd3ae845fc3a0126a7f1e707b0666f8cb` |
| Earlier corrected `fixed-summary.json` | `782ce1e3b7ce3e79684e34296af95cb0cee0bff57c05962b27fc787cdb5ffc55` |
| Latest `qualified-summary.json` (including alias topology) | `5d60410ae526c377767082f0c4e5991031616ef0bceb4556ccf519ee615cce8e` |

The corrected summary identifies the tested binary by SHA-256. The original
failure used the inherited prebuilt binary from the reviewed worktree, not a
fresh source build; its source attribution is limited accordingly.

## Remaining gates and attempt index

Run the full local gate, solver checks, every end-to-end phase including
`extra`, full fast and standard conformance and isolated
native performance measurements on the final source. Existing prototype
timings do not qualify the new source-inventory guards.

- `c226ff7e`, 2026-10-16: failed candidate review; real driver restored a
  successful result after a new module candidate should produce E0761.
  Cause: dep-info recorded positive input paths without absence coverage.


## Ancestor and mount mutation correction

The `0996f708` candidate's complete file/topology snapshots still accepted an
ordinary ancestor rename-and-restore: move the fixture's parent away, put a bad
source tree at the old path, read bad source, remove it and restore the parent.
The observed workspace inode and final bytes are unchanged. The regression
fails against the old guard (exit 101; raw log SHA-256
`a808ae500c18119c2bacaa1157d9a4fa71a2c4d595751086a84c234c93a4ff2d`).

A second actual old-guard negative control uses an interior directory alias
whose canonical target is already the visited workspace, so snapshot root
deduplication omits that alias. An intermediate symlink outside the workspace
is retargeted to a bad tree and restored after reading different source. The
old guard again accepts; the new strict assertion fails with exit 101 (log
SHA-256 `d795ce94df921eecf32eb81e7ea542cbf611f74a454a6ca6260b9493e03c341f`,
old source plus regression SHA-256
`4980b44987b4e9dc7219e3525d4aefd50b66853bdfbb7d4e464baffd3292bd58`).

The candidate now uses the shared synchronous resolution guard, collecting
aliases from its existing topology enumeration. It attaches watches to opened,
identity-checked ordinary ancestor inodes and expands every symlink target
component. Native activation is followed by a full snapshot/revision/mutation
consistency check before compilation or restoration; later barriers check the
native guard before and after their existing checks. Unavailable setup or any
latched mutation refuses. Original content/dep-info/environment/dependency and
artifact/provenance checks remain strict.

The shared guard's actual macOS overmount fixture mounts a small APFS image
over a source root, reads different bytes, detaches and confirms the old bytes
returned. A vnode-only negative control wrongly accepts (exit 101, log SHA-256
`a638cb31329bd16f8bcc04a604b88418096970571f0f61ceedaf93ea4030d54f`).
The global filesystem-event observer refuses and remains latched (exit 0, log
SHA-256 `869b8b4ed68bc84231e145e770f416324e8619123d4fa9dae830d765ae28acd4`).
This qualifies the shared observer fixture only, not a full retained-proof
Verus run, native Linux execution or real-project performance.

- `0996f708`, 2026-10-16: failed ordinary ancestor ABA regression; complete
  final snapshots did not reveal a different source tree read during the epoch.
- `0996f708` plus new alias regression, 2026-10-16: failed indirect alias ABA;
  an intermediate symlink was absent from the root mutation stamps.


The integrated candidate's bounded release build passes all 184 `sail-verus`
library tests serially, including both new ABA tests and the existing positive
A → B → A/generated-input restoration tests. The 13 history tests also pass
with default parallel execution. Their logs have SHA-256
`faeb8fc72dfcd86ea026aa0fbeaa48be28e31a5615e8edf913b4ef5ec3da4bf3`
and `5d04f05ea8fb62b820c1477a24c5deffbbd0f1b33354d026312b2153c189855d`.
Release clippy for all crate targets passes after a lint-only in-place parent
traversal change (log SHA-256
`1d6caa2c58634f24b16b8f6de66ff5d8c855d6ca5a160957eae5411ed3c5a42e`).
These checks precede the shared observer's final filesystem-admission and
nonblocking-open hardening, qualified below.


The final shared observer opens watch endpoints with safe nonblocking,
no-follow, close-on-exec descriptors, requires regular-file/directory type,
and admits only the documented local filesystem types. Every directory from
the source topology pass participates, including already-mounted subtrees.
The actual-source FIFO/watch test refuses without a peer; restoring the old
blocking watch-open variant fails its two-second assertion (exit 101, log
SHA-256 `266f26b07cf298b87a7c43827149dbd4801155fc109f212d446ab041c895f1e7`).
Admission tests cover APFS/HFS local flags and recognized Linux types versus
network, pseudo, FUSE and unknown types. This is predicate scope; actual Mac
filesystem execution is APFS, and native Linux/HFS execution is not claimed.

With the exact shared observer source SHA-256
`4fcb814a31c29aab0c49c115f5a2986a0023a0526e9bbef8b3338cff2bfec1a4`,
all 186 `sail-verus` library tests pass serially, including actual positive
retained-proof A → B → A and generated-input reuse, both ABA regressions and
bounded FIFO refusal. The bounded release log SHA-256 is
`fd2de65bd6313ea51747e742e2befd493d6a03299cec4e8fd1ab756c9b7160a3`.
All-target release clippy passes (log SHA-256
`7db328f8d531582edece0c006edb0c4eeca09066749f3eb0fab37ed6fb61e85f`).
These checks are the final focused Mac fixture scope, not the full host,
solver, real Verus conformance or performance gates.


## Literal compiler-input resolution correction

The `f608cad7` coverage check accepted a dep-info path after canonicalizing an
unobserved external alias into inventoried source. Retargeting that alias,
reading different compiler input and restoring it left the observed source
unchanged. The same failure occurs through an external directory alias and
through an unobserved ordinary directory followed by `..`: temporarily replace
that directory with a symlink into a different tree, read different source,
then restore the ordinary directory before checking.

An actual negative control uses the exact `f608cad7` resolver and coverage
behavior with the three regression tests. All three strict coverage assertions
fail (exit 101); the other 14 history tests pass. The source with tests and raw
log remain external: resolver SHA-256
`4fcb814a31c29aab0c49c115f5a2986a0023a0526e9bbef8b3338cff2bfec1a4`,
history SHA-256
`defad2b33fd3cb935f8ac5d41757cf17125d4aac796184bdc0d568c6d2c45b86`,
log SHA-256
`15e5353784a490909ea4eacf88919d72eb6c5d071eb691a674fdc3f0a5536180`.

Capture and read-only coverage now use one component walker. Every literal or
expanded intermediate endpoint, including ordinary components consumed by
`..`, must have been observed before the epoch. An unknown symlink refuses;
only a terminal regular file may be absent from that resolution map, and it
must still match a positive inventory digest. Native barriers bracket the
walk. Final canonical resolution is only a consistency check, never an
alternative authorization path.

All 190 actual `sail-verus` library tests pass in the bounded macOS release
build, including the three new negatives, observed-alias/parent-component
positive coverage, and retained A → B → A/generated-input restoration (log
SHA-256 `69a46159f172ef5865967834331d11de00961f9ae9a5a80e88862086881ec9d6`).
All-target release clippy passes (log SHA-256 `1b1d57eaad44a575984ad5698a161e44e3d559a64d4b3c167f7654ecc456ca1f`).
Shared resolver source SHA-256:
`22dde0d4e1f8518d29be4121bf05c645a527591002516d679716eb70cbdd17c0`.
Independent static review is clean; native/full-host/conformance/performance
gates remain pending for this correction.

- `f608cad7` plus regressions, 2026-10-16: failed three external input
  resolution ABA controls; canonical target membership omitted literal path
  components that the compiler could traverse during the epoch.
