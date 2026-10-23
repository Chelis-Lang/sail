# Run bounded neutral-source mutation campaigns

Use the Rust job client to request a campaign over a completed neutral source
job with a complete original proof and actual solver identity:

```rust,ignore
let campaign = jobs.submit_mutation(sail_api::mutation::Submission {
    baseline_job_id,
    max_mutants: 8,
    wall_time_ms: 900_000,
})?;
let retained = jobs.mutation(&campaign.id)?;
// Explicit cancellation stops the campaign's owned source jobs.
let cancellation = jobs.cancel_mutation(&campaign.id)?;
```

The client negotiates `mutation_version = 1`. Campaigns share the existing
Sail service; callers provide no executable path, generated proof code, model
provider or verifier setting. The current API has no general mutation CLI or
MCP submission tool. `SAIL3144` reports campaign admission, identity or evidence
failures.

The `neutral-free-scalar-literals-v1` policy admits contracted free functions
returning a boolean or a fixed-width Rust integer. It replaces exactly one
complete executable body with `false`, `true`, `0` or `1` as appropriate. It
preserves all bytes outside that body, including requirement IDs, predicates,
preconditions and guarantees. Each emitted source is admitted again by the
real neutral lowerer. Bodies already equal to a replacement in parsed tokens
are skipped. Supported candidates omitted by the requested budget and
unsupported contracted targets remain explicit and prevent a passing gate.

A campaign admits at most eight actual jobs and at most 2,700,000 milliseconds
in aggregate, including source submission and queue time. Each actual job
retains the neutral worker's fixed 300-second deadline, 24-MiB stdout bound
and 8-MiB stderr bound. The service admits at most two active campaigns.
Cancellation and shutdown stop only campaign-owned jobs. Operational terminal
states distinguish completion, cancellation, timeout and interruption.

`killed` requires original fully attributed stable SAT verifier rejection under
the unchanged admitted contract, with exact original tool/configuration
identities and no unknown, missing or unrecorded relevant evidence. It does
not assert a concrete source counterexample. A failed target has `complete =
false`; complete failure evidence comes from its explicit Verus verification
totals, failed report status and exact attributed rejection, not the success
flag or enclosing Cargo exit code alone. `survived` requires a complete
whole-source proof of the actual mutant. Every other result is `inconclusive`,
including a NotProved report whose underlying answer is unknown. Passing
requires all selected candidates killed and no omitted or unsupported scope.
No campaign outcome changes a source proof verdict or establishes requirement
entailment, feasibility, termination or exhaustive correctness.

Every campaign read authenticates independently retained original source,
worker stdout/stderr, baseline links and actual job identities. It regenerates
the exact inventory and classifications. Completed evidence survives pruning
of the original source jobs. An interrupted controller does not resume after
restart; available evidence remains visible with an interrupted state.
The full original worker streams are exported in `baseline_original` and each
available observation's `original`. Never truncate these streams. Aggregate
encoded original exports are bounded to half the 32-MiB job frame; the complete
response must also fit that frame. Oversize export is an explicit read failure
and establishes no passing mutation assurance.

Consumers retain the complete original campaign receipt and its content hash.
`sail_api::mutation::validate_view` performs pure offline relationship and
classification checks, starting no service. It does not certify the producer
or repeat native source admission, complete inventory or backend execution.
Consumers retain this translation trust. To validate an assurance receipt,
first reconstruct ordinary assurance from the original source job, call
`attach_assurance` with the retained complete campaign, and compare the exact
stored view. Matching campaign IDs without retained member evidence establish
nothing. Raw backend timings, paths and campaign start time are operational
receipt metadata; keep them out of semantic source identities.

[Requirement assurance](requirement-assurance.md) version two attaches bounded
mutation counts only after a campaign was requested for that exact baseline.
Proof evidence without a campaign remains `not-established`. The tier and
proof outcome remain unchanged.

The separate installed-service acceptance runner is:

```sh
cargo run -p sail-cli --example check_mutation_jobs --   path/to/installed/sail path/to/fresh-evidence-directory
```

It requests actual strong-contract kills and weak-contract survivors, records
original streams, checks deadline and cancellation, restarts the service,
prunes original jobs and refuses changed retained diagnostics. Fixture tests
are classification and deterministic-source controls, not a claim that an
actual backend campaign passed. Native acceptance remains required before
integration qualification. [ADR 0074](decisions/0074-bounded-neutral-mutation.md)
records the bounded interpretation.
