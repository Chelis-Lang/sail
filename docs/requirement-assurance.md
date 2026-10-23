# Read requirement assurance

Use `sail assurance JOB --output json` to inspect the requirement IDs carried by
an existing neutral source job. The API is
`sail_api::jobs::Client::requirement_assurance`; MCP exposes
`requirement_assurance` with a required `job_id`. Reading a view does not submit
source or run a verifier. The service authenticates the retained original source,
worker output and public metadata before deriving the view.

The version-two view groups evidence by stable requirement ID, with
content-derived obligation identities and the exact source, guarantee, lowered
source and backend report hashes. Each row states its relationship, conservative
proof outcome, establishment, optional evidence tier, actual solver identities
and qualifiers. Collections are sorted. The complete original job result remains
available through the existing result operation.

A complete `Proved` job with original solver identity establishes
`contract-to-code`, `confirmed`, `smt-verified` evidence for the exact submitted
source snapshot under its preconditions. Incomplete evidence and missing tool
identity receive no established tier. `NotProved` includes verifier unknowns and
does not assert a concrete counterexample. One passing function cannot promote
an incomplete whole-job result.

Source currentness is `unknown`: the service cannot establish that the caller's
working files still match its immutable snapshot. Consumers must check the
current source hash and establish the separate contract-to-requirement entailment
and required-input feasibility relationships. A matching requirement ID alone
establishes none of these. Mutation status remains `not-established` unless an explicitly requested
[bounded mutation campaign](neutral-mutation.md) matches the exact baseline.
Its separate killed, survived and inconclusive counts never upgrade a proof
outcome or tier. Reading proof evidence never creates a campaign or termination
claim.

The client negotiates `assurance_version = 2`. Older endpoints fail explicitly
without a fallback or synthesized view. This read interface applies to admitted
[neutral source jobs](neutral-contract-jobs.md). It does not reinterpret ordinary
project jobs, generic baseline comparisons or historical temporal evidence.

The [decision](decisions/0072-requirement-assurance.md) records the trust boundary.
