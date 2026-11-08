# 0106: Decode source-named inputs from failed-proof models

- **Status:** accepted
- **Date:** 2026-11-08
- **Implements:** item 5 of [ADR 0103](0103-scoped-neutral-widening.md)

## Context

A failed or unknown obligation retains the solver's first model as a raw
`get-model` reply with its zero-arity assignments. For a small function that
is dozens of constants, almost all of them internal: fuel identities, type
identities, location labels and dynamic-type witnesses. The source inputs
appear only as Verus-encoded names such as `input!`, and struct values as
constructor terms such as `(krate!Row./Row false 3)`. Consumers had to rebuild
this decoding themselves.

## Decision

- Add `sail_api::counterexample` (version 1). `decode` takes a function report
  and the struct field names in declaration order. From the first failed or
  unknown non-advisory obligation with a model, it keeps only constants named
  `IDENT!`: parameters and the return binder. It renders scalars in ITF and
  constructor terms as records whose fields follow the constructor's argument
  order. It counts the omitted internal constants and never alters the raw
  model.
- Neutral assurance rows that are not `Proved` carry the decoded inputs as an
  optional `counterexample`. The service names struct fields from the
  retained original source. `from_contract_result` names them by position and
  `from_contract_result_named` accepts the names. Attributed ordinary-job rows
  carry the same field, with positional field names.
- Every decoded value is labelled as the solver's model for one check. Sail does
  not evaluate it against the source, so it is not a validated concrete
  counterexample. An unknown answer adds a qualifier that the model may be
  partial or spurious. Outcomes, tiers and establishment are unchanged.

## Consequences

Requirement-level failures show the source values the solver chose without
noise. Concrete refutation still needs the separate literal-witness validation.
Proved rows and existing views serialize unchanged because the field is absent
when empty. The decoded view and value types are likely to grow;
`#[non_exhaustive]` is a release-scope decision for the next minor release.
