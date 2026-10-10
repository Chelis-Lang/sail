# Exact graph-read candidate

This candidate has compiled library checks and VIR unit tests, but no qualified runtime. It does not change
`interactive.patch`, the installer or any qualified runtime. See ADR 0028.

Apply `graph-ledger.patch` to a separate checkout of the primary pinned Verus
commit. Do not apply it on top of `interactive.patch`: the candidate is a complete
patch including that baseline. Before building, run:

```sh
python3 engine/verus/candidates/audit-graph-ledger.py --source VARIANT/source
```

After a separately authorized source build, run the seven ledger tests:

```sh
cargo test --release -p vir --lib sail_graph::tests
```

Use the variant's pinned source toolchain and normal source-build environment.
Then run the existing source tests and the local-only exact oracle:

```sh
python3 engine/verus/candidates/test-graph-ledger.py \
  --engine VARIANT/verus --stock STOCK/verus --z3 PINNED/z3 --out OUTPUT
```

The oracle uses the same input paths and compares original full diagnostics,
function outcomes and solver-input bytes without normalization. It retains
unchanged-callback, body-edit, stale-span, corrupt/misfiled/oversized checkpoint
checks. Cancellation and real-project catalogue gates remain separately
mandatory. Neither the scripts' presence nor static coverage is a runtime pass.
