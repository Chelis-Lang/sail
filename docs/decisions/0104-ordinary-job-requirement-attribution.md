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
  identity, current inputs, Verus's `--no-cheating` among the job's Verus
  arguments, and no Verus argument outside a reviewed set of selection,
  resource-limit, threading and report-detail options. The service applies
  live currentness before projecting, so a non-current job reads `NotProved`.
  The view records the job's arguments verbatim. Mutation status is
  `not-established`.
- Every row carries the neutral view's shared qualifiers and states that the
  link is an unchecked submitter claim, that no neutral contract was checked,
  that the Verus specifications and toolchain are trusted, and whether the job
  passed `--no-cheating`.
- The CLI exposes `sail verify --background --requirement ID=FUNCTION` and
  `sail assurance JOB --attributed`; MCP exposes `requirements` on
  `start_verification` and `attributed` on `requirement_assurance`. Errors use
  `SAIL3160`.

## Consequences

Verus reports a function that proves only through `assume(false)`, `admit`
or an `external_body` item as verified, which is correct for its default
mode, and Sail's verdict equality preserves that. The view cannot see such
assumptions, so without `--no-cheating` a `Proved` row establishes no tier and
says why. A neutral job avoids the question because Sail writes the verified
source itself. Weakening Verus options such as `--no-lifetime` also withhold
the tier.

Ordinary-route callers get per-requirement evidence with the same qualifiers
and conservative outcomes as neutral jobs, and the view can never be mistaken
for, or upgraded to, neutral contract-to-code assurance. Requirement
entailment from a Verus specification remains the caller's responsibility.
Older services fail explicitly. The view and request types are likely to grow;
marking them `#[non_exhaustive]` is a release-scope decision left to the next
minor release.
