# 0104: Submitter requirement attribution for ordinary Verus jobs

- **Status:** accepted
- **Date:** 2026-11-08
- **Implements:** item 4 of [ADR 0103](0103-scoped-neutral-widening.md)

## Context

The requirement assurance view ([ADR 0072](0072-requirement-assurance.md))
reads neutral source jobs only. Callers that verify ordinary Verus source, for
example functions that loop over buffers or call helpers across modules, have
no way to ask the job service which requirement a proved function serves. They
rebuild the link outside Sail from naming conventions, and the result loses the
service's conservative proof interpretation and solver identity.

Sail cannot check that an ordinary function's `ensures` states a requirement.
Only the neutral path relates a contract to requirement identities.

## Decision

Add an additive, separately negotiated interface (`attribution_version = 1`):

- `Request::SubmitAttributed` carries a stock or standalone `Submission` with a
  canonical, non-empty, sorted set of `(requirement_id, function)` pairs.
  The daemon validates and retains them in the durable job record (absent from
  unattributed records, so existing records replay byte-identically) and
  coalesces only identical attributions. Engine and repair jobs are refused.
- `Request::AttributedAssurance` projects the authenticated retained result
  with the retained attributions into a versioned view. It reuses the neutral
  view's run-completeness, exactly-one-proved and solver-identity rules.
- Each row has relationship `verus-specification-to-code` and attribution
  source `submitter`. It is `Proved` only for a completed whole job whose
  requested targets report exactly one function with that exact name, proved.
  `confirmed`/`smt-verified` additionally needs complete original solver
  identity and current inputs. Mutation status is `not-established`.
- Every row carries the neutral view's shared qualifiers and states that the
  link is an unchecked submitter claim, that no neutral contract was checked,
  and that the Verus specifications and toolchain are trusted.
- The CLI exposes `sail verify --background --requirement ID=FUNCTION` and
  `sail assurance JOB --attributed`; MCP exposes `requirements` on
  `start_verification` and `attributed` on `requirement_assurance`. Errors use
  `SAIL3160`.

## Consequences

Ordinary-route callers get per-requirement evidence with the same qualifiers
and conservative outcomes as neutral jobs, and the view can never be mistaken
for, or upgraded to, neutral contract-to-code assurance. Requirement
entailment from a Verus specification remains the caller's responsibility.
Older services fail explicitly. The view and request types are likely to grow;
marking them `#[non_exhaustive]` is a release-scope decision left to the next
minor release.
