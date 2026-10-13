# L21: remaining SMT output variation on the Verus fork

- **Lane:** L21, rescue. **Date:** 2026-10-13.
- **Question:** L18 found SMT digests still varying between runs of identical input on the fork: `human_eval_160` in 16 of 32 samples, plus Vest's `cbor::value` and `asn1::integer`. What causes the variation, and can the fork fix it under ADR 0039's rule?
- **Answer:** there is one source, VI-0001's `arrow_*` accessor order. The fork fix `9a5b2cc8` lives in `builtin_macros`. Under `cargo verus`, Cargo builds that crate from the project's dependency source, so the fix reached only direct `verus FILE` runs. Fork commit `1612b951` (branch `l21/determinism`) fixes it in the verifier. After the fix, no measured unit varies.

## Environment

- **Host:** AWS EC2 `c7i.8xlarge` (32 vCPU), us-east-1, Ubuntu 24.04.5, kernel 7.0.0-1014-aws. It was launched with `scripts/perf-host.sh up l21 c7i.8xlarge` and torn down after use. Everything ran natively on the host, with no container.
- **Toolchain:** Rust 1.98.1. Z3 4.16.0 from the pinned release archive (its sha256 matches `corpus/toolchain.toml`).
- **Builds:** made with `vargo build --release` from two fork trees:
  - **before:** `sail` at `d2b69aea`;
  - **after:** `l21/determinism` at `1612b951`, which is `d2b69aea` plus one commit.

  Both were run in stock mode, as `verus` and `cargo-verus verus verify`; the interactive engine's focus phase was not used.
- **Inputs:**
  - human-eval-verus at `bbe93f73`, Vest at `11af01dd`, and the verus examples and vstd at upstream `426d8b01`;
  - for the Cargo arm, Vest's `vstd` is patched to a path, and human-eval-verus is wrapped in a generated Cargo package, both as `sail-bench repair prepare` does.

## Cause

Cargo builds `verus_builtin_macros` from the source that the project's `vstd` dependency resolves to. That is crates.io for user projects, and the pinned upstream checkout for Sail's corpus and repair workspaces. Only direct `verus FILE` runs use the fork's bundled macro library. Of the fork's fix commits, only `9a5b2cc8` touches a crate that Cargo builds from the dependency source; the others are in `rust_verify` or `vir`.

Evidence, from the `sail` build with 10 runs of each unit:

| `vstd` and its macros resolve to | `human_eval_160` | Vest |
|---|---|---|
| the pinned upstream source | 2 distinct streams | `cbor::value` 4 streams, `asn1::integer` 2 (plus its two `i64_from_be_bytes` query files) |
| the fork's source (Vest: 8 runs) | 1 | all 298 files stable |

Every differing file differs only in the order of `arrow_<field>` items, and in what follows from that order:
- fuel declarations and fuel lists;
- `Function-Decl` and `Function-Specs`;
- `%%global_location_label%%` numbering.

The types involved are `Expr` (`human_eval_160`), `CborValue` and `CborValueSpec` (`cbor::value`), and `Integer` (`asn1::integer`). No other source of variation was found.

## Fix: fork `1612b951`, branch `l21/determinism`

**Commit.** "fix: declare enum arrow accessors in source order in the verifier" changes one function in `rust_verify/src/rust_to_vir_impl.rs` and adds one test file.

**What changes.** `translate_impl` now recognises the impl that `enum_synthesize` generates by its shape:
- it is an inherent `verus_macro` impl of a local enum;
- it has a run of `arrow_<field>` functions, one for each distinct field;
- exactly the enum's `arrow_<Variant>_<field>` functions follow, in declaration order.

It visits the `arrow_<field>` functions in the order their fields first appear in the enum. Any impl of a different shape keeps its written order.

**ADR 0039.** The chosen order is the one the fork's macro already emits, and one of the orders stock's macro emits. In the evidence:
- every after-fix SMT file, on human-eval-verus (114 files) and Vest (298), is byte-identical to the corresponding file from the before build with the fork's macros;
- each is also byte-identical to the file from at least one of the 10 before-fix runs with stock's macros;
- direct runs of all 147 fast-tier units are byte-identical with and without the fix.

**Regression test.** `sail_vi0001_impl_accessor_order.rs` has two tests:
- One writes the generated impl by hand, in one of stock's orders, and requires source order in the AIR log. It fails on `d2b69aea` and passes on `1612b951`.
- The other checks that a hand-written impl keeps its order. It passes on both.

**Other checks.** The commit message passes `scripts/check-ai-markers.sh`, and the changed Rust passes `rustfmt`. The fork's `sail` branch was not changed. Sail's engine pin and CI image inputs are unchanged.

## Tests on `1612b951`

**`rust_verify_test`, full suite:** 4,645 passed, 2 failed and 58 ignored.
- The 4,705 tests are upstream's 4,695 plus the fork's 10.
- The two failures are `cargo::cargo_new_builds` and `cargo::cargo_new_verifies`. They fail the same way on `d2b69aea`: `cargo verus new` reports "no known toolchain manifest for Verus version 0.2026.10.13.<sha>". A source build at an unreleased commit has no release manifest, so this comes from the environment and not from the fix.

**`sail_vi*`:** all 9 files and 10 tests pass, including both VI-0001 tests.

## Determinism: 10 runs per unit, before and after

| Units | How run | Before (`d2b69aea`) | After (`1612b951`) |
|---|---|---|---|
| Fast tier (145 verus examples, vstd, human-eval-verus) | `verus FILE --log smt`, 10 runs | 0 of 147 vary | 0 of 147 vary |
| human-eval-verus (114 SMT files) | `cargo-verus verus verify`, vstd from the upstream source, 10 runs | 1 varies (`human_eval_160`) | 0 |
| Vest `vest_lib` (298 SMT files) | same | 2 buckets vary (`cbor::value`, `asn1::integer`; 4 files) | 0 |

**What this does not cover:**
- the engine's focus phase;
- Sail's own `--verify-module` and `--verify-function` selections;
- the standard and deep tiers other than Vest;
- macOS.

Stock `sail verify` runs the stock release, so it still varies (VI-0001 is open in stock).

**Next steps for the lead:**
- Fold `1612b951` into `sail` among the fixes, before the engine commit; it does not depend on the engine.
- Rebuild, and re-pin together with L19.
- Rerun L18's changed-bucket count on the new runtime.

## Evidence

Raw files are in `~/sail-raw-evidence/l21/` on the lead's machine. `SHA256SUMS` covers every file.

| File | Contents | sha256 |
|---|---|---|
| `l21-out.tar.gz` | every run's output and SMT logs (`he-cargo-*`, `vest-cargo-*`, `direct-sail`, `direct-fix`, `he-sail`), determinism summaries (`direct-*.jsonl`), test logs (`tests/fix-all.log`, `tests/fix-sail-vi.log`, `tests/fix-vi0001.log`, `tests/sail-vi0001.log`, `tests/sail-cargo.log`) | `c2b34986bd6e2560e6f51cdc6ec48678d78a39f338b2a1215effcf48df0ae3cb` |
| `l21-scripts.tar.gz` | host setup, build, determinism (`cargo-det.py`, `direct-det.py`), comparison (`cross.py`, `cross-direct.py`, `show-diff.py`) and test scripts | `fd0e3f45b2bfadc86a2819949a49029ef2d05eb5a376223d5c1d03c34fe4cf41` |
| `fork-1612b951.patch` | the fork commit as `git format-patch` | `0aa818efb1379611995bb4d5fce6f0209019af31f6e9b42b127dc9e86a15ac39` |
