# Experimental Kani backend

Status: decoder groundwork on a development branch; native decoder qualification,
executor, CLI, complete direct-Kani conformance and both-platform acceptance are
pending. Nothing is selected or advertised as an available Sail backend.
[ADR 0128](../decisions/0128-kani-bounded-observations.md) defines the boundary.

The library accepts only original Kani 0.68.0 / CBMC 6.11.0 single hand-written
harness observations. It distinguishes bounded success, a property counterexample
and insufficient unwind bounds, without reporting an ordinary verified result.
It creates no proof receipt, currentness or cache authority. The contributor-only
`inspect_export` example decodes supplied captures; it does not invoke Kani.
Default Verus execution and Z3 keys are untouched. Consumer skill instructions
remain unchanged because no CLI/MCP or released availability changes.

A fresh native EC2 c7i.4xlarge/us-east-2 installation uses the official pinned
installer and nightly-2026-08-21. Direct original arrayvec 0.7.8 pilot calls
produce exit 0 for `p2_try_push_full_is_err`, exit 1 with a native property failure
for `f1_push_never_panics`, and exit 1 with an unwind failure and undetermined
checks for `e1_retain_unwind_too_small`. The dependency lock stays exact. These
are direct-tool setup observations, not Sail decoder or backend acceptance.
Raw exports, logs and immutable installation identities remain outside the
repository; their acceptance archive and decoder results are pending.

| Attempt | Date | Result and limit |
|---|---|---|
| First direct wrapper | 2026-11-13 | `cargo kani --locked` refuses an unsupported argument before execution, all three exit 2. Preserved; rerun fetches with Cargo's supported `--locked`, runs Kani offline and checks the unchanged lock before/after |

| `e8d1d553`, 2026-11-13 | All three decoder unit tests pass natively; clippy then refuses the test empty-slice assertion style. No actual-export decoder run is claimed from this attempt; successor keeps the assertion and prints the slice on failure |
