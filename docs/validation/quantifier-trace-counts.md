# Bounded quantifier trace counter

Status: bounded diagnostic component passes native Linux full gate and fresh
independent review. No complete profiler or public command availability is
claimed. Tracks #712 B/ADR0144.

Fresh fixture production runs only on native c7i.4xlarge/us-east-2 with pinned
Z3 4.16.0, binarye583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765.
Three calls complete exit0/empty stderr: two-check, quoted-qid and MBQI. Z3's
own statistics report8/8/1quantifier instances. Their minimal input/trace bytes
are regression fixtures; complete original streams/solver bindings stay outside
the repository. This capture alone does not qualify the new counter or prove
that tracing preserves a plain run's answers.

The component requires complete framing and bounded metadata, explicitly records
missing/colliding fingerprint ownership, supports MBQI and follows term-ID reuse.
Whole-prefix replay selection remains unchanged. The actual native and independent controls below pass. All required hosted
checks remain before merge. No default
runtime path, performance, real-project profile, source join, Mac or release
acceptance follows from fixture collection.

Documentation: ADR0144, this record, crate README, errors and changelog. No new
commands or release availability change; consumer skill commands remain unchanged.

## Retained attempts

| Source / attempt | Actual result |
|---|---|
| a3db6926, native1 | Clippy refuses an uninlined format argument in the fingerprint-bound test; unit tests were not reached. Successor corrects the expression without suppressing the lint. |

| ebb22c06, native2 | Clippy passes; 1 of 6 focused tests passes. Five tests expose using the SMT lexer on trace-only `#id` tokens. Successor uses a dedicated bounded trace cursor; original byte fixtures and expected counts remain unchanged. Native theory records also require the documented optional explanation and null-pointer forms. |

| 08ea65a4, native3 / review | Clippy and all6focused tests pass. Independent actual native datatype trace exposes decimal-null theory matches registered under numeric hash zero: one explicit theory instance becomes unattributed and can conflict with a quantifier hash0. Successor distinguishes the emitter forms and preserves an independent regression. The full local gate is interrupted after this rejection; completed scope and original exit remain separately retained. |

## Qualified exact component

Exact2c46889ee67ddb6163b318c538aabdc8e63b562d runs on native c7i.4xlarge,
us-east-2, with the ordinary pinned runtime/toolchains unchanged. Current-main
rebasebb4f3b76 preserves all crate, dependency, CI and runtime-pin bytes. Native
Clippy and eight focused tests pass, including the independent actual datatype
trace and the separate numeric-zero quantifier collision control.

| Gate | Actual result |
|---|---|
| Complete workspace | 1,431 passed /58 declared skips; full gate passes |
| Solver | 54 passed |
| Local E2E | Complete run, including Extra, managed cases and repair |
| PR conformance | Four arms each33 Same /311 functions, zero different/unstable/infra/skips;7resource-drift units retained |

The complete runner exits0. Every875original payload is independently lead-
rehashed and externally retained, including source bundle, actual runner/quick-
check logs/exits, tool/binary bindings, E2E evidence and corpus reports. Archive:
`c58f1b96beaff46357b6db641631c2d33777f8045860088fa9f98407e5edacd6`;
index `f1a01b30ae5dc21e84970a350ce5466ea3719d893314640208dac916938d753a`.

Fresh independent review approves the exact component after eight actual tests,
independent malformed/ownership/chunk probes, exact boundaries for all seven
resource limits, five fresh actual pinned-Z3 plain/traced pairs and nine original/
fresh trace comparisons. Every pair retains ordered answers, stderr and exits;
numeric timing/resource statistics remain captured, not assumed equal. Actual
quantifier/theory/check counts agree with an independent physical-event oracle
and Z3's quantifier-instance statistics. No universal equivalence is claimed.
All117indexed review originals and the exact source archive are independently
lead-rehashed and externally retained, archive:
`02a60f49ae8a6f44787d407f99a379784e6d1b6aecf5e451a856a2b40a1e09cc`.

This is an ownership counter, not complete producer-grammar validation. Opaque
record bodies receive minimal validation and binder cardinality is not checked;
synthetic impossible opaque/discovery records can therefore produce completed
counts. `attribution_complete` concerns only observed ownership, never trace
semantic validity, authentication, source identity, currentness or proof. A future
caller must bind the actual solver/session and compare traced/plain executions.
Original raw trace bytes, including native trailing spaces, stay unchanged.

ADR0087 classification: unselected diagnostic library component; no selected
verification, solver interaction, cache or oracle behavior changes. No new
dependency edge or proved-body/checker change. Standard/deep corpus, Mac,
performance and real-project/full-profile acceptance are not claimed or inferred.
