# Optional interactive Verus engine

The patch applies only to the pinned source commit and a separate Sail-managed
source variant. Stock releases and ordinary `sail verify` are unchanged.

A repair runs fresh Rust frontend, type, mode, trait and ghost lifetime checks,
then focused verification and an independent full verification in one active
compiler callback. Each phase lowers every bucket fresh, exactly as stock Verus
does, and creates fresh solver contexts. Full stdout and its diagnostics remain
authoritative only after deferred errors and the late EXEC compilation pass;
focused results and progress remain provisional.

The interactive driver catches only the pinned Rust compiler's expected
`FatalErrorMarker` unwind. Unexpected internal panics resume unwinding and cannot
reach a compiler seal; ordinary default verification retains its existing
catch-all behavior. A terminal `compiler-sealed` frame includes
`payload.normal_termination: true` only on a normal driver path, including
reported proof, VIR and late EXEC errors. This flag describes completion, not
success: the full JSON, exit status, selected scope and coherent input checks
still determine authority. Missing or malformed completion, internal panic,
signal, or malformed full output requires the stock fallback.

## What the patch contains

`interactive.patch` changes four files:

- `rust_verify/src/driver.rs`: the fatal-error handling above and the
  `compiler-started` and `compiler-sealed` observations.
- `rust_verify/src/verifier.rs`: the focused phase before the full phase in the
  same callback, the reporter it uses, and bounded observation hooks
  (diagnostics, obligation progress, functions observed).
- `rust_verify/src/sail_engine.rs` (new): the focused phase, its bounded report
  (`SAIL_ENGINE_FOCUS_REPORT`, holding only the selected function) and the
  bounded observation journal (`SAIL_ENGINE_EVENTS`).
- `vir/src/ast_util.rs`: a friendly-name helper that admits a name's bytes
  before allocating it.

`Cargo.lock` is the stock source lock: the patch adds no dependency.

### Removed: prepared-lowering reuse and the same-input oracle

Earlier patches cached prepared lowering (the SST and polymorphism output of
each bucket) within one invocation and, with an opt-in, across edits, and
checked every restored hit with a same-input cached-versus-fresh byte oracle.
Measured reuse was near zero (no in-batch hits on HumanEval; L1) while every
miss paid for key encoding and admission under one global lock, about 6.7 s
per repair on HumanEval and 40 s on Vest. The oracle also needed confirming
fresh arms and an unstable class to stop flaking on stock trigger
nondeterminism (VI-0015). Both were removed (rescue L7a; ADR 0038). Reuse
across edits is redesigned for M8 with content-addressed keys. The removed
patch, its qualification pipeline and its record are in this directory's git
history and the `archive/m7-contracts-f7103bf` tag.

## Validation

`test-engine.py` and `test-cancel.py` are local-only differential checks that
use an already built variant; CI never compiles Verus.

- `test-engine.py` compares the engine with stock on exit status, exact
  stderr, verification counters, per-function verdicts and full raw SMT
  sessions. Fixtures cover bitvectors, termination, failed focus, failed other
  functions, macro diagnostic spans, missing and ambiguous exact selectors,
  Rust frontend errors and late EXEC errors.
- `test-cancel.py` kills a worker at an actual blocked solver check, requires
  no terminal seal or unfinished focus report, and verifies the restart
  against stock.

```sh
python3 engine/verus/test-engine.py --engine BUILD/verus/source/target-verus/release/verus \
  --stock STOCK_VERUS --z3 Z3 --out OUT/source-fixtures
python3 engine/verus/test-cancel.py --engine BUILD/verus/source/target-verus/release/verus \
  --stock STOCK_VERUS --z3 Z3 --out OUT/cancel-restart
```

Also run the patched crates' library tests in the build checkout
(`cargo test --release -p rust_verify --lib`, and likewise `vir` and `air`).
Final performance acceptance uses the frozen repair catalogue and the complete
focused-plus-full loop ([product acceptance](../../docs/bench/m7-repair-acceptance.md)).
[qualification-linux.json](qualification-linux.json) records the qualification
of the runtime currently pinned; a new patch needs its own record.

## Corpus conformance arm

`sail-conformance engine` (run by `ci/run.sh conformance`) compares this
runtime's full phase with the stock release on the corpus, on verdicts and
diagnostics, and requires a normal, consistent compiler seal; see the
[repair guide](../../docs/proof-repair.md#engine-conformance).

`cargo xtask build-verus-source --interactive-engine` builds vstd the way the
release does (`cargo-verus build --manifest-path vstd/Cargo.toml` from
`source/vstd`, variant feature `vstd-build:cargo-verus-release`), so
diagnostics that point into vstd render as `seq.rs:980:12`, as stock does,
rather than `vstd/seq.rs:980:12` (L4b).

## Pinned Linux runtime for CI

`artifact-linux.json` binds the private release asset, archive digest, source
commit, patch, lockfile, Rust toolchain and solver. `ci/engine-artifact.py`
checks those pins and every runtime file before installation. The Docker image
uses a BuildKit secret for the private download; it does not retain that
credential or build Verus from source. The optional runtime is exercised by
the real repair CLI/MCP end-to-end phase alongside the stock checks. CI
downloads these exact prebuilt bytes; it never compiles the engine.

A changed patch needs a new runtime: build it with
`cargo xtask build-verus-source --interactive-engine`, pack it with
`ci/engine-artifact.py pack`, publish the archive as a private prerelease
asset and commit the new pin. Until then the committed pin is stale and the CI
image build refuses it.

This selection is on the in-progress M7 branch. The private prerelease
artifact is a runtime prerequisite, not M7 product acceptance.
