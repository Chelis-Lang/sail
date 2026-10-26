# Source-visible managed transport environment

This is the minimal real-verifier witness for
[#385](https://github.com/Chelis-Lang/sail/issues/385). The caller leaves
`SAIL_MANAGED_RUNTIME` absent. The source asserts that this variable is present.
Plain Cargo-Verus must reject it; managed transport must not change that source
input merely to collect evidence.

Copy this standalone fixture outside the repository. Use the qualified pinned
fork through `SAIL_CARGO_VERUS`, with its exact pinned Z3 selected by both
`VERUS_Z3_PATH` and `SAIL_Z3_PATH`. Use a fresh `SAIL_CACHE_DIR` and a fresh
`CARGO_TARGET_DIR` outside the copied source for the plain arm; keep the caller
environment unchanged between arms.

```sh
unset SAIL_MANAGED_RUNTIME
"$SAIL_CARGO_VERUS" verus verify --lib -- --output-json --time
sail verify --output json --lib
sail verify --output json --lib
```

The reproduced journal-recovery `4f46` candidate returned plain exit101 with an
assertion failure, followed by Sail exit0 with Current/complete/confirmed
verification and then Current/complete/reused verification. Both arms used the
same fork36d65c3f and the same Z3. This is a default verdict difference caused by
compiler environment injection, not a solver or translation disagreement. The
main920 source has the same injection/forwarding path; this record does not
claim a main-binary execution.

Source SHA-256 is
`1c259c0cbd2784985522786fcc6e7de2dafa64d20619ae94739ddada6e525967`.
The qualified Sail executable SHA-256 is
`3743b6a31aaf008db98a1095af04223379a032e8fca3b06b550d9cd5734dcb5e`.
The issue records all raw stream hashes and the observed target/function state.
The expected fixed behavior is the plain failure on both fresh and repeated
Sail requests, preserving original absence separately from a present empty
value. Do not repair this witness by excluding the variable from input keys or
by suppressing its failed proof.
