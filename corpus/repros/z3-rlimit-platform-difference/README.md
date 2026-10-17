# Z3 4.16.0: an rlimit outcome that differs between macOS arm64 and Linux x86-64

**Found:** 2026-10-16, running FPBench `f64` kernels through conformance and
the edit workload
([ADR 0054](../../../docs/decisions/0054-verus-mutation-corpus.md)).

**Pinned versions:** Verus `0.2026.10.04.426d8b0`, Z3 4.16.0
(`corpus/toolchain.toml`).

## What happens

`test.rs` is one unmodified generated crate: six FPBench kernels
(`fptaylor_extra`, FPBench `d7ade34e`, MIT) as `f64` functions with a small
user axiom set. `delta` (line 65) runs at the default rlimit.

Plain Verus gives a different verdict on the two platforms, the same in
every run on each:

```
$ ./repro.sh <install dir> 3      # macOS 26 arm64, Apple M3, stock release
Z3 version 4.16.0 - 64 bit
   3 verification results:: 6 verified, 0 errors

$ ./repro.sh <install dir> 3      # Linux x86-64, EC2 c7i.4xlarge
Z3 version 4.16.0 - 64 bit
   3 error: function body check: Resource limit (rlimit) exceeded; ... verification results:: 5 verified, 1 errors
```

On Linux the same result comes from the stock release, the stock source
build of `426d8b0` and Sail's fork runtime `73dea021`, three runs each, in
the CI image. Z3 answers `unknown` to `delta`'s check.

The outcome depends on the rest of the session: a crate holding only the
axioms and `delta` verifies on both platforms. On Linux every mutant of
this crate therefore fails at `delta`, including those that verify on
macOS.

Within one platform the solver input is deterministic:
`scripts/verus-determinism.py --runs 5` on Linux found identical solver
bytes in every run. Whether the bytes differ between the platforms, or Z3
counts resources differently for the same bytes, was not established.

## Impact

An outcome near the rlimit measured on one platform does not hold on the
other. Sail's verdicts are unaffected: Sail matched plain Verus on the same
platform every time. Expected outcomes for mutant catalogues are measured on
the reference platform only (ADR 0054). See also the
[nonlinear rlimit reproduction](../z3-nla-rlimit-nondeterminism/README.md),
which varies within one platform.
