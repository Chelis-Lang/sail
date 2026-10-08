# VI-0006: The 0.2026.09.06 macOS release archive bundles no z3

- **Status:** open (tracked here; not filed upstream; see the README)
- **Verus:** `0.2026.09.06.8dea4a2` (secondary pin, ADR 0010 item 4)
- **Found:** 2026-10-08, while adding the secondary pin
- **Impact on sail:** without a bundled `z3`, the `verus` launcher finds no
  solver unless one is on `PATH` or named by `VERUS_Z3_PATH`, so plain Verus
  cannot run as unpacked.

## Description

`verus-0.2026.09.06.8dea4a2-arm64-macos.zip` (SHA-256 `23be2e81…`) has the
`verus`, `cargo-verus` and `rust_verify` binaries but no `z3`. The pinned
primary release (`0.2026.10.04.426d8b0`) bundles z3 4.16.0, and both
releases pin Z3 4.16.0 (`source/tools/get-z3.sh` at each commit). The Linux
archive has not been checked.

## Workaround

In `corpus/toolchain.toml`, the secondary pin's assets say `z3 = "primary"`:
the primary release's bundled, SHA-checked z3 is used. When a release
bundles no z3, the harness sets `VERUS_Z3_PATH` for plain Verus too, so both
sides of every comparison use the same binary (`sail_verus::run`).
