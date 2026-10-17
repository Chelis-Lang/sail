# Z3 4.16.0: nonlinear queries that are slow to stop at their rlimit

**Found:** 2026-10-16, generating the FPBench integer translation for the
mutation corpus pilot
([ADR 0054](../../../docs/decisions/0054-verus-mutation-corpus.md)).

**Pinned versions:** Verus `0.2026.10.04.426d8b0`, Z3 4.16.0
(`corpus/toolchain.toml`).

## What happens

Each crate holds one FPBench kernel as a `spec fn` over `int` and one
`by (nonlinear_arith)` lemma bounding it, and nothing else. The files are
generated outside this repository (FPBench `d7ade34e`, MIT) and checked in
unchanged.

| File | Layout |
|---|---|
| `delta_loose.rs` | kernel at the crate root |
| `kepler1_loose.rs` | kernel at the crate root |
| `kepler1_loose_split.rs` | the same kernel inside `pub mod kepler1` |

Plain Verus with the default rlimit either ends with an rlimit error after
minutes, or does not finish at all, on identical input. A run that is killed
is "did not finish", never a Verus outcome. `--rlimit 1` stops each query,
but only after seconds to over a minute.

Linux x86-64 (EC2 c7i.4xlarge, CI image, stock release):

| Crate | Command | Result |
|---|---|---|
| `kepler1_loose.rs` | default rlimit | rlimit error in 3 of 3 runs, 279–288 s (one run alone, two alongside other runs) |
| `kepler1_loose.rs` | `--rlimit 1` | rlimit error, 2.2 s |
| `kepler1_loose_split.rs` | default rlimit | rlimit error in 3 of 3 runs, 233–238 s |
| `delta_loose.rs` | default rlimit | did not finish within 900 s, 2 of 2 runs; in a third, Z3 was still running after 47 min |
| `delta_loose.rs` | `--rlimit 1` | rlimit error in 2 of 2 runs, 81–83 s |

macOS 26 arm64 (Apple M3, stock release):

| Crate | Command | Result |
|---|---|---|
| `kepler1_loose.rs` | default rlimit | rlimit error at 17–19 s in 3 runs; did not finish within 900 s in 1 run; still running after 26 min in 1 further run (killed) |
| `kepler1_loose.rs` | `--rlimit 1` | rlimit error, 1.3 s |
| `kepler1_loose_split.rs` | default rlimit | did not finish within 900 s, 3 of 3 runs |
| `delta_loose.rs` | default rlimit | did not finish within 900 s (1 run) |
| `delta_loose.rs` | `--rlimit 1` | rlimit error, 73 s |

```
./repro.sh <verus install dir> kepler1_loose.rs 3 900
./repro.sh <verus install dir> delta_loose.rs 1 900 --rlimit 1
```

`repro.sh` copies the crate to a temporary directory and runs
`verus --crate-type=lib [ARGS] <crate>` the given number of times. It kills
the whole process group, Verus and Z3, at the cap. Killing Verus alone
leaves Z3 running.

Not established: whether the cause is in Z3's nonlinear solver (the lemma
runs under `smt.arith.solver 6`) or in how often it checks the resource
budget, and why the same input stops on one run and not on another on
macOS.

## Impact

- **A cap can be mistaken for a verdict.** A harness must report a run it
  killed as "did not finish", never as an rlimit error or a failure. The
  mutation corpus leaves such lemmas and mutants out of its catalogues and
  lists them (ADR 0054).
- **Expected outcomes and run times near the rlimit are platform-specific.**
  The same crate stops after 19 s on macOS and after 280 s on Linux, or not
  at all; see also the
  [platform reproduction](../z3-rlimit-platform-difference/README.md).
- Sail's verdicts are unaffected: Sail forwards the solver's own answer, and
  conformance matched plain Verus on every crate that holds these kernels'
  other lemmas.
