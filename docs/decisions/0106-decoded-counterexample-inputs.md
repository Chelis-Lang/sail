# 0106: Decode source-named inputs from failed-proof models

- **Status:** accepted; amended by [ADR 0109](0109-oyster-retired-v030-scope.md): with the neutral assurance inputs view removed in v0.3.0, decoded inputs are reported by the MCP `counterexample` tool and in attributed assurance rows
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
  unknown non-advisory obligation with a model, it keeps only constants Verus
  names `IDENT!`, which include parameters and the return binder. It renders
  scalars in ITF and constructor terms as records whose fields follow the
  constructor's argument order. It counts the omitted internal constants and never alters the raw
  model.
- Decoded inputs for neutral assurance rows that are not `Proved` are a
  separate version-one view (`counterexample::View`), not a field of the
  version-two assurance view. `assurance::decoded_inputs` computes it from the
  result document and caller-supplied field names. The service reads it through
  `Request::AssuranceInputs`, negotiated as `counterexample_version = 1`, and
  names struct fields from the retained original source; the CLI reads it with
  `sail assurance JOB --inputs`. Attributed ordinary-job rows carry the decoded
  inputs in their own view, with positional field names, because that view
  had no earlier records.
- Every decoded value is labelled as the solver's model for one check. Sail does
  not evaluate it against the source, so it is not a validated concrete
  counterexample. An unknown answer adds a qualifier that the model may be
  partial or spurious. Outcomes, tiers and establishment are unchanged.

## Consequences

Requirement-level failures show the source values the solver chose without
noise. Concrete refutation still needs the separate literal-witness validation.
The version-two assurance view is unchanged: a client that checks a served
view against `from_contract_result` on the same result document still finds
them equal, for proved and not-proved jobs and for views recorded before this
decision. An earlier revision of this decision placed decoded inputs inside
assurance rows, which broke that equality for not-proved jobs with a model; it
was corrected before any release. The decoded view and value types are likely to grow;
`#[non_exhaustive]` is a release-scope decision for the next minor release.
