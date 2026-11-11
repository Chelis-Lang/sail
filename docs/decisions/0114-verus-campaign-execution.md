# 0114: Owned Verus mutation campaigns and their evidence

- **Status:** accepted for implementation; product qualification pending
- **Date:** 2026-11-11
- **Evidence policy:** owner-approved [ADR 0111](0111-mutation-evidence.md)

## Context

The exec-only generator is implemented. General campaigns need explicit source,
function, runtime, solver and resource identities; bounded owned copies;
independent full-unit confirmation; and one view shared by CLI, JSON and MCP.
The removed neutral mutation endpoints and retained records cannot establish
these identities. Ordinary verification and its proof tiers must not acquire
campaign interpretations.

## Decision

Use `sail_api::campaign`, versioned independently from ordinary output, for
plans, original observations, classifications and reporting. Exec classes have
a closed vocabulary with no spec/proof/ghost operator. Runtime and solver
identities are separate SHA-256 bindings. Functions use an exact qualified
producer identity and Cargo target identity; ambiguous friendly names remain
inconclusive. Diagnostic messages retain explicit producer attribution and
original rendered diagnostics, without line-window inference.

The pure classifier implements ADR 0111. Baseline twins require at least two
complete stable observations and complete per-check headroom evidence; the
half-limit comparison is overflow-safe. An authoritatively empty check
inventory differs from an unrecorded inventory. Baseline failures in other
functions stay visible and do not manufacture a new mutation failure. Every
repetition must satisfy the final outcome's conditions. Solver answers,
reason-unknown strings and native models remain original observations. Models
are not independently validated merely because a detection has native `sat`.

Every final rejection and survivor uses repeated independent unscoped full-unit
runs. Scoped survival can be shown only as `survived-scoped`, explicitly pending
confirmation. Scope-specific diagnostics come from the actual producing run.
The score always carries rejected/survived counts, the native-sat versus
verifier-rejected breakdown, named inconclusive counts, unviable/unsupported,
omitted and cancelled work. No denominator means no score. An incomplete
campaign never appears complete.

The daemon owns a separate durable campaign namespace and an explicitly bounded
copy pool: one baseline/confirmation copy and independently warmed mutant
copies. Workers edit only owned copies; input revisions and exact patches are
checked before applying and after verification. Preserve all attempts and
partial work on cancellation, budget exhaustion or source changes. Restarted
in-flight work is interrupted evidence, never a completed campaign.

CLI `sail mutate plan|run|status|survivors|show|recheck|cancel`, JSON and MCP ship
together over the same view. Negotiate new campaign capabilities explicitly;
never reuse removed `SubmitMutation`/neutral wire records. Keep ordinary job
and verification documents unchanged. Unsupported source shapes, recursive
SCCs, `when_used_as_spec`, constant initializers and ambiguous target mappings
are explicit exclusions. Deduplicate identical mutant bytes and preserve the
exact inventory before bounded scheduling.

Initial hard bounds are 10,000 mutants, eight mutant copies, 2–20 repetitions
and 24 hours including setup and confirmations. Defaults are 100 mutants, two
copies, two repetitions and one hour. Requests exceeding bounds are refused,
not silently truncated. Lower user budgets retain omitted/cancelled counts.
The execution limits do not change the original verification limits.

## Consequences and gates

No new crate dependency or layering edge is needed for the evidence model.
Its addition to the API crate root has a separately reviewed exact factory
assembly binding; that review proves no campaign classifier or service theorem.

Implementation proceeds model/classifier, source admission and owned-copy
runner, durable daemon execution, then CLI/JSON/MCP together. Delivery requires
real Vest, IronKV, CapybaraKV and HumanEval controls, native Linux/Mac campaign
costs, baseline/weak-spec/rejected/resource/unsupported/cancellation/stale-source
cases, independent review and the applicable repository gates. Useful requirement
grouping is decided only after a real hand-joined IronKV or Vest example.
None of these gates is claimed by accepting this design.
