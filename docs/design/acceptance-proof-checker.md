# Direct stock-Verus acceptance model checks

Four proof components share exact input, scope, mode, runtime and solver context.
They prove a conditional model theorem. Selected production operations now
share checked source through [concrete refinements](acceptance-production-refinements.md);
complete native observation and service composition remain unproved. The [design](evidence-acceptance-core.md) defines those remaining
refinement obligations, and the [qualification record](../validation/evidence-acceptance-models.md)
records the actual source and platform results.

| Component | Source | Checked model boundary |
|---|---|---|
| [Query association](acceptance-query.md) | `proofs/acceptance/query_association.rs` | Ordered exact-prefix association, trusted observation ledger, publication, crash/recovery and suppression |
| [Input binding](acceptance-inputs.md) | `proofs/acceptance/input_binding.rs` | Complete capture, read/change/projection/seal transitions, original guards and fresh retained-read evidence |
| [Result authority](acceptance-authority.md) | `proofs/acceptance/authority.rs` | Invocation lifecycle, independent child-stream ledger, checked records, exact coverage, fallback and repair |
| [Composition](acceptance-composition.md) | `proofs/acceptance/composition.rs` | Shared actual facts, complete trace inventories and fresh current presentation |

`proofs/acceptance/context.rs` defines the shared exact map and selected solver
identity. A digest or opaque input ID does not replace that map. The query
component permits cross-caller cache reuse only for identical solver/mode/SMT
bindings. The composed relation admits default mode; opt-in modes need their
own acceptance relation.

## Run the checker

Use Python 3.11 or newer and the upstream stock release pinned by
`corpus/toolchain.toml`. Keep the validated original archive and its extracted
runtime, including the compatible Rust toolchain and explicit pinned Z3:

```sh
python3 scripts/check-acceptance-proofs.py \
  --stock-archive "$SAIL_STOCK_ARCHIVE" \
  --stock-dir "$SAIL_STOCK_RUNTIME" \
  --out "$SAIL_PROOF_OUTPUT"
```

The output must name a new directory outside the checkout. Commit the inputs
first. The checker reads canonical Git blobs with replace refs and inherited
Git redirection/configuration disabled, then hashes and compiles those captured
bytes. It verifies exact equality with the working files before checking and
checks for later changes. Transient path edits cannot substitute different
compiled bytes for the recorded source. Each candidate and generated driver
receives its own digest.

Manifest schema `sail-acceptance-proofs-v2` requires all four component IDs,
all five source files and each fixed dependency closure. The checker generates
only the approved relative module declarations and writes every dependency
from its captured Git blob into a private temporary directory. Individual roots
include the shared context; the composition includes every actual component.
It cannot silently substitute copies or omit a dependency.

Source and mutations conservatively reject `mod`, `include`, `include_str`,
`include_bytes`, `extern`, `macro_rules`, `env` and `option_env` tokens, even in
comments/strings. Controlled module declarations exist only in the generated
driver. Arbitrary imports or compile-time environment-dependent source need a
reviewed closure mechanism before support.

Archive and all extracted runtime files are checked, including the launcher,
verifier, vstd and Z3. Rust compiler identity is checked. Inherited Sail, solver,
compiler-argument, wrapper and loader injection settings are cleared. Stock is
invoked directly with `--no-cheating --output-json --num-threads 1`; Sail, its
fork, managed receipts and caches do not establish these proofs.

Every positive result requires normal exit zero, the exact stock pin,
whole-crate verification, expected integer counts, zero errors and the complete
function inventory. Every deliberate guard-removal control requires normal
exit one, ordinary verification errors, unchanged complete inventory and full
obligation accounting. Signals, malformed/duplicate JSON, syntax/VIR errors,
resource exhaustion and proof timeouts fail the gate. Omitted components or
selected/focused verification cannot pass.

## CI and evidence

`ci/local.sh proofs` runs the same direct checks in the core CI image. The image
retains the pinned archive so the proof step needs no network or private source
credential. It is a local tier, not a CI job (AGENTS.md, Testing tiers); run it
before merging changes to the proved models or the checker. The workspace gate
runs the checker's failure tests in CI.
Failed-run artifacts replace previous task-owned outputs rather than retaining
an old success report. Native duration qualification belongs in the record.

The runner writes `report.md`, a raw receipt and per-case stdout/stderr outside
the checkout; the CI wrapper copies these into its artifact directory after
checking. Receipts bind source, driver, dependency, runtime hashes, Rust compiler identity
and actual invocations. Raw receipts are evidence, not independent certificates
of Python checker correctness.

The checker, toolchain, runtime and OS must remain trustworthy and immutable
during checking. Pre/post hashes do not prove atomic launch identity against
transient executable replacement. The Python checker remains reviewed/tested
ordinary code. Complete production parser/writer/recovery, actual compiler-input closure,
filesystem/process observations and CLI/JSON/MCP adapters still need refinement.
No service-wide correctness or product performance result follows from these
model proofs alone.
