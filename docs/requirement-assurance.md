# Requirement attribution retirement

The v0.3.0 development candidate retires ordinary requirement attribution,
`sail assurance` and the MCP `requirement_assurance` tool. Its stated evidence
bar was not met: a real five-function IronKV/Vest hand join preserves the same
per-function observations, without a demonstrated outside-user specification
improvement or demand for grouped CI output. See
[ADR0148](decisions/0148-retire-active-requirement-attribution.md) and the
[hand join and qualification](validation/requirement-attribution-disposition.md).
This candidate is not a published release.

Submit ordinary verification jobs without `--requirement` or a MCP
`requirements` input. Read original results with `sail jobs result JOB`,
observed resource coverage with `sail health JOB`, and per-function mutation
evidence where that development feature is available. Keep requirement/intent
tracking in the project's existing system; Sail does not certify that a
function's specification states a requirement.

Current candidate services advertise no attribution capability. CLI flags and
direct legacy RPC calls refuse with `SAIL3161`; older SDK calls refuse during
capability negotiation rather than launching an unattributed substitute. MCP
no longer advertises or dispatches the retired tool and rejects the removed
input as an unknown argument. Original ordinary job results remain readable,
and historical tags stay in their journals; no new assurance view or tier is
manufactured. Legacy wire/data types remain for compatibility with old records
and older services.

Sail 0.2 also offered neutral source jobs and their `contract-to-code` view.
Those are separately removed; retained neutral records stay listed and read as
`SAIL3161`. General function attribution, evidence tiers and the ledger, raw
SMT runtime requirement IDs and decoded counterexample inputs remain. Use the
MCP `counterexample` tool for original failed-check inputs; those model values
remain distinct from a validated concrete counterexample.
