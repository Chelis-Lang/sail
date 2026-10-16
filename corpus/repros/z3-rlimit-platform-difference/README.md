# Z3 4.16.0: an rlimit outcome that differs between macOS arm64 and Linux x86-64

**Found:** 2026-10-16, running seeded mutants of FPBench `f64` kernels
through the edit workload ([ADR 0053](../../../docs/decisions/0053-verus-mutation-corpus.md)).

**Pinned versions:** Verus `0.2026.10.04.426d8b0`, Z3 4.16.0
(`corpus/toolchain.toml`).

## What happens

`test.rs` is one generated crate: FPBench's `test02_sum8` and
`test04_dqmom9` (FPBench `d7ade34e`, MIT) as `f64` kernels with a small
user axiom set, and one mutation: in `test04_dqmom9`, the constant `-3.0`
of `t29` doubled to `-6.0`. `test04_dqmom9` carries
`#[verifier::rlimit(30)]`, which the unmutated kernel needs.

Plain stock Verus gives a different verdict on the two platforms, the same
in every run on each:

```
$ ./repro.sh <install dir> 3      # macOS 26 arm64, Apple M3, stock release
Z3 version 4.16.0 - 64 bit
   3 verification results:: 3 verified, 0 errors

$ ./repro.sh <install dir> 3      # Linux x86-64, EC2 c7i.4xlarge, CI image
Z3 version 4.16.0 - 64 bit
   3 error: function body check: Resource limit (rlimit) exceeded; ... verification results:: 2 verified, 1 errors
```

The Linux rows come from the source build of `426d8b0` in the CI image
(`sail-ci:3c02e445e25df766`); the edit workload's plain arm agreed in three
rounds. Four other mutants of the same generator behave the same way
(`fptaylor_extra` `floudas`, `sum` and twice `himmilbeau`); `floudas` also
carries `rlimit(30)`.

Not established: whether Verus sends Z3 the same bytes on both platforms,
or whether Z3 counts resources differently for the same input. The query
streams were not captured.

## Impact

An outcome near the rlimit measured on one platform does not hold on the
other. Sail's verdicts are unaffected: Sail matched plain Verus on the same
platform every time. Expected outcomes for mutant catalogues are measured on
the reference platform only (ADR 0053). See also the
[nonlinear rlimit reproduction](../z3-nla-rlimit-nondeterminism/README.md),
which varies within one platform.
