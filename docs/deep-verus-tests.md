# Deep Verus tests

`ci/local.sh deep-rvt` runs Verus's own `rust_verify_test` suite on a native
Linux host. It is a local acceptance gate, never a required CI job. Use
`ci/run.sh deep-rvt` for a native host run, including macOS.

```sh
ci/local.sh deep-rvt
ci/local.sh deep-rvt cache
ci/local.sh deep-rvt --filter output_json::
ci/local.sh deep-rvt --every 10
```

The step builds two source variants with the pinned Z3 and toolchain:

- `cargo xtask build-verus-source --fork`: the exact fork commit from
  `engine/verus/fork.toml`, with the full test suite. Its cache identity binds
  that commit, source lock, platform and test-suite variant. It is separate
  from the installed runtime and the stock source build.
- `cargo xtask build-verus-source`: the pinned upstream source used only as
  the independent oracle.

The first arm runs every fork test plainly and through `sail-z3`. Exit codes,
libtest outcomes, stdout and stderr must agree exactly after the existing
thread-id, libtest-duration and narrowly identified Cargo-new temporary-path
handling. A reordered diagnostic is a difference. Observed variation in the
fork is a determinism defect, never accepted as stock instability.

The second arm compares stock with the fork through `sail-z3`, using the
**same stock test executable** on both sides. The runner physically copies
that executable beside the fork runtime, because Verus's test harness finds
its verifier and libraries relative to its own executable. Stock's working
directory and fixtures stay the same. This prevents changed fork tests from
changing the oracle's input or assertions. Temporary executable copies are
removed after the run, including errors.

The stock arm compares complete diagnostic messages as a multiset under
[ADR 0042](decisions/0042-diagnostics-compared-as-a-set.md). Message bytes and
counts, all other stderr, stdout, outcomes and exit codes remain exact.
Stock instability follows [ADR 0009](decisions/0009-solver-nondeterminism-and-unstable-results.md):
the candidate must match a complete observed stock run. Unseen or hybrid
outputs remain differences. JSON documents printed by the test harness are
not normalized, so stock's known JSON-order instability can remain visible.
Fork-only regression tests are listed separately in the stock report and
remain fully covered by the first arm.

`cache` adds fill and warm rounds of the exact fork arm. Each arm writes its
own report under `conformance-out/rust-verify-test/`, recording both source
builds, the upstream and fork commits, comparison policy and selected tests.
Filters and samples qualify only their stated scope; a release needs the
complete suite. The source variants use upstream's test-build layout, rather
than the release archive's packaging; release installation and diagnostic
checks remain separate gates.

For direct diagnosis, after building `sail-conformance` and `sail-z3`:

```sh
sail-conformance rust-verify-test --baseline fork --filter modes::
sail-conformance rust-verify-test --baseline stock --filter modes::
```

`SAIL_VERUS_SOURCE_BUILD` can select another fork-test build only if its
manifest matches the exact requested fork variant. A stock build or an
installed engine runtime cannot stand in for it. Missing or mismatched builds
report `SAIL3040`; another owner of either build reports `SAIL3043`.
