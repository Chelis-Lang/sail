# Verus lists `--output-json --time` timings in a run-dependent order

**Found:** 2026-10-15, rescue lane L42 (determinism of the fork's Verus).

**Pinned versions:** Verus `0.2026.10.04.426d8b0`, Z3 4.16.0
(`corpus/toolchain.toml`).

## What happens

`test.rs` has eight functions in the root module and two in each of three
submodules. With `--output-json --time`, the `times-ms` report lists modules
by elapsed time and each module's `function-breakdown` in `HashMap` order.
`repro.sh` prints one digest per run of the *names* in those lists (the
times are left out):

```
$ ./repro.sh <install dir> 10
   1 15e81c1a8341e778
   1 19fcad6fed8b5cec
   ... (10 distinct orders in 10 runs, stock release, macOS arm64)
```

The Sail fork's pinned runtime (`13a1176c`) behaves the same on Linux x86-64
(10 orders in 10 runs). With the fork fix (VI-0017) every run prints one order.

## Impact

Byte comparison of `--output-json` stdout differs between identical runs even
after blanking the wall-clock numbers. See
`docs/verus-issues/VI-0017-timing-json-order.md`.
