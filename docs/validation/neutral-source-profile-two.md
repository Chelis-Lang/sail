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
| Focused test log | `67b209b07a2e2bb90fe3d527624b2dae9ce6d82c5e1f236d7600a55b527d6730` |
| Strict workspace Clippy log | `c9596b21806ccfa6d23c483738eb5575a665c9660cc92f96ff0013dcfb3c6bef` |
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

## Independent review and corrections

A fresh native review rejected candidate `4f53ac30`. Both backends failed the
original order fixture's submission and cancellation postconditions with two
UNKNOWN and no SAT checks. The reviewer also admitted a conditional non-Copy
record projection whose actual runtime macro failed Rust ownership checking,
although its ghost shadow verified. The rejected evidence remains archived as
SHA-256 `546b0ce30701249b33501a3fe8bc690a6b5586b36619f48f70c29dd40e00a594`.
Neither failure is relabeled as acceptance.

The original AIR shows executable enum inequality through an opaque PartialEq
postcondition while the guarantee uses structural variant inequality. An
isolated experiment changed only the generated enum's derive list to include
Structural. Both actual fork and stock then exited zero, each verifying all
four original functions with four UNSAT and no UNKNOWN or SAT. The experiment
archive has SHA-256
`f75f230e1b455f47f2d320e08a86f9260481aba6c33f9cad98c96ec169efa9ac`;
its corrected shadow has SHA-256
`27617ea26ccd5430d7127225efde44b3e20dce12003053a07a0a39c41d90591b`.
This isolated experiment motivates the typed emitter correction; it does not
replace the frozen implementation's native oracle or installed source-service
qualification.

The emitter now adds the Structural marker only to enums already admitted with
exact compiler-derived equality. The original application source and guarantees
are preserved. The ownership correction explicitly refuses nominal conditional
and block specification results and owning nominal input parameters. It keeps
direct non-Copy flat-field checks, shared references and scalar conditionals.
Regressions retain the reviewer's failing shape and its block/enum variants,
as well as move-consuming enum/record inputs. Legacy profile-one lowering
retains its original emitted bytes and interpretation.

The native oracle also includes the actual consumer-emitted order source,
SHA-256 `04c4578ba8b9f6f2ac778745ee642408d56b555b1ed10cff5e1c8b88d3afbafe`,
alongside the unchanged preparatory fixture. Its exact generated formulas and
original non-Copy types remain in the tracked input. Supplemental compilation
and runtime checks pass; native execution on the frozen corrected producer
remains required before acceptance.
