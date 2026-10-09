# 0019: `rust_verify_test` conformance from a Verus source build

- **Status:** accepted
- **Date:** 2026-10-09

## Context

ADR 0010 (item 4) put Verus's own test suite, `rust_verify_test`, in the
deep tier only. The corpus survey (`docs/survey/corpus.md` §1.3) found:
- it is the widest feature-coverage set Verus has, with many expected
  errors;
- its tests embed Verus code in Rust test macros, and the harness
  (`tests/common/mod.rs`) runs only against a source build: it finds
  `rust_verify`, `vstd.vir` and the macro libraries relative to its own
  executable, in `source/target-verus/<profile>`;
- it passes `VERUS_Z3_PATH` through to `rust_verify`, so the suite can run
  through `sail-z3` unchanged;
- `VERUS_KEEP_TEST_DIR=1` keeps each test's materialized input and prints
  how to rerun it.

Facts found while building it at the pinned commit (`426d8b0`):
- **Layout.** Verus's `source/.cargo/config.toml` sets `target-dir =
  "target-verus"`, so the test executables are in
  `source/target-verus/release/deps`, next to 1.1 GB of intermediates.
- **Size.** The suite has 4,695 tests in 142 test binaries; 58 are
  `#[ignore]`d.
- **Speed.** Most tests take a fraction of a second. The 19 `cargo::`
  tests drive `cargo verus` on small crates, each with a fresh temporary
  target directory, so each one builds (and its `verify` step verifies)
  vstd from scratch: minutes per run.
- **Two pieces of the output vary between identical runs,** and neither is
  Verus output: the OS thread id in a panic message (`thread 'name'
  (12345) panicked at`, Rust 1.98) and libtest's `finished in 0.52s`.
- **Temporary paths.** The two `cargo::cargo_new_*` tests fail plainly in
  a source build (`cargo-verus new` reports "no known toolchain manifest
  for Verus version 0.2026.10.04.426d8b0"), and their panic message
  names a `tempfile` directory with a random name, so plain runs of them
  never agree byte for byte.
- **Error order.** Verus verifies modules on several threads and prints
  each module's errors as it finishes, so errors from different modules
  come out in a run-dependent order (VI-0007). In the first full run (with
  Verus's default threads and 2 confirming plain runs), plain Verus
  disagreed with itself on 11 tests besides the two above, and 7 of them
  were first reported as `different`, because no plain run had happened to
  produce sail's order. None was a sail difference: rerun with up to 30
  confirming plain runs, 4 matched a plain run; for the other 3, 20 plain
  runs each produced sail's exact stderr several times (5 to 9 of 20).
- **One thread is stable but reports less.** With `--num-threads 1` the
  error order is the same in every run. But Verus then reports fewer
  errors on a few inputs: three tests whose expected outcome counts errors
  in two modules (`recursion::termination_checked_across_modules`,
  `exec_termination::mutually_recursive_exec_functions_in_different_modules_need_decreases_clause`,
  `state_machines::test_state_machine_req_custom_err_is_inert`) get one
  error instead of two and fail, plainly and under sail alike.
- **Other plain variation remains.** The `output_json` tests print
  Verus's `--output-json` document, whose `func-details` come out in hash
  order (VI-0008): one test showed 4 orders in 40 plain runs, even with one
  thread. Tests without `--no-report-long-running` can print timing notes
  ("has been running for 2 seconds") under load.

## Decision

**The build.** `cargo xtask build-verus-source` builds the pinned commit
(`corpus/toolchain.toml` `[verus].commit`) with Verus's own tooling, as
`BUILD.md` describes:
- **Location.** `<cache>/verus-src-build/<commit>`, never inside the
  repository: the commit is fetched shallowly into `verus/` there.
- **Toolchain.** `rustup toolchain install` in the checkout installs
  exactly what Verus's `rust-toolchain.toml` lists (1.98.1 with
  `rustc-dev`, `llvm-tools`, `rustfmt`). Its channel must equal
  `rust_toolchain` in `corpus/toolchain.toml`.
- **z3.** `tools/get-z3.sh` is never run. `VERUS_Z3_PATH` names the
  pinned z3 of the installed release (`cargo xtask fetch-verus`), checked
  by SHA-256 before the build and again after `vargo` copies it into
  `target-verus/release`.
- **Steps.** Build `vargo` (`tools/vargo`), then `vargo build --release`
  (Verus, and vstd built and verified), then `vargo test -p
  rust_verify_test --release --no-run --message-format=json`, whose
  messages name the test executables. At most 4 Cargo jobs
  (`SAIL_BUILD_JOBS` overrides).
- **Pruning.** Unless `--keep-intermediates`, everything the test
  executables do not need at run time is deleted: vargo's build,
  `source/target`, and in `target-verus/release` the `build`,
  `incremental`, `.fingerprint` and `examples` directories and every file
  in `deps` except the test executables. 1,481 MiB becomes 471 MiB.
- **Manifest.** `sail-build.json` (schema 1, `sail_verus::source_build`)
  records the commit, toolchain, z3 SHA-256, build time and the test
  executables. A complete build matching the pins is left alone; `--force`
  rebuilds.

**The runner.** `sail-conformance rust-verify-test` lists every test
(`--list --format terse`, plus `--ignored` for the ignored ones) and runs
each one alone, as `<binary> --exact <test> --nocapture --test-threads 1`
in the `rust_verify_test` package directory, with the environment `vargo
test` gives the suite (`VERUS_IN_VARGO`, `RUSTC_BOOTSTRAP`,
`RUST_MIN_STACK`), Verus's toolchain, and its library directory on the
dynamic-library path (`sail_verus::rvt`).
- **One verification thread by default.** Both sides get
  `VERUS_EXTRA_ARGS="--num-threads 1"` (the harness appends it to Verus's
  arguments), so errors come out in a stable order (VI-0007). Like `-Z
  write-long-types-to-disk=no` for corpus units (ADR 0006), this removes
  variation at the source, and both sides run with identical arguments.
  The cost is the three tests above, which fail on both sides. A
  difference is then far less likely to be noise, which matters more for
  a conformance check than three expected-outcome assertions (conformance
  compares sail with plain Verus, not with the tests' expectations).
  `--verus-threads 0` runs Verus's default instead, and should be run too
  before a release: it covers those three inputs fully, and its error-order
  variation is classified as unstable.
- **Two sides per test.** Plain (`VERUS_Z3_PATH` = the pinned z3) and sail
  (`VERUS_Z3_PATH` = `sail-z3`, `SAIL_Z3_PATH` = the pinned z3, and
  `SAIL_DAEMON_SOCKET` when the harness has one). Both sides of a test run
  back to back in one worker, rather than as two whole-suite passes, so
  load affects them alike and they never share a test's input directory
  concurrently. Ignored tests are listed with outcome `ignored` and not
  run.
- **What must be equal:** libtest's outcome (`ok`, `FAILED`, `ignored`),
  the exit code, stderr byte for byte (every diagnostic Verus reported, as
  the harness renders it, and any panic message), and stdout (libtest's
  report). The only text blanked is the panic thread id, libtest's
  timing, and the six-character tempfile name in the two cargo-new tests'
  known missing-manifest panic (the complete message and path suffix are
  retained). These are not Verus diagnostics.
- **Flakiness is classified, never hidden** (ADR 0009). On a difference the
  plain side runs again, up to `--confirm-runs` times (default 10: tests
  are cheap, and a 4-way variation escaped 5 confirming runs once). The test is
  `unstable` if the plain runs disagree among themselves and sail matches a complete observed plain run after the documented
  normalization. An unobserved message or a combination of lines from
  different plain runs remains `different`. Otherwise it is
  `different`. Unstable tests are listed in the report and summary.
- **Reproductions.** For a `different` test the runner saves every run's
  stdout and stderr under `<out>/tests/<test>/`, reruns the test plainly
  with `VERUS_KEEP_TEST_DIR=1`, copies the materialized input to
  `input/`, and writes `reproduce.txt` with Verus's own rerun line and the
  exact plain and sail commands.
- **Report.** `report.json` (schema 1) with machine details and per-test
  verdicts and timings. Exit 0 if no test is `different`, 1 otherwise, 2
  on infrastructure errors (a test that cannot be run or exceeds
  `SAIL_UNIT_TIMEOUT`).

**Where it runs.** `ci/run.sh deep-rvt [cache] [ARGS...]` builds (or
reuses) the source build, then runs the suite pass-through; with `cache`
it then runs it twice more with a fresh cache daemon (fill, warm). It is
local-only, refused when `CI=true` (ADR 0013), and runs through
`ci/local.sh` in the CI image or on the host. `ci/run.sh
build-verus-source` builds only. The CI image is unchanged: the build
installs `rustc-dev` and `llvm-tools` with rustup when it runs, and running
the tests needs only the `rustc` component the image already has.

**Singular is not built.** Verus's `singular` feature is a plain Cargo
feature (no build dependencies), so `vargo build --features singular`
would work. Using it for the corpus's Singular entries
(`examples/integer_ring/*`, `verified-memory-allocator`) would also need:
- the Singular binary at run time (`VERUS_SINGULAR_PATH`): Ubuntu's
  `singular` package in the image or installed at run time, and a
  separate install on the macOS host;
- a second source build (another 12 to 18 minutes and about 500 MiB);
- the corpus harness pointed at a source build's `verus` launcher instead
  of the release's, which `Installed` does not support yet.

That is more than a modest addition, so it is left for a later ADR.

## Consequences

- The deep tier gains Verus's whole feature-coverage suite, with
  diagnostics compared byte for byte.
- The first `deep-rvt` builds Verus: 12 minutes on the development
  machine (macOS arm64, 4 jobs, load average 30 to 50 from other work) and
  18 minutes in the CI image under amd64 emulation on the same machine.
  Later runs reuse the build.
- The `cargo::` tests dominate the run time (about 25 of 32 minutes on
  the host), and two of them fail plainly in a source build; both are
  upstream behaviour, recorded here rather than worked around.
- Expect a handful of `unstable` tests per full run (random temporary
  paths, JSON order, timing notes); each is listed with its runs saved, and
  a growing count is a signal to look at.
- A Verus pin change needs a new source build (`<cache>/verus-src-build`
  is keyed by commit); old builds can be deleted by hand.

## Results

Verus `0.2026.10.04.426d8b0`, Z3 4.16.0, 2026-10-09. Development machine
(macOS arm64, 8 cores, shared: load average 20 to 50 throughout), 4 tests
at a time. No run found a sail difference.

| Run | Tests | Same | Unstable | Different | Wall |
|---|---|---|---|---|---|
| Host, full, Verus's default threads, 2 confirming runs | 4,695 (58 ignored) | 4,682 | 4 | 9, all plain variation (below) | 51 min |
| Host, full, `ci/run.sh deep-rvt` (one thread, 5 confirming runs) | 4,695 (58 ignored) | 4,687 | 8 | 0 | 32 min |
| CI image (`ci/local.sh deep-rvt --every 20`, amd64 emulation) | 235 (4 ignored) | 234 | 1 | 0 | 4 min |
| Host, `deep-rvt cache --every 5`: pass-through | 939 (10 ignored) | 937 | 2 | 0 | 5 min |
| same, cache daemon fill | 939 | 937 | 2 | 0 | 4 min |
| same, cache daemon warm (50,948 cache entries) | 939 | 937 | 2 | 0 | 3 min |

- **The 9 first-run differences.** Two were the `cargo::cargo_new_*`
  temporary paths, before the by-line unstable rule existed. The other 7
  were error order (VI-0007) and `func-details` order (VI-0008): with up
  to 30 confirming runs 4 matched a plain run, and for the other 3, 20
  plain runs each produced sail's exact stderr 5 to 9 times.
- **One earlier cache-fill difference**, in a run with 5 confirming runs,
  was `output_json::test_json_proof_note_on_assume_and_req_with_no_cheating`:
  sail's output was one of the 4 plain variants (9 of 40 plain runs). The
  runs in the table above used 10 confirming runs.
- **Unstable tests** in the one-thread full run: `cargo::cargo_new_builds`
  and `cargo::cargo_new_verifies` (temporary paths), three `output_json`
  tests (VI-0008), and `bitvector::div_mod_signed_properties`,
  `core_special_setup::test_is_core` and
  `examples::examples_cuckoo_hash_table_cuckoo` (timing notes).
- **Plain outcomes** (one thread): 4,632 ok and 5 FAILED, the same on both
  sides: the two `cargo_new_*` tests and the three tests that need two
  threads' errors (above).

## Strict comparison correction (2026-10-09)

The original varying-line rule could accept an arbitrary new diagnostic
on a line where plain output varied. It is removed before merge. The
regressions reject both an unseen message and a hybrid of two plain outputs.
The historical results above used that broader classifier; the strict
rerun results below supersede them for acceptance.

The same `--no-report-long-running` flag is supplied to both sides, as in
ADR 0006. It suppresses timing-triggered Verus progress and completion notices
at their source; no Verus diagnostics are removed by the comparator.

## Exclusive build ownership

A targeted normal-thread run overlapped the full suite during takeover testing.
Both suites called `clear_inputs` on the same materialized input directory,
causing missing-file panics in the other suite. Those results were discarded.
The runner and source builder now hold the same exclusive OS file lock for
the entire operation (including cleanup). A second operation fails with
`SAIL3043` before touching the shared directory; process exit releases the lock.
No dependency or unsafe code is needed. Normal-thread coverage runs separately.
