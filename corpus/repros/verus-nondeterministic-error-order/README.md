# Verus reports errors in a run-dependent order

**Found:** 2026-10-09, in the first `rust_verify_test` conformance run
(ADR 0019), where plain Verus disagreed with itself on several tests.

**Pinned versions:** Verus `0.2026.10.04.426d8b0`, Z3 4.16.0
(`corpus/toolchain.toml`).

**Machine:** macOS arm64 (8 cores).

## What happens

`test.rs` is the input of `rust_verify_test`'s
`traits_modules::test_verify_6` (Verus repository at the pinned commit,
`source/rust_verify_test/tests/traits_modules.rs`), as the test harness
materializes it (`VERUS_KEEP_TEST_DIR=1`). It has two verification errors,
in different modules (`M5` and `M7`). Verus verifies modules on several
threads and prints each module's errors when it finishes, so the order of
the two errors in stderr depends on which thread finishes first:

```
$ ./repro.sh <install dir> 12 8
   5 5c4830ab92eccedf
   7 6972dbd642b5312c
$ ./repro.sh <install dir> 12 1
  12 6972dbd642b5312c
```

With `--num-threads 1` the order is the same in every run. Across runs with
the same thread count the verdicts and the set of errors never change, only
their order.

## Impact

Byte-for-byte stderr comparison (ADR 0006, ADR 0019) sees plain Verus
disagree with itself on any input with errors in more than one module. The
`rust_verify_test` runner passes `--num-threads 1` to both sides by default,
which removes this variation at the source without changing what is
compared. Corpus units keep Verus's default and rely on ADR 0009's
unstable classification. See `docs/verus-issues/VI-0007-nondeterministic-error-order.md`.
