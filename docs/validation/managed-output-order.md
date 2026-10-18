# Managed final-stdout completion

## Status

The printable-marker original-pipe successor for #215 is implemented in source
only and remains unbuilt and unselected. The preceding control-character marker
candidate failed its actual-Cargo test despite eight other focused controls
passing. It restores Cargo's original handling of every stderr
byte, then acknowledges only each driver's final stdout through a same-stdout
completion marker. Proof metadata, output filtering, receipts, runtime pins and
reuse checks are unchanged. No corpus entry changes. The complete local and
native gates, real Verus output cases and native overhead remain pending.

The [proposed decision](../decisions/0055-acknowledged-managed-output.md) claims
final stdout delivery before that driver exits and allows its subsequent Cargo
error. It makes no global stdout/stderr or independent-job ordering claim.

## Evidence and limits

An actual main-relay fixture reversed 44 of 96 combined causal writes while
preserving separate bytes. It wrote a large filtered Cargo JSON record, a
summary and then a Cargo-like error. This is a controlled relay reproduction,
not real-project Verus acceptance or attribution of #174's original failure.
Raw SHA-256: `145ac95ef790cbea0ee9b17e6e73f5f4d15b692b897999ae1d560c22955a3377`.

The superseded payload-IPC candidate `8766ff71` passed eight focused macOS
production-sender/relay and cleanup controls with a Cargo-like child and release
Clippy. Raw result: `c90c892df3a4d9a133d8138d2fe42723b9fd335878455a9b5e50168fe67c2448`.
Those fixtures did not exercise Cargo's real diagnostic parser. A subsequent
actual Cargo 1.98.1 control demonstrated that raw rustc JSON stderr is rendered
and filtered before becoming public stderr. Payload IPC bypasses that behavior;
its fixture pass does not qualify it. Both original and marked commands retain
exit 101 and their complete raw streams. Result SHA-256:
`40a756ef1c23ab422b80e66762e17a7d6454539c6c94b2f844f13e6d0e19f210`.
The first wrapper also introduced a jobserver warning; the later control
preserves inherited jobserver descriptors and does not reproduce that warning.

Four actual Cargo boundary controls establish the narrower mechanism: a
same-stdout marker after either a complete or incomplete line is observed and
acknowledged before wrapper exit. Plain Cargo adds a newline to an unterminated
stdout prefix at EOF. A JSON diagnostic marker is rendered on stderr but does
not flush an earlier incomplete stdout prefix. Raw result SHA-256:
`89fd8e464d32086740544ac3a4956e05a2e54e419181c418ec46a8f2a014a8df`;
assessment: `8ddf3cc99625841dcf4b12d99d62c573623fad2a9c67372204578ff9a66c9e80`.
These are tiny Cargo transport controls, not verifier or performance acceptance.

The new source tests require the real production sender and relay through
matched plain/barrier arms through actual Cargo with byte-for-byte equality of
complete captured and public stdout/stderr and exit status, final stdout delivery
before Cargo's final error, and the observed unterminated-line behavior. The
expanded unrun matrix covers plain partial text, valid JSON with and without a
final newline, and multiline output with a 64 KiB JSON field. The sender completes
an unterminated record with Cargo's EOF newline before a separate marker, keeping
JSON parsing independent of the marker. Additional
owned-pipe controls retain binary bytes, a large complete filtered JSON record,
structured-output suppression, broken-public-pipe NACK and endpoint cleanup,
lock serialization and release after NACK, symlink/capability collision refusal,
malformed/foreign markers, and the bounded missing-ACK path. A real grandchild
retains both Cargo pipe descriptors while a failed relay must stop and join
its readers without waiting for EOF; the test parent then reaps its owned
grandchild. Cancellation ownership and process groups are unchanged. They are unrun.
The old broader concurrent-cross-stream and payload-chunk fixture claims do
not apply to the new protocol.

The current real-Cargo fixture writes captured/public/combined streams and
status before assertions. An explicit `SAIL_VISIBLE_TEST_EVIDENCE` directory
retains complete owned fixture file snapshots with path/mode/size/hash indexes
under an 8 MiB cumulative cap. Known FIFO metadata is indexed without opening
the pipe. A failed or absent external capture preserves the original failed
fixture tree and prints its path; it does not silently remove uncaptured raw.
Printed diagnostics have a 64 KiB prefix bound per stream, with full bytes
retained in the fixture files. This retention is test-only.

A separate actual Cargo 1.98.1 tiny control preserves an ASCII marker but strips
its surrounding record/unit-separator bytes (`0x1e`, `0x1f`) from stdout. The
exact control-character ACK therefore times out; raw result SHA-256:
`c5cc33b95843f7e5df4d8a0fc65f57e3de906bb80fab6fa9f88226d76cef416f`.
This establishes an independent framing defect. It does not reconstruct the
missing inner streams of focused attempt 07. The successor uses printable,
fresh-capability framing and retains all exact full-stream and exit checks.

## Failed and interrupted attempts

- `3892881a` plus payload candidate, 2026-10-18, attempt 01: disk admission
  refused; controls unrun.
- Same base, attempt 02: interrupted compilation after a normal transient Cargo
  release path vanished; no unit result.
- Same base, attempt 03: five controls pass; the broken-pipe fixture incorrectly
  required its producer to succeed after pipe closure. The corrected fixture
  retained NACK and cleanup checks.
- Same base, attempt 04: six relay controls and compiler cleanup pass; four
  Clippy errors and Cargo's regenerable `.rustc_info.json` update fail the
  enclosing attempt. Protected bodies remain unchanged.
- Same base, attempt 05: under-specified deferred-buffer type fails compilation;
  attempt 06 supplies its explicit type and passes the focused scope above.
- `8766ff71`, actual Cargo control: payload IPC bypasses diagnostic rendering;
  rejected despite the prior Cargo-like fixture passes. The original #174
  complete temporary streams remain unavailable and its cause stays unknown.
- `e3b1f42f`, 2026-10-18, focused attempt 07: eight of nine tests pass,
  including protocol/refusal and descendant-FD cleanup controls. Actual Cargo
  fails at the missing barrier-completion record; exit 101 is retained, but the
  fixture deleted its inner streams on assertion failure, so the exact cause
  stays unknown. Source/protected-file/child guards pass; Clippy is unrun after
  the unit failure. Raw result SHA-256:
  `f20057251f2bdedd939edb710764509794ddbfe13bb6717eda13348807f3a7bc`.

Acceptance still requires passing/failing/compile-error/unchanged real Verus
runs through plain and managed commands, strict combined and separate byte
comparisons, JSON/MCP behavior, closed-pipe handling, all nine E2E phases plus
extra, workspace/solver gates and applicable FAST/STANDARD conformance. Measure
native overhead before selection. Raw originals remain outside the repository.
