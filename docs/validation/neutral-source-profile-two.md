# Neutral source profile two qualification

## Current result

The profile-two implementation passes supplemental local Rust compilation,
focused source/runtime/protocol regressions, and strict workspace lint on the
Rust 1.99.0 toolchain. Native Linux backend acceptance and complete native
workspace, solver, E2E and conformance gates remain separate requirements.
These local results do not establish a source proof or native mutation kill.

The actual bounded order fixture uses genuine unit enum state, non-Copy flat
records, shared references, mutable scalar references, and conjunctive
requirements. Ordinary Rust executes its real submission, cancellation, price
bounds and counter updates. The mapping retains the independently identified
requirements; adding Copy or replacing the function bodies with a toy scalar
companion is not part of this qualification.

Retained profile-one manifests and legacy protocol JSON keep exact bytes on
round trip. The new client refuses profile two before submission when explicit
profile support is absent. Raw enum old captures and mutable enum references are refused; enum entry
state is captured through checked Boolean comparisons. Unsupported enum payloads, discriminants, derives,
nominal variants, mutable record references and file configuration remain
explicit admission failures.

## Supplemental evidence

| Original artifact | SHA-256 |
|---|---|
| Focused test log | `66c6dfdf621584440a581f2033c1d4b73f6f5201a42b190a9a6ad3ad53721ce6` |
| Strict workspace Clippy log | `564520d083e3408565de9e8cf91f2aba0e59b7726aded2beb7ecf22528e399ec` |
| Annotated order fixture | `9ad2317e0e82b6aa9981aa851a20ca7125f9c2a8c79810a26267a906ad2723e4` |

The focused commands cover `neutral_profile2`, `neutral_contracts`,
`neutral_mutation`, shared IR/runtime tests and API protocol/mutation tests.
The workspace lint command is `cargo clippy --locked --workspace --all-targets
-- -D warnings`. Native gates must use the frozen source that their records
identify; subsequent semantic changes require corresponding requalification.

## Unknown diagnosis

The original direct fork/stock reproduction continues to produce actual
postcondition failures with incomplete-quantifiers UNKNOWN and no SAT. The
native diagnostic archive SHA-256 is
`20d5bcb9d045f71879cfb3bd08d78ad5535f22a6fef4e730aa31654f74863c72`.
Its public-image runtime identifies fork `7d9d541` and stock `7d829946`;
it is distinct from the older locally retained runtime. The first diagnostic
harness attempt did not invoke the verifier because Docker stdin was absent;
it remains a setup failure, not acceptance evidence. The subsequent direct
attempt retained both original streams and literal solver transcripts.

`neutral_mutant_diagnostics` records bounded inline Boolean/scalar controls
with the same contracts across each body mutation. No default solver option,
seed, verifier policy or mutation classifier is changed. UNKNOWN remains
inconclusive even when Verus reports failure or returns a model. Conclusive
native kills remain unestablished until actual authenticated SAT rejection
passes the strict installed-service campaign.

## Inline-contract native controls

A native Linux diagnostic then executed the unchanged inline Boolean identity
and bounded scalar increment contracts with their valid bodies and each literal
replacement. Each source ran separately on the pinned fork and stock verifier.
All twelve observations returned the expected verifier frontend outcome:

| Exact source cases | Original result in each backend |
|---|---|
| Boolean and scalar baselines | Exit zero; one UNSAT; no UNKNOWN or SAT |
| Boolean false/true and scalar zero/one mutants | Exit one; one incomplete-quantifiers UNKNOWN and two UNSAT; no SAT |

Inlining the guarantee does not remove the observed unknown in these controls.
The diagnostic establishes neither its underlying cause nor a semantics-preserving
solver fix. No default options, assertion context or outcome classifications
changed. Native mutation kills remain unestablished, and the public strict
passing gate remains false for this evidence.

The complete diagnostic archive has SHA-256
`59f2d4237102c503e465688b88eacbc3a036a5be0ef85c4f825256319c0c719d`.
The independent controller rehashed all eighty indexed original files, including
sources, lowering manifests, verifier streams and literal solver logs. Its
verification receipt has SHA-256
`a7c70ab373daf23bc6fc055a206b3797911c41824608095810b980622f7631fe`.
These are bounded direct compiler/solver controls on the recorded runtime,
separate from profile-two source-service qualification and complete producer
acceptance. The first six-case harness attempt could not create its target mountpoint
inside a read-only workspace and returned Docker exit 125. It remains a setup
failure and does not authorize a pass.
