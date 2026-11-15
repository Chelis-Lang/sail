# sail-kani

Development components for the experimental Kani backend selected by ADR 0109.
The strict decoder reads original Kani 0.68.0 / CBMC 6.11.0 exports and produces
bounded observations. The Linux tools authenticator checks original Kani and
Rust payloads; Stage creates a private copy with one exact Rust toolchain route.
The execution module and contributor inspect_run example invoke that original
driver offline with an explicit harness, unwind and process budget.

Native Linux component gates and bounded independent reviews pass. These
components remain unselected: source/configuration/dependency/build-script
closure is unauthenticated, and they create no currentness, reusable receipt,
proof/cache authority or ordinary verified verdict. Selected CLI/backend, full
arrayvec/direct comparison and Mac qualification remain open.

The original release pins and ci/kani-bundle.py / ci/kani-toolchain.py retain
their strict original-file boundaries. Rustup bookkeeping does not authenticate
a toolchain; ordinary Tools authentication refuses links. Only Stage admits its
fixed exact route. Prefix ownership/immutability and private parents remain
caller preconditions.

Contributor preparation can materialize both original payload trees with
`ci/kani-install.py` from the pinned bundle and six Rust component archives.
It copies authenticated payloads directly and publishes one private directory;
it executes no installer or verifier. The read-only validators retain their
existing contracts. This helper is not a selected consumer setup command or
backend. Native component execution against its new prefix passes the three
original harness cases; evidence closure remains pending. `ci/kani-fetch.py`
adds exact bounded original downloads for the same private layout (ADR 0155).

Execution clears the child environment, retains argv/environment/stdout/stderr/
exit/export, and refuses interrupted or contradictory attempts. Each capture is
limited to 16 MiB; the child budget is 1 ms–600 s. Filesystem authentication and
all generated target disk usage are outside that budget. Cleanup covers the
inherited process group; this is not a sandbox.

See ADR 0128/0137/0149/0152/0153/0154/0155 and
[original-driver qualification](../../docs/validation/kani-original-driver.md).
