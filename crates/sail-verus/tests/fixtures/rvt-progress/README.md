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
