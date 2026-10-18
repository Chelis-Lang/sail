# Spinoff logging exceeds the filesystem component limit

Use the existing fork runtime `e5365c05`:

```sh
"$SAIL_VERUS" input.rs --crate-type lib --crate-name verifiable_controllers \
  --verify-only-module vdeployment_controller::proof::liveness::resource_match \
  --output-json --time -V spinoff-all --log air --log smt-transcript --log-dir logs
```

This panics with `dropped, expected call to into_inner instead`. Removing
`-V spinoff-all`, removing the two `--log` options, or replacing the long
function identifier with `short` makes the corresponding control pass.
The identifier appears twice in the spinoff log filename. Its `.air` component
is 311 bytes, beyond the 255-byte filesystem limit.

See [VI-0029](../../../docs/verus-issues/VI-0029-spinoff-log-path.md).
