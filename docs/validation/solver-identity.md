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
and removed execute permission. Native Linux and macOS tests, solver tests,
full fast/standard conformance, independent review and the normal PR gates
remain pending. Fresh version processes and full binary reads add work to
each solver-session handshake. Measure that cost against the preceding build
on native hosts, including the warm-cache fresh-project workload, before
acceptance; no negligible-overhead claim is made.

Failed attempts remain indexed here:

- 2026-10-17, `af339f8a` private Mac CLI / daemon source equal to `79e26818`:
  actual stale identity reproduced; original outcome remains failed.
