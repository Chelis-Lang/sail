# Composed acceptance model

The composed default-mode model connects the actual component sources through
`proofs/acceptance/context.rs`. It is checked as one complete crate with all
component dependencies. This is a conditional model theorem; the production
Sail implementation is not refined to it yet.

## Admission and safety

`accept_current_complete` creates a private `Accepted` token only when sealed
input evidence is valid at the supplied modeled world/observation point, the
request has whole scope, the authority component accepts its completed evidence,
and every recorded solver stream matches a complete query execution.

`composed_model_safety` establishes exact observed input context, default mode,
unique complete requested coverage, completed invocation origins and exhaustive
closed child-stream ledgers. `composed_reply_has_exact_origin` connects every
reply to its exact solver/mode/raw prefix and admitted origin. The context uses
actual modeled input facts and an explicit selected solver identity, rather
than equal unvalidated digest tags.

Each verifier invocation records child-stream starts and closures independently
of parsed documents. Completion requires their ordered inventory to equal all
document-declared transcripts. The composition then matches every inventory
position to an actual complete query trace. An empty trace cannot replace a
recorded stream, and omitting a started child from both document/query lists
cannot complete the invocation. Genuine solver-free verification is allowed;
there is no invented one-query-per-function assumption.

The nonempty witnesses construct actual model evidence through checked factories:
fresh verification and cached-origin recovery followed by fresh solver output.
The latter permits different caller inputs only when the reused SMT binding is
identical. Query, input and authority witnesses separately cover noisy/partial
publication, corrupt or unpublished recovery, mutation/ABA, absent/unreadable
inputs, missing siblings, ambiguous targets, interrupted/malformed output and
independent fallback.

## Retained results and presentation

An old acceptance token proves a fact about its stored point. It does not
expire automatically or promise an unchanged future workspace. A new Current
presentation or repair projection must use `refresh_retained`, a strictly later
point and sealed input evidence whose actual read witnesses are all later than
the old point. It rechecks the same complete admission relation. Passing the
old evidence/point again cannot construct a new Current projection.

`historical_view` sets Current false. The only Current presentation factory
requires fresh revalidation. The model permits exact input restoration with new
read evidence; it does not restore an old mutation epoch or relabel old bytes
with a fresh stamp. Reports retain their observation point and promise nothing
about mutations after that point.

## Remaining concrete refinement

The model environment supplies trusted process observations and a complete
modeled input map. Production must still establish which real compiler inputs
and stream events populate that map/history, how every physical child stream
is observed, how executable identities bind actual launches, and how protected
cache history corresponds to modeled origin ledgers. The local origin model
excludes shared/bundled imports; signatures alone are not solver-truth evidence.

The source interpreter, filesystem/process interfaces, cryptographic use and
all runtime success surfaces remain explicit refinement obligations under the
[design](evidence-acceptance-core.md). No model predicate named complete supplies
those production facts. The [checker guide](acceptance-proof-checker.md) owns
commands, scope/integrity checks and the [record](../validation/evidence-acceptance-models.md)
owns platform qualification.
