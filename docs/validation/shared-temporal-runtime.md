# Shared native temporal runtime

Status: implemented; the final source passed the local Linux workspace gate.
Native EC2 qualification, macOS platform qualification, installed consumer
acceptance and protected CI are separate gates. No result from those gates is
claimed here until its source and original evidence are recorded.

## Source and independent review

The reviewed implementation is commit `75bc5f5a`, based on worker/main merge
`a420ee7d`. Its exact patch has SHA-256
`d9af26512968d8cbb4bb989f6b81cf86a0674e40c1bc9b598b4e303833c2e6fb`.
Independent fresh-context review reported no findings; original review
`review.md` has SHA-256
`139c42cc46b2102e5993b1694ef5afd16d62218df640314558d33ee0b3c61e31`.

Final behavior source `e15a245f6dc9e4671d956f60fed6e931d7ea1f70` adds only the
reviewed dependency correction for RUSTSEC-2026-0386. It pins Tokio 1.53.3 and
preserves the D.I.V.E. revision
`1025cd6a0510183bc16abc1e6e3eea4f9305c5a9`. No Java, TLC or D.I.V.E. command
runs in this integration.

## Local Linux gate

`ci/run.sh gate` passed with two build jobs and two test threads on final source
`e15a245f`. It ran formatting, full-workspace strict Clippy, nextest, the MCP
example tests, doctests, rustdoc, layering, dependency policy and benchmark
page freshness. Nextest reports 998 passing tests and 44 explicit skips.
The skips include backend-dependent suites; they are not acceptance results.
The original complete log `full-gate.log` has SHA-256
`58b6417253fac5d48d7f1976cafb8834acf737d9f4feffa49419153c875979d2`.
The supervising command recorded exit zero. This is a local native Linux
correctness gate, not the separate EC2 conformance or performance qualification.

Actual shared-service controls execute the pinned worker through the private
runtime endpoint. They cover the generated Step plus Within fixture, a real
action-property mutation with independent native replay, valid and forged trace
validation, finite simulation, stopped exploration, host-module refusal,
whole-worker timeout and cancellation that restores service capacity.
Existing worker process controls retain cancellation after observing a real
child, wrong identity and missing proof refusal, malformed/trailing/oversized
output, unexpected stderr and an exited leader with descendant-held pipes.
Capability controls preserve old SMT clients/new services and explicit absence
of temporal support on old services. Staging tests bind final executable bytes;
missing, incompatible and changed installations establish no readiness.

## Installed development artifact

A separate immutable debug installation was built from `e15a245f` after the
full local gate. Its native worker manifest binds source revision, protocol,
platform and exact executable bytes. These are development artifacts; they do
not establish a packaged release or an installed consumer's result.

| File | SHA-256 |
|---|---|
| `sail` | `cc17d9771e5c1e56db9159379fb4fc57b90e46d432d11dbbf1eb1c4e65d35d0d` |
| `sail-runtime-worker` | `273fa3be0308fff4ae332188c3a641de7382b13515bd766cf07008e5bcb79ea0` |
| `sail-runtime-worker.json` | `31e3a272a42b2910f31405551fa6feaf1f8a7ceddbafd80d831dd86e7048c216` |

## Earlier attempts

| Source and attempt | Result and original evidence |
|---|---|
| Reviewed implementation before the Tokio correction | Earlier gate stages passed, including 998 tests, but dependency policy correctly failed on RUSTSEC-2026-0386. Original `full-gate-pre-tokio.log`: `5b31c0ed697c93eec0b343c91f262886dd753df6732750648cc7af34e6a3162c`. |
| `e15a245f`, initial retry | Interrupted at the owner's request while credential-capable background work was stopped. No complete result is claimed. Original `full-gate-interrupted-auth.log`: `39ef6be53a580ec0d734d37e9fba80d59c376880390d24aaa36dd38ce4d72408`. |
| `e15a245f`, sealed-environment retry | Complete local gate passed as recorded above. |

## Assurance limits

No temporal cache, evidence tier, requirement ledger relationship or hard RAM
limit is established by this slice. Native process completion is separate from
logical outcome and typed complete exploration. Simulation and an independently
validated individual trace never establish an exhaustive property. Operational
paths and elapsed values remain exact original evidence; they do not enter
semantic source, projection or task identities. Identical recorded evidence can
be replayed deterministically, but fresh raw reports are not claimed byte-identical.
