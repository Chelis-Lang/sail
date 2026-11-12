# Platform-specific rlimit outcome

The six-kernel floating-point source preserves a historical platform difference: the same stock release reported six verified on macOS arm64 and five verified plus one rlimit failure on Linux x86-64. The `delta` query behaved differently when the other kernels were absent.

Sources and [historical measurements](HISTORICAL.md) are salvaged from PR #205,
exact commit `104ca5499318a49eb7f872af45fe837e10bdae57`. The Rust inputs are
byte-identical. The measurements used Verus `0.2026.10.04.426d8b0` and Z3
4.16.0; they do not establish behavior on the current pin. FPBench source
revision was `d7ade34e` (MIT). No corpus tier selects these reproductions.

Use the bounded raw-output runner from the mutation examples with an explicitly
chosen native install:

```sh
python3 corpus/repros/verus-mutation-evidence/repro.py VERUS_INSTALL \
  corpus/repros/z3-rlimit-platform-difference/*.rs --runs 1 --cap 120 \
  --out EVIDENCE_DIRECTORY --verus-arg=--rlimit --verus-arg=1
```

The output directory must be new. Each attempt retains original stdout,
stderr, exit, command, source hash and elapsed time. `timed-out` means no
completed verification outcome. Inspect original diagnostics for finished
runs; the runner supplies no solver or campaign classification. To reproduce
the historical default-limit experiment, omit the two `--verus-arg` options
and choose an explicit cap; preserve interrupted attempts too.

The original kernel collection is upstream
[fptaylor-extra.fpcore](https://github.com/FPBench/FPBench/blob/d7ade34e/benchmarks/fptaylor-extra.fpcore).
Its pinned [copyright and permission notice](LICENSE.FPBench.txt) is retained
beside this derived input. The external translator is not shipped or
independently validated by preserving its output.
