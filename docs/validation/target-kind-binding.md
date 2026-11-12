# Cargo target-kind binding

| Field | Current state |
|---|---|
| Tracking | #722 |
| Defect | Main `4ec9521d` keyed kinds only by manifest/name; actual Cargo metadata for same_target has both lib/bin, while the map retains only bin |
| Change | Keep every target's name, kinds and Rust crate types; constrain an exact Cargo name when supplied, otherwise consider every normalized alias; require exactly one compatible target |
| Qualified source | `ed882f4e`, rebased onto current main as `b27b81092d5787e6a46d7b748e39f0bc39362150`; target-kind production source unchanged |
| Native Linux | c7i.4xlarge/us-east-2, Ubuntu24.04, CI Rust1.99.0: full gate1,350 pass/53skip; solver50; full E2E including extra; four PR arms each33 Same/311functions/seven drift units, zero unstable/different/infra |
| Current Linux | Rebased full gate1,375 pass/53skip; four current PR arms each33 Same/311functions/seven drift units, zero unstable/different/infra |
| Mac | Exact ed882: host clippy1,337 tests/53skip and book source/build/links pass; rebased b27b: host clippy1,362 tests/53skip pass |
| Live binding | Native Linux same-named capture_fixture lib/bin: selected has four lib events, binary_selected four bin events; independently rerun and inspected, 7,339 total events |
| Status | Implementation qualified; protected PR checks remain |

This changes observation identity, not solver inputs or verdicts. No corpus
entry changes and no performance claim. Unsupported or ambiguous identities
produce unavailable kinds. The private runtime document requires the new
metadata shape; legacy documents cannot provide a new binding. Multi-type and
proc-macro cases are synthetic selector controls, not real-project acceptance.

The native caller binding changes only two Runtime test-fixture fields in
`adapter.rs`. Every production byte, assertion and checked report-mode delegate
is identical to main. Independent reviews approve only the final exact
`MODE_NATIVE_SHA256`
`cb9e2f940f54aa87d6750279796571b27332187c878f37116a62176eb9298cdf`.
All78 checker controls pass under the reviewed in-memory hash advance;
production-delegate mutations still refuse. This is binding maintenance, not
a new direct-Stock proof/refinement execution.

Current report archive SHA-256:
`d4502aba5dfaed233c977574d6232ea90c6901b59d4832a52743b87a595e0f93`.
Current Linux gate/PR log:
`0a4b0633bbdb41c8fa62f9ffc5a457c3e54284e1bf5c8257b54e624a6a095ec6`.
Current Mac host log:
`366cd1d4257e9213ce6350e6490a9ecf66d439c367a51703ce652023e848ebd5`.
Corrected selector/native-binding review:
`4a35975146d16698a99507832a07c56ac1b20995e53e26c8937d514e59967f53`.
Final fixture-only binding review:
`2cae08def1e2fc3115bc673d8bfde3dabc0f0229ad41fc929514535abc411488`.
Raw logs, original failed runs and independent controls remain outside git.

| Earlier source/date | Result and cause |
|---|---|
| `31bdbd36`, 2026-11-12 | Independent review rejects absent-name alias selection: foo-bar/example and foo_bar/bin can resolve incorrectly to bin. Corrected selector refuses both same-kind and different-kind ambiguities; removed predicate still fails the controls. |
| `28f986e7`, 2026-11-12 | Full gate stops on unchanged native caller hash:78 Python controls, one failure/two errors. Exact first fixture-only binding independently reviewed. |
| `93128e57`, 2026-11-12 | Full gate1,349 pass/one fail: retained-plan JSON fixture has the old required field. Assertions and parser unchanged; second fixture and final binding independently reviewed. |
| Initial dual-target wrapper, 2026-11-12 | Invalid setup: hyphenated package gives different Cargo lib/bin names. Replaced with underscore-named package before the real shared-name control. |

Docs are this record, the native-binding record and changelog. Consumer
verification instructions stay unchanged because command behavior is unchanged.
