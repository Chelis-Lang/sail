# Read requirement assurance

Requirement assurance is experimental. It groups the evidence of an ordinary
`sail verify` job by requirement IDs that the submitter attached to exact
functions. The link from an ID to a function is an unchecked submitter claim:
Sail proves the function against its own Verus specification and does not check
that the specification states the requirement.

Sail 0.2 also offered a `contract-to-code` view over neutral source jobs. Sail
0.3.0 removed neutral source jobs and that view. A retained neutral job record
stays listed and reads as `SAIL3161`.

## Ordinary Verus jobs

An ordinary `sail verify` job proves the submitted Verus source against its own
specifications. A submitter can attach requirement IDs to exact functions when
it submits the job:

```sh
sail verify --background --output json \
  --requirement REQ-1=demo::checksum --requirement REQ-2=demo::parse
sail assurance JOB --output json
```

Each `--requirement` names a requirement ID and the exact crate-qualified
function name the verifier reports; the last `=` separates them. Attribution
applies to stock project and standalone-file jobs submitted with
`--background`, not to engine or repair jobs. The API is
`sail_api::jobs::Client::submit_attributed` and
`Client::attributed_assurance`; MCP accepts a `requirements` array on
`start_verification` and reads the view with `requirement_assurance`. The
`--attributed` flag and the MCP `attributed` argument, which selected this view
beside the neutral one, are still accepted and change nothing. The service
retains the canonical, sorted attributions with the durable job and across
restarts. Identical submissions coalesce only when their attributions also
match. Reading a view does not submit source or run a verifier.

The version-one attributed view groups rows by requirement ID. Each row has the
`verus-specification-to-code` relationship and the `submitter` attribution
source. A row is `Proved` only when the whole job completed, the verifier
reported exactly one requested-target function with that exact name, and that
function proved. The service reads the job against its live inputs first, so a
job whose inputs are no longer current reads `NotProved`. `NotProved` includes
verifier unknowns and does not assert a concrete counterexample. One passing
function cannot promote an incomplete whole-job result.

Verus reports a function that proves only through `assume(false)` as verified,
and the view cannot see assumptions in the source. A `confirmed`,
`smt-verified` tier therefore needs all of:

- actual original solver identities;
- inputs the service classifies as current;
- Verus's `--no-cheating` among the job's Verus arguments (after `--`), which
  refuses `assume`, `admit`, `external_body` and `assume_specification` in the
  verified crate;
- no other Verus arguments outside a reviewed set of selection, resource-limit,
  threading, report-detail and standalone crate-naming options
  (`--verify-root`, `--verify-module`, `--verify-only-module`,
  `--verify-function`, `--rlimit`, `--num-threads`, `--multiple-errors`,
  `--expand-errors`, `--time`, `--time-expanded`, `--crate-name`,
  `--crate-type`, `--edition`).

In a Cargo project, `cargo verus verify` forwards the Verus arguments after
`--` to every verified package, including the `vstd` dependency, which uses
`assume`, `admit` and `external_body` itself. A Cargo job with `--no-cheating`
therefore fails in `vstd`, and its rows read `NotProved`. Standalone-file jobs
record no per-check solver provenance, so their rows also read `NotProved`.
Attributed rows therefore report `Proved` without a tier until Sail can apply
the check to the requested package alone.

The view records the job's arguments verbatim. Every row carries qualifiers
stating that requirement entailment, input feasibility, mutation and
termination are not established, that the link is an unchecked submitter claim,
that the Verus specifications and toolchain are trusted, and whether the job
passed `--no-cheating`. An unreported function, missing solver identity,
non-current input, missing `--no-cheating` or unreviewed Verus argument adds
its own qualifier and establishes no tier. Mutation status is always
`not-established`.

## Decoded model inputs

A row that is not `Proved` carries decoded model inputs from the solver model
of that function's first failed or unknown check. Sail keeps parameters and the
return binder (constants Verus names `IDENT!`), renders scalars in ITF, decodes
struct values into records with fields named by declaration position, and
reports how many internal constants (fuel, type identities, labels) it omitted.
The raw model stays in the obligation's `counterexample` in the job result. The
decoded values are not evaluated against the source, so they are not a
validated concrete counterexample, and an unknown answer may give a partial or
spurious model. Decoded inputs never change an outcome, establishment or tier.
Library consumers can decode any project report with
`sail_api::counterexample::decode`. The
[decision](decisions/0106-decoded-counterexample-inputs.md) records the limits.

The client negotiates `attribution_version = 1` and fails for older services.
Reading attributed evidence for a job submitted without attributions fails with
`SAIL3160`. The
[decision](decisions/0104-ordinary-job-requirement-attribution.md) records the
trust boundary.
