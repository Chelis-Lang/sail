# Retained-proof qualification (#163, ADR 0050)

**Status:** candidate implemented; local regression checks pass. Independent
review of the source-inventory fixes and the full macOS host check pass. Native
Linux gates, full fast/standard conformance and performance
qualification remain required. This record does not qualify a release.

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
