# Managed final-stdout completion

## Status

The joined #215/#174 candidate is implemented and remains unselected. Executed
source `7411f458` combines output `354d2995` and scheduling facts on
`17464f16`. Accepted main `ee163bc3` (#225) differs from that baseline only in
three Markdown files; every executable body and mode is identical. The local
delivery rebase preserves every non-Markdown body and mode from `7411f458`.

| Joined qualification | Actual scope and result |
|---|---|
| Independent source reviews | Output and relevant-script scheduling reviews CLEAN; source review is not execution acceptance |
| Full macOS host | PASS: 759 tests, 35 solver/platform skips and all-target workspace Clippy on `7411f458`, 206.32 s |
| Native workspace gate | PASS checkpoint on `7411f458`, 83.09 s; complete enclosing qualification remains running |
| Solver, all nine E2E phases plus extra, PR subset and full FAST | PASS checkpoints: 9.09 s / 640.94 s / 54.60 s / 491.74 s; complete enclosing capture remains pending |
| Full STANDARD and final native guards | Current joined qualification running; no whole-pass claim |
| Current-pin Vest changed/unchanged costs | Fresh four-actor comparison running; the preceding complete-capture failure supplies no performance result |
| Selection | Pending complete native correctness, cost, capture and current-head CI gates |

The full Mac result is outside git at
`vest-output-scheduling-joined-qualification-2026-10-18/mac-host-01/result.json`,
SHA-256 `bb15223b5cdd4b806a5ba7128b854b7ddc8c9a69f7c0439119f593150bd505cb`.
Accepted-main equality is recorded in
`unscheduled-script-output-join-2026-10-18/combined-vest-source-prep/accepted-main-equality.json`,
SHA-256 `729c608a785db6ed0aae96bf0bc954ec140ca8429a5e1e84c000731341f1dfd7`.
Baseline #225 gates do not qualify this joined implementation.

## Behavior and controls

Keep user bytes on Cargo's original stdout and stderr pipes, including Cargo's
rendering and filtering of rustc JSON. Each managed driver appends a private,
fresh-capability completion marker after its final stdout record. The runner
removes only that exact marker and acknowledges after forwarding and flushing
the record, or intentionally suppressing human output for a structured caller.
A failed public write sends NACK; the driver cannot report successful completion.
The [proposed decision](../decisions/0055-acknowledged-managed-output.md) promises
that final stdout precedes the driver's exit and its subsequent Cargo error.
It makes no global order claim across stderr or independent Cargo jobs.

Matched production sender/relay controls run through actual Cargo 1.98.1 and
require complete captured/public stdout, stderr and exit equality. They cover
partial text, newline-terminated and unterminated valid JSON, and multiline
output with a 64 KiB JSON field. Cargo adds a newline to an unterminated stdout
record at EOF; the sender preserves that behavior before its separate marker.
Additional controls cover structured suppression, binary bytes, broken-pipe
NACK, lock serialization, malformed/foreign markers, private endpoint identity,
capability collision and the bounded missing-ACK path. A real grandchild holds
both pipe descriptors while cancellation must stop and join the readers without
waiting for EOF. Existing owned Cargo/compiler kill-and-wait guards remain.
There is no new dependency, query normalization, proof authority or receipt key.

The lock and private ACK have a 30-second bound. This does not bound ordinary
public-pipe backpressure or promise termination of every arbitrary descendant.
The real-Cargo test arms share one isolated owned `CARGO_HOME`; no lock-warning
line is filtered. Explicit test capture retains the complete fixture bytes
before assertions. The old missing historical streams are not reconstructed.

## Prior source scope

Standalone output source `9dff1ab4` passes nine production sender/relay controls,
the separate compiler cleanup control and Clippy, then the full Mac host
(752 tests, 35 skips). Focused result SHA-256:
`1cfe7d12b7155f06dc6d328956daabbef1d664f9f944ee4faa3c7c2aa8d869d3`;
full-host result: `ef734f53b96e8dc5a29f3d00f85b0c8c262de40a3c130c3826f299755eff6518`.
Its native gate, solver, all nine E2E phases plus extra and PR subset pass.
Full FAST passes all four arms, 147 Same / 3,450 functions each. STANDARD's
three completed arms pass 159 Same / 12,726 functions each; the engine arm has
no final report after its fixed 5,400-second deadline. This is incomplete
standalone qualification, not joined-source acceptance.

The complete failed standalone archive is outside git at
`managed-output-order-qualification-native-2026-10-18/isolated-cargo-capture-successor/result.tgz`,
SHA-256 `b168dacc2f9e1e225a626325a51a0c6749c968d54ccd40abf941146b160b2a8a`
(50,932,321 compressed / 855,678,177 raw bytes). Its source/runtime final guards
pass. The current joined run reserves the full prospective STANDARD cap; it
does not extend or reclassify that earlier attempt.

## Failed and interrupted attempts

Each line remains scoped to its original source and command; later success does
not erase it.

- `3892881a` plus payload candidate, 2026-10-18, attempt 01: disk admission refused; controls unrun.
- Same base, attempt 02: compilation interrupted after a regenerable Cargo path vanished; no unit result.
- Same base, attempt 03: five controls pass; the broken-pipe fixture wrongly required producer success after closure.
- Same base, attempt 04: six relay controls and compiler cleanup pass; test-only Clippy and a changing Cargo metadata guard fail the enclosing attempt.
- Same base, attempt 05: an under-specified deferred-buffer type fails compilation; no production behavior is accepted from this attempt.
- `8766ff71`, actual Cargo control: payload IPC bypasses JSON diagnostic rendering; rejected despite Cargo-like fixture passes. Result `40a756ef1c23ab422b80e66762e17a7d6454539c6c94b2f844f13e6d0e19f210`.
- `e3b1f42f`, focused 07: eight of nine controls pass, actual Cargo misses ACK; the old fixture deleted inner streams, so cause UNKNOWN. Result `f20057251f2bdedd939edb710764509794ddbfe13bb6717eda13348807f3a7bc`.
- `def2b553`, focused 08: zero selected tests; the required nine-test count rejects the result.
- Same source, focused 09: test-only missing Unix trait import causes E0599; no tests run.
- `2a523370`, focused 10: nine sender/relay controls plus compiler cleanup pass; three test-only Clippy findings fail the whole attempt. Result `936e24ab58b4c3d007f3c811afbf06e6ea198a521e2122961e6b4e14dc6f63d3`.
- `d26b6bb8`, native 01: local `operator.py` entered the remote package and shadowed the standard module; product gates unrun.
- Same source, native 02: exact real-Cargo comparison rejects two barrier-only package-cache lock lines; the successor isolates the arms without filtering. Separate raw-capture permission failure remains. Diagnosis `041031037b87acff380b34eb9c848df6c3839baa21b4f03962dd9e70f085a1f2`.
- `9dff1ab4`, standalone native: STANDARD engine arm interrupted at the fixed 5,400-second cap after three 159-Same arms; complete archive `b168dacc2f9e1e225a626325a51a0c6749c968d54ccd40abf941146b160b2a8a`.
- `7411f458` four-actor Vest comparison: all 36 calls and final product/runtime/cache guards complete, but strict whole capture rejects a hard-linked exported Sail inode. Detailed rows were lost; no timing or performance acceptance. Preserved top-level status SHA-256 `f8747031869c8ac85653103acde21145e5c9c5c137ea7720f065d1150ba43770`.

The actual relay fixture reversed 44 of 96 controlled causal writes, preserving
separate bytes (SHA-256 `145ac95ef790cbea0ee9b17e6e73f5f4d15b692b897999ae1d560c22955a3377`).
Actual Cargo also strips control-character framing bytes; the printable-marker
successor preserves its full comparisons. These reproduce concrete transport
problems, not the cause of #174's historical missing-stream failure.
