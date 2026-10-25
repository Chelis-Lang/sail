# Neutral contract jobs, version 1

Status: proposed implementation boundary after the qualified neutral helper.

## Request and capability

The existing private Sail project-job endpoint gains a separately negotiated
neutral source family. `Capabilities.neutral_contracts_version` advertises
version 1. A client requires that exact capability before submitting; an older
endpoint is an explicit unsupported service, with no ordinary-Verus fallback.

`SubmitContracts` carries `source`, `source_sha256`, `crate_name` and an optional
`profile`. Omission selects profile one and retains its exact serialized shape.
Profile two requires `Capabilities.neutral_contract_profiles` to contain `2`;
older services cannot silently accept it. Unknown profiles are refused.
The source is exact UTF-8, at most 64 KiB. The required lowercase SHA-256 must
match those bytes. Unknown request fields are rejected. No project directory,
shadow, executable, arguments, environment, dependency selection or verifier
controls are accepted from the caller. Source admission and supported semantics
are those of [neutral Verus lowering](neutral-verus-lowering.md).

## Owned execution

The existing durable job scheduler and owned worker-process supervision execute
the request. Source and request files live in the private Sail job store under
server-generated names. The worker uses Sail's pinned Cargo/Verus/Z3 factory and
the qualified `contracts::verify` helper. The worker inherits only operator-owned
runtime discovery settings; it never imports a submitting client's environment.
The parent-death guard, process-group cancellation and existing job logs remain
in use. Neutral jobs additionally have a fixed 300-second deadline, 24-MiB exact
stdout limit and 8-MiB exact stderr limit. Exceeding a bound kills the group and
produces an explicit job error, never a parsed prefix or proof.

The worker returns one versioned owned envelope containing the original
`ProjectResult` and typed contract evidence. No public result contains the
helper's `shadow` field. Pure source ranges, manifest/bindings, proof and
translation-trust types live in `sail-api::contracts` and are reexported by the
helper. Backend execution, source admission and proof derivation stay in Sail's
backend. The original project result remains the existing result payload;
`jobs::ResultDocument.contracts` is an optional additive evidence field, absent
for ordinary jobs. It contains the manifest, conservative proof and translation
trust, not a fabricated per-requirement verdict.

New profile-one jobs require lowering semantics version two. Profile-two jobs
require lowering version three and contract version two. Source-job protocol
version one remains unchanged. Reads bind the manifest profile to the exact
original selected submission. A worker cannot supply a historical lowering for
a new job.

Successful processing with a backend result is `Completed`, including a failing
body whose contract is `NotProved`. Unsupported source, missing tools, setup,
process, deadline and output failures retain their exact error codes through the
existing job-error path and contain no fabricated project result or proof.
Cancellation retains the existing explicit cancelled lifecycle.

## Currentness and retention

Public job currentness is `Unknown` for the caller's live source. An owned
shadow's currentness cannot establish currentness of that source. The original
backend result records its establishing snapshot; the caller compares the
manifest's original hash with its actual source on every read before accepting
current assurance. Contract proof applies under requires. Entailment, feasibility,
requirement tiers and mutation establishment remain independent work.

Each neutral result read verifies the complete retained worker bytes against the
existing job-output seal, the original submission's exact source hash, crate
identity, and the returned manifest. Retained contract metadata is never
reconstructed from source positions or legacy function tiers. The public
baseline adds optional typed contract evidence. A neutral baseline independently
retains the original source submission and exact worker stdout/stderr bytes outside job pruning,
with exact SHA-256 seals checked on every read. Its public metadata and historical
backend projection must agree with that original evidence. Retained version-one
lowerings are admitted only when their exact original manifest matches the
corrected projection under its original header. Affected version-one evidence
fails closed without changing the original report, source or output bytes.
Ordinary baseline
encoding and behavior are unchanged.

Neutral jobs support status, result, cancellation, logs and baseline save/read.
Generic function comparison/explanation is explicitly unsupported for neutral
jobs until it understands this contract evidence; a legacy failed function
containing unknown is never converted into a neutral refutation.

## Acceptance

Fast tests cover capability refusal without submission, strict source-only
request decoding, bad digests, bounded execution/cancellation, persistence,
restart/readback and tampering. Existing ordinary job and baseline tests remain.
An actual installed Sail service runs the original scalar fixture and its real
body edit through `sail-api` only. The original is `Proved`; the edited body is
`NotProved` with its actual unknown retained. A separate compiled runtime witness
records the precondition-satisfying input and exact postcondition violation.
Source and output artifact hashes are retained. This actual service acceptance
runs in the end-to-end tier, alongside full gate, solver and conformance checks.
The compound-record-field regression separately submits the original unsafe
source (`NotProved`) and an entry-guarded source (`Proved`), retains their original
solver outcomes and records the exact runtime overflow at the admitted input 255.
Historical controls admit unchanged safe version-one jobs and baselines and
refuse affected records even with otherwise intact content seals.
