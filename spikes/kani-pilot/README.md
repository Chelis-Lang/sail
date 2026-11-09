# Kani pilot spike (issue 135)

Prototype only; not part of the workspace and not product code.

Pins: Kani 0.68.0, CBMC 6.11.0, toolchain `nightly-2026-08-21`; crate
`arrayvec` 0.7.8 (crates.io checksum
`d3fb67a6e08acf24fdeccbac2cb6ac4305825bd1f117462e0e6f2f193345ad56`, upstream
commit `0cb664cf505844348538230479b0040b44f3faf1`), bound by `pilot/Cargo.lock`.

- `pilot/`: harnesses in a separate crate; the pinned crate's source is untouched.
- `patches/`: the seeded off-by-one and the in-crate contract experiment.
- `host-setup.sh WORK`: install the pinned Kani and lay out the three copies.
- `run-direct.sh`: direct `cargo kani`, one harness per process, with costs.
- `run-extras.sh`: output formats, batch cost, solvers, playback, contracts.
- `sail_kani_proto.py`, `run-proto.sh`: the thinnest keyed, cached sail path.
- `run-all.sh WORK`: everything above, in order, on a native host.

Run on a native host with `RUSTC_WRAPPER=` (the scripts set it).
