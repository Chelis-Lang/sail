# Verify neutral Rust source through Sail

A client submits a complete supported Rust source snapshot to Sail's existing
local job service. Sail admits the source, creates its own Verus shadow and
executes its pinned verifier. The client provides source bytes, their SHA-256
and a Rust crate name. Executable paths, Cargo arguments, environment settings
and generated proof code are not submission fields.

```rust,ignore
let jobs = sail_api::jobs::Client::new(job_socket);
let status = jobs.submit_contracts(sail_api::contracts::Submission {
    source,
    source_sha256,
    crate_name: "order_service".into(),
})?;
```

`submit_contracts` negotiates `neutral_contracts_version = 1` before dispatch.
An older endpoint is refused without falling back to ordinary verification.
Use the existing status and result requests with the returned job ID. A completed
result contains the unchanged managed project report and a `contracts` field
with source mappings, exact translation trust and a conservative proof summary.
Ordinary project results omit this field.

The [neutral source subset](spec/neutral-verus-lowering.md) is intentionally
restricted. Unsupported source reports its existing `SAIL3140` diagnostic. The
service accepts at most 64 KiB of UTF-8 source and runs each worker with a fixed
300-second deadline, 24-MiB stdout bound and 8-MiB stderr bound. Exceeding a bound
stops the entire worker process group and returns `SAIL3143`. Invalid source
identities or retained evidence report `SAIL3142`. No partial output establishes
a proof. Cancellation uses the existing durable job cancellation operation.

`Proved` means every mapped function has complete attributed proof evidence
under its stated preconditions. `NotProved` includes incomplete, failed and
unknown outcomes; it is not a concrete refutation. Every underlying solver
unknown and its recorded reason remain visible, even when a legacy function
report says failed. Component function proofs do not establish a tier when the
aggregate is `NotProved`. Requirement IDs and guarantee hashes bind source;
they do not establish semantic entailment or a mutation-gate verdict.

Job currentness is always `Unknown` for the caller's live source. Compare the
manifest's original SHA-256 with the actual current source before using the
snapshot. The managed backend's currentness describes Sail's owned snapshot.
It does not establish that the caller's source still matches.

Read [requirement assurance](requirement-assurance.md) through `sail assurance
JOB --output json` for stable requirement groups, exact snapshot relationships,
original tool identities and qualified evidence tiers. This read never executes
a verifier or establishes missing entailment, feasibility or mutation evidence.

The existing baseline save/read operations retain neutral source and exact
worker stdout/stderr independently of job pruning. New jobs use corrected
lowering version two. Unaffected historical version-one evidence remains
readable with its original bytes; records affected by the compound-field
translation defect are refused without rewriting them. The MCP server has no
tool that submits neutral source: `start_verification` and `verify` submit only
ordinary workspace jobs. Its `verification_result` and `requirement_assurance`
tools read a neutral job through the same service requests, so they return the
same refusal. Every read verifies their hashes
and agreement with the public metadata and historical project report. Generic
comparison and function explanation refuse this evidence family until those
operations have contract-specific semantics. See the [service specification](spec/neutral-contract-jobs.md)
and [decision](decisions/0070-neutral-contract-jobs.md).

Run `ci/local.sh e2e contracts` to exercise the installed Sail service with a
proved positive source and an edited body whose underlying unknown remains
`NotProved`, followed by restart and baseline readback after job pruning. The
compiled runtime body-edit witness is recorded separately; it does not turn an
SMT unknown into a refutation. The same runner checks an overflowing
compound-record-field guarantee (`NotProved`) and its entry-guarded counterpart
(`Proved`), recording the compiled overflow separately.
