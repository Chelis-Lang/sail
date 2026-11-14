# Publication explanation comparison

Status: native full workspace/solver/E2E/PR and standard gates, including the
owning mutation sweep, pass at7b68ef69. Fresh bounded independent review passes.
Tracks #748 and ADR0142. Runtime/tool sources remain identical in documentation
successors; no new verifier behavior or performance acceptance is claimed.

The benchmark separates only the production-defined publication explanation
schema, with one occurrence per typed stage. Original stderr remains captured;
all verifier diagnostics and unknown/malformed explanations stay comparable.
The producer and adapter share one vocabulary without a new dependency edge.

The original native efff77ec run passes all four147-unit fast and four159-unit
standard corpus arms; its owning18-mutant sweep fails five of21 steps solely
on the additional publication note. Every false-verified list is empty. All
32,698 original payloads are lead-rehashed and externally retained, archive
`57c32df7db6a02a72450c67d3b35651276b205aebd9dcadf31bb5bdbafe6ba1a`.
Those original sweep failures remain failures.

Focused controls exercise every produced stage, canonical positive u64 count,
maximum count, malformed/overflow counts, unknown wording and code, exact
remaining diagnostic bytes, independent stages and repeated-stage refusal.
Native c7i.4xlarge/us-east-2 qualification at7b68ef69 passes the complete
workspace gate (1,413 tests /58 declared skips),54solver tests, complete local
E2E including Extra/all managed/repair and all four PR arms (33Same/311functions,
zero different/unstable/infra/skipped). All four standard arms also pass:159Same/13,166functions each,32resource-drift
units and zero different/unstable/infra/skipped. Its full21-step owning sweep
(18mutants plus cold/revert/forced revert) passes, all comparison results Same
and every false-verified list empty. Complete runner exit0 is retained. The
lead independently rehashes all28,847 original payloads,5,329,561,478raw bytes,
and retains the archive externally:
`efce30fc0175a0fdafa46ca7fc034e16b1733b99590028d8452be92dac39fad9`.
The initial collector accidentally selected older runner-log names; the five-file
supplement retains the actual original log, exit, wrapper and source bundle,
independently rehashed and retained externally:
`57bc4e9f34cc3c6a1e8c96400cfd6ef5bb00527574b67f31786375eca65cb2aa`.

Fresh independent review passes baseline13benchmark/9publication tests,
candidate15/10tests and five independently authored actual-crate tests. It
checks64valid/528malformed notices,256ordered stage pairs,19schema line-ending
boundaries, raw execution capture, actual comparison and64producer/emission
policy cases. The lead rehashes all4,239 indexed originals, the embedded index,
six symlink targets without following them and74local review payloads. Archive
`44e86fb2ebac581eeadf44d5b5d1e92f54c00973b1c8638d43a3b059b0038352`.
The invalid first producer wrapper remains preserved and excluded. All six hosted checks pass at the preceding documentation head; the final
documentation amendment must pass them again before merge.

Documentation: ADR0142, testing guide, changelog and this record. No CLI/MCP
commands or release availability change; the consumer skill needs no change.

## Retained failures

| Source / attempt | Actual result |
|---|---|
| `0d05474b`, native1 | Focused production/benchmark tests pass; workspace gate fails Clippy `assert_is_empty` in the new test. No solver/E2E/standard pass claimed. Successor uses an informative equality assertion. |
| `0d05474b`, independent review | Exact line-ending path strips repeated CR bytes before recognition; original controls are retained. Successor removes at most one LF or CRLF terminator and requires extra controls to remain diagnostics. The successor independent review passes as recorded above. |
