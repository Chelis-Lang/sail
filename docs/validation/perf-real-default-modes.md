# Real-project benchmark mode defaults

Source `48eaa4e0`, based on main `c96fdbec`, passes the full native workspace
gate on a dedicated EC2 c7i.8xlarge in us-east-1: 631 tests pass, with 35
solver-dependent tests skipped. All 504 tracked-file hashes match before and
after the run except excluded local agent settings. Affected internal Cargo
packages were cleaned and Rust/Cargo mtimes refreshed before rebuilding.
Required PR checks are enforced separately.

Only the benchmark's default mode list changes: `plain sail` replaces
`plain sail incremental`. The explicit historical `incremental` arm and its
reader remain unchanged. Shell syntax passes; re-rendering a retained
five-mode historical result is byte-identical to its prior Markdown report.
The original report's measured values are unchanged.

This removes one duplicate arm per default benchmark run. No new timing,
verification, solver, cache or conformance-harness behavior is claimed, and
no corpus pins change. No full benchmark or new corpus tier was run for this
mode-selection change.

Raw native archive: `perf-modes-native-native-evidence.tgz`, SHA-256
`8ab58557e2ed3e549cafdc2ca65c14968f1a484e6589ed269a6447444206f575`.
Historical-reader control: `legacy-reader-evidence.tgz`, SHA-256
`012874ff85c2189a6ac22775e7ac0c5916866a929ad5a4c73cc4bdbfa29e6203`.
Raw files remain outside git.
