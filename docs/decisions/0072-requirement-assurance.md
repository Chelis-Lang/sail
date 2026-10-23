# 0072: Derive requirement assurance from authenticated neutral evidence

## Decision

Add a versioned read-only requirement assurance view to the existing durable job
service. Reuse the same authenticated source-job result read before projecting
rows grouped by their original requirement IDs. Expose the view through the Rust
client, CLI and MCP. Capability negotiation fails closed for older services.

Move the existing conservative neutral proof projection into the backend-free
API crate so worker validation and assurance use the same interpretation of the
original attributed backend records. Recomputing this summary does not execute
SMT or trust caller-created documents as authenticated service output.

Establishment, evidence tier, currentness, relationship and mutation status are
independent. Confirmed SMT evidence requires a complete whole-job proof and
actual original solver identities. It describes implementation satisfaction of
the stated contract under its preconditions. Currentness of live caller source
is unknown. Requirement entailment, input feasibility, termination and mutation
assurance remain explicit qualifiers until separately established.

## Consequences

Clients can inspect retained evidence by stable requirement ID without deriving
an assurance tier from a job's process exit code or a matching label. Obligation
identities bind source, guarantee, function and backend context. Invalid hashes,
contradictory proof summaries and unsupported evidence versions are refused.
Unknown verifier outcomes remain unproved with no established tier.

The interface does not implement the complete M10 ledger or its mutation and
backend-selection work. Future evidence families must define their own
authenticated relationships and preserve these distinctions.
