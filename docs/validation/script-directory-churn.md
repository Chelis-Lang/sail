# Build scripts that churn their own directory

## Status

Implemented on branch `fix/capybarakv-rebuild` (#294). Measured on a native
EC2 c7i.8xlarge in us-east-1; gates below. Not yet merged: it changes when a
run is Current (ADR 0046 amendment) and which script contracts a compiler
family shares (ADR 0036 amendment), which needs the owner's review.

## Defect

The 2026-10-20 Linux release matrix (main `006ee5d0`, EC2 c7i.8xlarge) found
CapybaraKV's reruns slower than stock: unchanged 36.45 s against 25.20 s
(0.69×), module edit 37.69 s against 25.12 s, spec-wide 36.08 s against
25.17 s. Every Sail rerun recompiled about 20 crates, verified vstd again
(2,059 functions) and read Historical with the note "Build-script input
declarations were established during this run". Reproduced on current main
`e45f9031`: noop 35.5 s, module edit 37.3 s, stock 24.6–25.0 s.

## Root cause

- **The churn.** rustix 0.38's build script (`can_compile`) runs
  `rustc -o -` in rustix's registry directory. rustc creates and removes a
  temporary `rmeta*` directory there (an inotify watch shows one `CREATE` and
  one `DELETE` per run). No content changes, but the directory's times do,
  so every run that ran the script was Historical.
- **The loop.** A Historical run withholds the compiler family's
  build-script export. The next run therefore treated every script as
  unobserved and cleaned all of them (`cargo clean -p` of proc-macro2,
  quote, libc, clang-sys, bindgen and the rest), so rustix's script ran
  again.
- **The refusal.** rustix also declares `CARGO_CFG_*` and `CARGO_FEATURE_*`.
  Family sharing refused those names, so even an exported generation never
  covered rustix: it, which, bindgen and capybarakv were cleaned on every
  run.

capybarakv's `rust-toolchain.toml` and bindgen/clang-sys's own scripts
played no part. The e2e `scripts` phase now reproduces the defect with a
path dependency whose script creates and removes a directory where it runs
and declares a cfg and a feature variable (`ci/test-managed.py`).

## Results

SAILAB_TABLE

## Gates

SAILAB_GATES

## Evidence

SAILAB_EVIDENCE
