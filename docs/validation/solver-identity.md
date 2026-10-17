# Solver executable identity (#213)

**Candidate, unqualified.** The daemon now computes solver identity for every
HELLO instead of retaining it under `(path, length, mtime)`. Each handshake
runs the version command and reads and hashes the executable bytes. A launch,
version-parse or read error refuses the handshake; the shim's existing fallback
is unchanged. This removes the demonstrated stale-metadata memo, not every
possible concurrent executable-mutation race.

On shared macOS arm64, the retained private CLI built from `af339f8a` returned
the preceding solver SHA after a one-byte comment replacement at fixed path,
length and nanosecond mtime. `server.rs` is byte-identical at main `79e26818`.
The two real protocol connections returned
`b9c10bcc305463722b87534227458cb5fd0d8d168f5d68bbc665a9dc6ad7bcf0`;
the second executable's actual SHA was
`048a3e1560ee4d1cd082a30d08483a345bd8b2a3fb6e6fdaa58ac9abd5acb811`.
Its ctime changed. The fixture used only a private fake solver and daemon;
no installed runtime was changed and no wrong proof or verdict was tested.

Raw observation SHA-256
`3ef96d8617994a0d5abe37b2bd6f7d117ff8db6ee46552ab28677173d32aa34c`.
The retained private Mac CLI SHA-256 was
`d68e680cc50ddf50b0c98518315d2dad760566635c604c3ca6b0ad470361e947`.
The [standalone reproduction](../../corpus/repros/daemon-solver-identity/README.md)
returns nonzero for stale identity and removes its owned process and scratch.

The candidate adds real-client regressions for in-place byte changes with
restored mtime, atomic replacement, unchanged executables, removed executables
and removed execute permission. Independent static review passes. On exact integration source `dfafffcf`,
shared-Mac host checks pass clippy and 646 tests (35 solver tests skipped),
and the actual standalone reproduction passes. The corresponding native Linux
checkpoint passes the workspace gate (657 tests, 35 skipped, two doctests),
all 35 solver tests and the actual standalone reproduction. The two native
HELLOs return the distinct expected hashes above. These are the stated source's
checkpoints, not full acceptance of its rebased review branch. The exact frozen
source subsequently passes all nine end-to-end phases plus extra and four full
FAST arms (147 Same / 3,450 functions each, zero disagreements, instability,
infrastructure errors, skips or rlimit drift). All four STANDARD arms pass
159 Same / 12,726 functions each, with recorded rlimit-only drift. Ordinary
empty-cache bundled delivery also passes; performance acceptance is pending. All 17 required and
supporting PR checks pass on review head `1ec72807`, whose non-documentation
Git blobs and modes match the frozen source; later documentation changes still
require their own exact-head checks. See the
[integration checkpoint](combined-fork-candidate.md#current-integration-checkpoint-2026-10-17).
Fresh version processes and full binary reads add work to
each solver-session handshake. Measure that cost against the preceding build
on native hosts, including the warm-cache fresh-project workload, before
acceptance; no negligible-overhead claim is made.

Native checkpoint archives: workspace
`73383481aa397fd030ce5b50cfd169c234bf27cfef3ce6458733585b53d6300b`,
solver `b31d3600dc411ca45ef93aba3e6262a0b52918c3cd6566f6fbe3eadaf62c0839`,
standalone reproduction
`a43905e579933d3c7df5f4fd0a72e6e3fc3c951cb43d0eb866396aedbb2a6f90`.
Every actual member in all three archives was independently checked. Mac host
log SHA-256 is
`203c8b3c5f43ede47adcb8b3320e2ba43e62e82238c1d6eab30b09b71102607c`.
No runtime performance claim follows from these correctness checks.

Failed attempts remain indexed here:

- 2026-10-17, `af339f8a` private Mac CLI / daemon source equal to `79e26818`:
  actual stale identity reproduced; original outcome remains failed.
- 2026-10-17, isolated cost screen `658961e2` / source `36f39a79`:
  stopped at the before-fix workspace gate, before any of the 110 planned
  timed rows. The external controller injected `SAIL_VERUS_BUILD` and
  `SAIL_ENGINE_BUILD` into that gate; two runtime-location tests selected the
  forced runtime instead of their isolated temporary fixtures (653 passed,
  two failed, 35 skipped). Both assertions remain unchanged. This is failed
  setup evidence, not a measured identity overhead or a conformance pass.
  Final archive SHA-256
  `8665fc8816d35986348ddff7572edc0877481c7173436b611cc0aed50d52bb37`;
  actual gate log
  `ca5506463353cc0d6d28ed1fda4982ac2f9de735ba2534bd7d7547f3b4afa100`.

- 2026-10-17, isolated cost screen `86083` / five unchanged source arms:
  all five workspace gates and releases pass, including both original failing
  runtime-location tests with unchanged assertions. After genuine runtime and
  changed-compiler controls pass, the Ubuntu controller cannot create a derived
  spec inside Docker-root-owned mode-0755 `specs`. The mode-0644 producer JSON
  is readable; creating its sibling fails with `PermissionError(13)`. Zero of
  110 planned timing rows ran. A prospective correction separates host-owned
  derived specs and requires an actual native ownership control before builds.
  Final archive SHA-256
  `2e8cf7388384e13f7302d0a56a912e9141569b85b140606de642f5277eb57f1d`.
