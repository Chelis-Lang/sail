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

Decoded model inputs for the rows that are not `Proved` are a separate
version-one view, so the version-two assurance view stays exactly reproducible
from the job result with `sail_api::assurance::from_contract_result`, including
views recorded before decoding existed. Read it with
`sail assurance JOB --inputs --output json` or `Client::assurance_inputs`, after
`counterexample_version = 1` negotiation. Each row names the requirement ID,
obligation identity and function, and decodes the solver model of that
function's first failed or unknown check. Sail keeps parameters and the return
binder (constants Verus names `IDENT!`), renders scalars in ITF, decodes struct
values into records with fields named from the retained source, and reports how
many internal constants (fuel, type identities, labels) it omitted. Library
consumers compute the same view with `sail_api::assurance::decoded_inputs`,
supplying struct field names or accepting positional ones. The raw model stays
in the obligation's `counterexample` in the job result. The decoded values are
not evaluated against the source, so they are not a validated concrete
counterexample, and an unknown answer may give a partial or spurious model.
Decoded inputs never change an outcome, establishment or tier. Library
consumers can decode any project report with `sail_api::counterexample::decode`.
The [decision](decisions/0106-decoded-counterexample-inputs.md) records the
limits.

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
project jobs (see [ordinary Verus jobs](#ordinary-verus-jobs)), generic baseline comparisons or historical temporal evidence.

The [decision](decisions/0072-requirement-assurance.md) records the trust boundary.

## Ordinary Verus jobs

An ordinary `sail verify` job proves the submitted Verus source against its own
specifications. A submitter can attach requirement IDs to exact functions when
it submits the job:

```sh
sail verify --background --output json \
  --requirement REQ-1=demo::checksum --requirement REQ-2=demo::parse
sail assurance JOB --attributed --output json
```

Each `--requirement` names a requirement ID and the exact crate-qualified
function name the verifier reports; the last `=` separates them. Attribution
applies to stock project and standalone-file jobs submitted with
`--background`, not to engine or repair jobs. The API is
`sail_api::jobs::Client::submit_attributed` and
`Client::attributed_assurance`; MCP accepts a `requirements` array on
`start_verification` and `attributed: true` on `requirement_assurance`. The
service retains the canonical, sorted attributions with the durable job and
across restarts. Identical submissions coalesce only when their attributions
also match.

The version-one attributed view groups rows by requirement ID. Each row has the
`verus-specification-to-code` relationship and the `submitter` attribution
source. It never reports `contract-to-code` and is never neutral-contract
assurance. A row is `Proved` only when the whole job completed, the verifier
reported exactly one requested-target function with that exact name, and that
function proved. A `confirmed`, `smt-verified` tier additionally needs actual
original solver identities and inputs the service classifies as current. Every
row carries the neutral view's shared qualifiers plus qualifiers stating that
the link is an unchecked submitter claim, that no neutral contract was admitted
or checked, and that the Verus specifications and toolchain are trusted. An
unreported function, missing solver identity or non-current input adds its own
qualifier and establishes no tier. Mutation status is always `not-established`.
A row that is not `Proved` carries decoded model inputs as for neutral jobs,
with struct fields named by position.

The client negotiates `attribution_version = 1` and fails for older services.
Reading attributed evidence for a neutral job, or for a job submitted without
attributions, fails with `SAIL3160`. The
[decision](decisions/0104-ordinary-job-requirement-attribution.md) records the
trust boundary.
