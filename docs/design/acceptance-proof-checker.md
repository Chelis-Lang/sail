# Direct stock-Verus acceptance model checks

The initial acceptance components are standalone proof models. Their source
does not replace Sail's runtime predicates or establish the composed service
theorem. The [design](evidence-acceptance-core.md) defines the remaining
source-sharing, adapter, cache-origin and observation-refinement obligations.

| Component | Source | Proved boundary | Production refinement |
|---|---|---|---|
| [Query association](acceptance-query.md) | `proofs/acceptance/query_association.rs` | Inductive raw-prefix binding, ordered pairing, fallback and replay suppression | Parser, shim, writer, persistence and recovery remain unproved |
| [Input binding](acceptance-inputs.md) | `proofs/acceptance/input_binding.rs` | Exact-byte comparison, scoped projection and guarded observation point | Compiler-input closure and Linux/macOS observation adapters remain unproved |
| [Scope and result authority](acceptance-authority.md) | `proofs/acceptance/authority.rs` | Executable unique target coverage and validated scoped/whole acceptance | Production report, receipt, repair, job and presentation adapters remain unproved |

These models cover arbitrary finite traces or vector lengths within their
stated relations. They are not finite corpus checks. Their assumptions still
include stock Verus/vstd, Z3, Rust/build semantics and the interfaces described
in each component guide. Cache initial-state validity, admitted writer truth,
complete effective-input observation and the trusted verifier's completion
remain explicit boundaries; this work does not independently check SMT
certificates or establish those interfaces by naming them.

## Run the complete checker

Use Python 3.11 or newer and the upstream release in `corpus/toolchain.toml`.
The selected stock pin is a compatibility baseline for these models, not a
change to Sail's runtime or Verus fork. Install its minimal Rust toolchain and
retain the original archive whose digest the manifest pins. The checker needs
both the archive and extracted runtime; an executable path alone is inadequate.

```sh
python3 scripts/check-acceptance-proofs.py \
  --stock-archive "$SAIL_STOCK_ARCHIVE" \
  --stock-dir "$SAIL_STOCK_RUNTIME" \
  --out "$SAIL_PROOF_OUTPUT"
```

`SAIL_PROOF_OUTPUT` names a new directory outside the checkout. Commit the
proof inputs before running: the checker reads the selected commit's Git
blobs, hashes those exact bytes and compiles copies of those bytes. Current
working files must agree with that commit. It also binds the proof manifest,
checker and toolchain pin, and records a digest for every mutated candidate.
An edit-and-revert between path observations cannot change the compiled proof
while retaining the recorded source digest.

The checker validates the archive digest and every runtime file against that
archive, including the launcher, verifier, vstd and explicit pinned Z3. It
checks the Rust compiler commit and sanitizes inherited Sail, solver, injected
compiler arguments, wrappers and loader settings. It directly invokes stock
Verus with `--no-cheating --output-json --num-threads 1`. No Sail binary, fork,
managed receipt or cache establishes the proof.

The inventory requires all three component IDs and all `.rs` roots under
`proofs/acceptance/`. Every positive result must terminate successfully, report
the pinned stock commit, whole-crate scope, exact verified count, zero errors
and the complete expected function inventory. Timeouts and incomplete or
duplicate-member JSON fail the check. No focus, skip, no-verify or extra
verification-argument option is accepted by the runner.

The initial source subset is deliberately standalone. It conservatively rejects
the tokens `mod`, `include`, `include_str`, `include_bytes`, `extern` and
`macro_rules`, even inside comments or strings. This excludes untracked helper
files and macro-based source loading; arbitrary Rust source closure is not
claimed to be captured by a filename inventory. Supporting those constructs
requires a reviewed closure-capture mechanism first.

Every component has two deliberate mutations of real model checks. A negative
control must terminate with ordinary verification failure, preserve the full
function inventory and account for all expected obligations. A crash, signal,
syntax/VIR error or unexpectedly successful proof does not pass a control.
Checker tests also exercise omitted components, altered verifier/solver bytes,
inconsistent evidence and injected environment settings.

`error[SAIL9002]` is a refusal, never proof evidence. Correct the identified
input, identity or verification failure and rerun the entire checker.

## Evidence and limits

The runner writes a concise `report.md`, machine-readable raw receipt and
stdout/stderr for every invocation outside the repository. The receipt binds
the commit, source bytes, expected inventory, runtime files, toolchain and
actual invocations. An owning qualification record must distinguish macOS and
native Linux results, failed attempts and measured scope.

Runtime files and the executing checker/toolchain must remain immutable during
checking. Pre/post hashing detects many changes but cannot prove atomic launch
identity against an actor replacing an executable and restoring it between
observations. Trusted OS, launch interfaces and toolchain behavior remain
assumptions. The Python checker is reviewed and tested ordinary code, not a
formally verified checker of its own implementation.

The workspace gate runs the checker failure tests. Actual stock proof runs are
local-only in this initial slice; there is no hosted proof-check job yet.
Their measured native duration and runtime packaging must be reviewed before
adding a fast-tier proof job. Existing Cargo/solver/E2E/conformance gates still
govern production changes, and runtime integration requires its own conformance
and native cost qualification. No product performance improvement is claimed.
