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
checkpoints, not full acceptance of its rebased review branch. Full end-to-end,
fast/standard conformance and normal PR checks remain pending. Fresh version processes and full binary reads add work to
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
