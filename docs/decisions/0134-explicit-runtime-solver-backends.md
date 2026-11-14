# 0134: Keep raw SMT backends explicit and their dialects separate

- **Status:** implemented development candidate; Linux gates, corpus and installer review pass; Mac pending
- **Date:** 2026-11-13
- **Tracking:** #712 package G, cvc5 spike

## Context

The raw SMT runtime currently binds Z3 identity, argv, option admission and reply
dialect directly. The retained cvc5 spike demonstrates why these policies cannot
be inferred from a shared solver name: resource units, option aliases and
reason-unknown products differ. This is separate from Verus verification.

## Decision

An internal Backend trait supplies exact solver identity, version checking,
argv, fixed zero-seed/cache policy, closed option admission and response dialect.
Z3 keeps its existing argv, option table, key bytes and response rules. cvc5
1.4.2 is separately archive/executable-pinned for Linux x86-64; no Mac pin or
readiness is invented. Each query still names one backend. Solvers never race
for a preferred answer and their cache identities cannot collide. Query bytes
are forwarded exactly, without normalization. Unknown and resource exhaustion
remain undecided observations and are not cached as decided answers.

Cvc5 uses --lang=smt2, -q, --incremental, fixed zero seeds and --rlimit-per, whose
units remain backend-specific. The closed dialect accepts its original bare
reason-unknown product. All existing process/output/cancellation bounds and
strict malformed-response checks remain. No proof tier or ordinary Verus
serving path is added or changed. No crate dependency is added.

The embedding config may explicitly name cvc5; otherwise its capability is
omitted. The CLI development opt-in SAIL_RUNTIME_CVC5 supplies this config. It
has no default installer or consumer recipe and remains unadvertised pending
Mac qualification. The CI image supplies SAIL_TEST_CVC5 only, so native solver
tests exercise the pinned binary without activating user-facing readiness.
The fixture installer preserves bundled licenses and validates both digests
before publishing its curated directory. Its source participates in image tags.

Primary references: [release1.4.2](https://github.com/cvc5/cvc5/releases/tag/cvc5-1.4.2),
[official options](https://cvc5.github.io/docs/cvc5-1.4.2/options.html). The GitHub
asset digest matches the retained spike pin; local archive verification agrees.

## Consequences

Native per-solver direct-output/model/core/incremental/resource controls, cache
separation/corruption and process-fault checks, complete local/PR gates, fresh
independent review, affected conformance and Mac qualification remain required.
Unit/key equivalence alone does not establish real-project acceptance or a
performance gain. Default capabilities and Z3 behavior must remain exact.
