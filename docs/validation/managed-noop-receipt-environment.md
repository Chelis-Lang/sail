# Unchanged-call receipt environment binding

Status: all required local gates pass on native source
`1a28d671c66beecea6c909fb47e746c59a83425b`. The delivery updates only this
qualification record after that source; all non-Markdown tracked inputs and
Git modes remain identical. Native receipts retain their original source
attribution. Receipt version 9 and the selected schema-1 Verus runtime remain
unchanged; compiler-origin protocol and Cargo cleanup work are separate.

## Main witness

On main `d5a5d58baf65762871a7aafbf81c80563d438257`, a normal Cargo project
contains a ghost proof asserting that
`option_env!(" __VERUS_DRIVER_SHADOW__")` is absent, using `by(bit_vector)`.
The name has one leading ASCII space. With the variable absent, verification
succeeds and unchanged requests establish a genuine version-3 replay hit.
Changing only that caller variable to `source-visible-change` then produces
another replay hit: exit 0, Current, no verifier invocation and the old
confirmed function report. A fresh pinned-stock run on the identical source
and changed environment exits 1 with one verification error. The actual
replay record and every captured Cargo dep-info omit this ghost-only name.

The native x86-64 host was a `c7i.4xlarge` in `us-east-1`. The coherent
main/schema-1 pair used fork
`36d65c3f3161de34efa0150231a47c1777369a5a`, runtime manifest
`3b385a69339733ca0afc519a5179a985ed35575ea631c5c77f25764dafb0ec8a`
and pinned Z3 SHA-256
`e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765`.
The private evidence packet key is
`experiments/2026-10-28/acceptance-env551-main552-shadow-d5a5d58b-attempt1-native.tgz`;
SHA-256 `0e93ea6deeaf6a61ea38d03e229fc74b625c267113a824e6c2f076fc03f15e07`
(7,896,992 bytes). It retains actual commands, output, traces, version-3
records, admitted receipts, dep-info, source, stock solver logs and identity
guards. This is the existing [#552](https://github.com/Chelis-Lang/sail/issues/552)
reproduction; it does not exercise the separate compiler-origin gap in #551.

## Implementation

`runner::target_records_in` carries environment names from actually admitted
receipts and eligible staged invocation records into `noop::Facts`.
`noop::prepare` requires complete requirements for every included report before
reading recording inputs, unions those names with dep-info declarations and samples caller values.
Version 4 rejects earlier replay records without changing receipt admission,
query bytes, solver answers or source/runtime stamps.

Tests exercise direct, sibling and seeded admissions, rejected declarations,
restored zero-compiler invocations and unknown/artifactless observations.
Missing targets omitted from a human Cargo-output result confer no returned
proof authority: their absence does not suppress the known admitted subset.
Structured requests retain their separate retry and complete requested-scope
checks, and the replay key binds the output mode. Any included report without
known requirements still prevents recording.

Recording additionally requires a strict current invocation-directory read:
missing directories, entry/read errors, unknown records or any `.started`
entry refuse. Only an empty directory or readable cached invocation JSON
records qualify. The public execution counter remains unchanged; its
best-effort directory read alone is insufficient evidence for recording.
Separate child callers exercise actual prepare/commit/replay with an unchanged
key: unchanged input hits; changed input, unknown coverage and version 3 refuse.
These are unit regressions, not real-project acceptance.

## Native qualification

The same native `c7i.4xlarge` in `us-east-1` used the selected old fork/runtime
and pinned Stock/Z3 identified above. No corpus entries changed.

| Gate | Result |
| --- | --- |
| Focused regressions | 6 admission/subset/inventory tests and 9 no-op tests pass |
| Workspace gate | Strict Clippy, 1,170 nextest tests, doctests/rustdoc, checkers, layering, deny and generated benchmark page pass; 54 nextest tests skipped |
| Solver/fault tests | 51 pass with the normal three separately qualified compiler-boundary exclusions |
| Full E2E | All original local phases plus extra pass; includes unchanged/focused replay, 84 managed verification scenarios, real MCP tool listing and repair acceptance |
| Public changed-input witness | Actual v4 hit, then one fresh compiler and confirmed Failed/SAT after the caller input changes; fresh pinned Stock fails too |
| PR fast conformance | Every arm: 33/33 same, 311 functions compared, zero unstable/different/infrastructure errors, zero skipped projects |
| Full fast conformance | Every arm: 147/147 same, 3,451 functions compared, zero unstable/different/infrastructure errors, zero skipped projects; engine has 146 focused units |
| Standard conformance | Every arm: 159/159 same, 13,166 functions compared, zero unstable/different/infrastructure errors, zero skipped projects; engine has 158 focused units |
| Mutation controls | All 18 HumanEval mutant comparisons, original revert and forced revert pass |

The conformance reports retain resource-limit drift in 5 PR-fast units,
11 full-fast units and 17 standard units. These are not performance samples.

The normal E2E build and its first public witness used Sail executable SHA-256
`c83d529646ba47790ca9f65cbb0260d3302a097d228e585e3cd66830fbcc6448`.
The later conformance/mutation executable was
`91bca30ef18095ada2267ca05f8f3c4c0ee4625a7f2c0123caa32e97cd0ed6b5`;
that exact value is recorded in the mutation header. These normal E2E/conformance build commands selected different Cargo
dependency fingerprints; the executables are not labeled byte-identical. Both original binaries and
fingerprints are retained.

A separate normal production build ran exactly
`cargo build --release --locked -p sail-cli -p sail-shim`, matching the release
entrypoints. It selected executable `91bca30e…`, byte-identical to the mutation
executable. Its strict public witness repeated on a fresh project/cache and
passed all original assertions. The shim stayed
`dbbe700b86475a86d3aa92587fb9676fb7719a1918a1d1d0a07ff39783915cd4`.
Source, toolchains, lock and old-fork runtime were unchanged. Actual command,
binary, before/after fingerprint and feature-tree receipts preserve each
scope separately.

The final public witness uses the identical main-reproduction source bytes.
Its actual version-4 record includes the leading-space ghost name with an
absent value; Cargo dep-info still omits it. After only that caller value
changes, the same key misses with `a declared variable changed`, starts one
actual same-unit compiler with a new run identity, and returns Current,
Cargo exit 101, nested verifier exit 1 and a single confirmed failed function
with a concrete SAT obligation. Actual `.started`, invocation and raw Verus
records establish that fresh attempt independently of the public counter.
The old own receipt and replay record are removed. Fresh pinned Stock on the
same source/value exits 1 with one error. Source, binary, tool/runtime
inventories and literal command/output hashes are retained.

The best private evidence packet is
`experiments/2026-10-28/acceptance-main552-noop-1a28d671-release-final-native.tgz`;
SHA-256 `8471d87387a76a7977d5d152440ddb5d1bc957123469ba50ecfc1b5d8aec8fe1` (`306,569,716` bytes).
It includes the original failed attempts, final literal commands/streams,
actual candidate binaries, reports, solver inputs, source bundles and guards;
authenticated upload/readback hashes match.

All 171 enrolled assembled refinement programs and model/checker inputs are
byte-identical to accepted main. This transfers existing proof-input evidence;
it does not prove the new carrier or invocation-inventory helper. No overall
service theorem, separate performance result or compiler-origin fix follows
from this qualification.

## Attempts

- `d16bb185`, 2026-10-28: native focused tests passed (4 admission tests and
  9 no-op tests); workspace gate failed on a new test's bare empty-vector
  assertion (`clippy::assert_is_empty`). The assertion is replaced with exact
  vector equality; this is a PR-local test fix. The failed attempt is retained.

- `b0086aa0`, 2026-10-28: workspace gate passed (1,168 nextest tests;
  54 skipped), solver/fault suite passed (51 tests; the normal three
  compiler-boundary exclusions). Full E2E failed when a shown human Cargo
  no-op omitted a target with no matching receipt and the new carrier refused
  to record its known returned subset. Original assertions remain unchanged;
  the carrier now scopes requirements to returned reports. The failure
  collector retained bounded evidence and reported its selected-file limit.
- `b0086aa0`, 2026-10-28: an independent public JSON diagnostic reached a
  real v4 hit, then Sail exit 101 and fresh stock exit 1 on the changed input;
  the controller failed during final recording because a new compiler-start
  list shadowed its monotonic timestamp. Its raw calls and compiler records
  remain evidence of that failed attempt, not a completed qualification. The
  local variable is renamed without changing any test assertion.

- `dd472f68`, 2026-10-28: focused controls passed strict inventory and
  included-unknown/artifactless boundaries; three route tests failed on their
  old `None` expectation for a rejected receipt returning no reports. Receipt
  rejection and missing-target checks passed. Only that carrier expectation
  becomes `Some(empty)`, matching the reviewed returned-subset contract; every
  included unknown report still refuses recording.
