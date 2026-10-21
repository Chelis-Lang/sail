# Source-supplied progress option fixture

Unmodified `source/rust_verify_test/tests/sail_vi0012_progress_order.rs` from
`Chelis-Lang/verus` commit `8a373f81a507b7ef4f144767ccd237d3d3de6c48`,
under the adjacent MIT license. SHA-256:
`046f7fb964c3a6dcb4cbccca1ec9cd61e4d4ed603d09b7976353f930fb6d8a63`.

The original test supplies `--no-report-long-running` itself. The source-suite
runner must omit its own duplicate only for this exact source, binary and
case. This fixture tests option selection; it is not compiled or counted as
native Verus regression coverage. The original fork test and its assertions
remain unchanged. Changed or missing source and unrelated cases retain the
runner's default.

The dispatch-order fixture is the unmodified
`source/rust_verify_test/tests/sail_vi0012_dispatch_order.rs` from the pinned
fork runtime source `f8589ed4345f147d8fb43d8cb7992921944b91f0`, under the same
MIT license. SHA-256:
`0eaec1bc54d7821f3026e4151bc07d7eafeff7b69cf3dc92e7a5c608536160db`.
Only its `sail_vi0012_dispatch_order_keeps_start_note_order` case supplies the
progress flag. Its other cases retain the runner default.
