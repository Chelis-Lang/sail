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
  from the installed runtime and the stock source build. The build creates
  its own toolchain manifest so source-built Cargo-new tests can resolve vstd.
- `cargo xtask build-verus-source`: the pinned upstream source used only as
  the independent oracle.

The first arm runs every fork test plainly and through `sail-z3`. Exit codes,
libtest outcomes, stdout and stderr must agree exactly after the existing
thread-id, libtest-duration and narrowly identified Cargo-new temporary-path
handling. A reordered diagnostic is a difference. Every nonignored fork test must also
pass; two identical libtest failures do not qualify the fork. Observed variation in the
fork is a determinism defect, never accepted as stock instability.

The second arm retains the original stock-with-and-without-shim control.
It compares complete diagnostic messages as a multiset under
[ADR 0042](decisions/0042-diagnostics-compared-as-a-set.md). Message bytes and
counts, all other stderr, stdout, outcomes and exit codes remain exact.
Stock instability follows [ADR 0009](decisions/0009-solver-nondeterminism-and-unstable-results.md):
the candidate must match a complete observed stock run. Unseen or hybrid
outputs remain differences. JSON test output is not normalized.

These source-suite arms do not compare stock and fork runtimes with each
other. That equivalence remains the corpus conformance harness's job, in
fast, standard and deep tiers. An attempted cross-runtime source-suite arm
was rejected because relocating an identical test executable changes its
reported input paths and runtime identity; those failed results are retained
in the [deep-tier record](validation/deep-tier.md).

`cache` adds fill and warm rounds of the exact fork arm. Each arm writes its
own report under `conformance-out/rust-verify-test/`, recording its source
build, upstream commit, fork commit when applicable, comparison policy and selected tests.
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
report `SAIL3040`; another owner of the selected build reports `SAIL3043`.

The fork arm obtains the pinned solver from the installed binary release
(`SAIL_VERUS_HOME`, or its normal cache location), then validates the selected
fork suite separately. The source selector is not passed through the ordinary
stock-source installer. The `--baseline stock` arm retains that installer's
explicit source override and Singular checks; its test suite still comes from
the default stock suite directory. A fork-suite selector therefore cannot
stand in for a stock installation. The release lookup still reports `SAIL3003`
when the release is missing.

Selection controls require Python 3.11 or newer and run against an already
built matching harness and the pinned solver, without executing any verification:

```sh
python3 scripts/test-rvt-selector.py --binary target/release/sail-conformance \
  --z3 "$SAIL_Z3_PATH" --out selector-controls.json
```

These isolated manifest and list-only executable fixtures test routing and
refusal. They do not qualify a source runtime or replace the real suite gates.
