# 0077: Separate scalar literal-contract counterexamples from Verus mutation verdicts

## Decision

Add the explicitly requested `neutral-scalar-literal-contract-witness-v1`
evidence family. Keep the native mutation policy, strict SAT rejection rules,
default Verus source, solver options and proof tiers unchanged. A native
UNKNOWN remains Inconclusive even when this separate family finds a concrete
counterexample. The [original reproduction](../../corpus/repros/neutral-contract-unknown/)
retains the failing source and literal unknown reason. The returned model after
UNKNOWN is never treated as a SAT model.

Sail prepares an exact deterministic query for each admitted original
literal-policy candidate. The first slice supports free functions with owned
Boolean or fixed-width integer parameters and a scalar literal result. Admit
the complete source through the existing neutral lowerer first. Preserve every
byte outside the replaced body and all original requirement IDs, preconditions
and guarantees. Keep omitted candidates and every unsupported target explicit.
Predicate calls, references, nominal input/output values and multiple requirement
identities remain outside this witness slice.

Reuse the existing typed specification translation and its checked arithmetic
definedness. Encode bounded mathematical integers and Boolean values in a
separately named quantifier-free SMT query. Conjoin all original defined
preconditions, defined old captures and defined postcondition evaluation with
the negation of the original postcondition. This deliberately finds violations
only where the specification's scalar computations are defined. UNSAT does
not prove the original program, establish feasibility or rule out undefined
specification arithmetic. Unknown and incomplete runtime execution remain
explicit. Queries use the pinned existing raw SMT runtime and its deterministic
resource limits; there is no local solver fallback or default-query rewrite.

After SAT, a second exact transcript requests all scalar input assignments.
Regenerate the complete original-source plan on validation. Bind source,
candidate, unchanged guarantee, exact query bytes, requirement identities,
limits, solver identity and complete original response products. Decode only
Boolean or fixed-width integer values, then independently evaluate the
original typed preconditions, postcondition and eager old-capture definedness.
Reject false preconditions, out-of-range values, undefined arithmetic, satisfied
postconditions, missing/duplicate assignments, UNKNOWN and altered source or
response attribution. The independent evaluator walks the typed Rust
expressions; it does not interpret the emitted SMT expression.

Use the already locked `num-bigint` release directly in `sail-verus` for exact
signed and unsigned 128-bit intermediates. Host-width arithmetic would create
false refusals or overflow while validating source-definedness. Add the direct
`sail-api` to `sail-smtlib` pure-parser dependency so first-pass observations can
bind their check index to the sole final exact `check-sat` command. Reusing the
bounded shared parser avoids a second command counter or unverified metadata.
Update the layering table accordingly. The API gains no solver, runtime or
Verus dependency, and no default verifier algorithm changes.

Decode concrete scalar response terms structurally: a Boolean atom, a canonical
nonnegative SMT numeral, or a two-element list whose operator is `-` and whose
operand is a canonical numeral. A token such as `-3` inside `(-3)` is not unary
minus applied to a numeral. Keep the general output presentation decoder
unchanged; witness evidence has this stricter admission boundary.

The source owner exposes preparation and concrete validation through the
existing local job RPC. Consumers retain original plans, requests, responses
and validated assignments, and revalidate through Sail on reads. API-level
relationship checks alone do not independently certify source translation or
backend execution. Operational solver provenance and source-level concrete
counterexample validation are distinct claims.

## Consequences

This family can demonstrate that an actual bounded literal replacement violates
an unchanged scalar contract even while Verus's quantified query remains
UNKNOWN. It does not establish a native mutation kill, promote proof assurance,
demonstrate exhaustive mutation coverage or validate the intended requirements.
General program bodies and wider contract shapes need separate qualified
translations. Product guidance is in [the witness guide](../literal-witnesses.md).
