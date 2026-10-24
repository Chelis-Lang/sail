# Scalar literal-contract witness qualification

Status: local generated-query and concrete-evaluation checks pass. Qualification
of the new installed source-owner service on native EC2 remains pending.

The candidate is based on protected Sail `d24297db`. Source admission and typed
specification semantics reuse its existing neutral lowerer. Default Verus
lowering, native mutation classification and solver configuration are unchanged.
The witness translation is explicitly named
`neutral-scalar-literal-contract-witness-v1`.

The local generated-source runner used the previously qualified installed Sail
binary SHA-256
`0995010368465958b2b1b56da269b97687d8a927013c17a4894f0f697ea40f26`
for actual raw-runtime execution. Each returned response identified pinned Z3
4.16.0, executable SHA-256
`e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765`.
It did not execute the new preparation/validation RPCs in that older binary.
The new owned-library preparation and concrete evaluation ran in the candidate.

The retained matrix records sixteen actual SAT responses and sixteen decoded,
independently evaluated concrete assignments across Boolean identity, guarded
unsigned increments, signed/wide integers, checked multiplication, scalar
conditionals, eager old captures and the original order-service price function.
The order-service original source SHA-256 is
`04c4578ba8b9f6f2ac778745ee642408d56b555b1ed10cff5e1c8b88d3afbafe`.
Its nominal, multi-requirement and reference-input targets remain explicitly
unsupported. Tautological guarantees, impossible preconditions, undefined
postcondition arithmetic and undefined eager old captures produce eight UNSAT
responses and no concrete counterexamples.

Focused source/model tests pass, including deterministic queries, exact signed
and unsigned 128-bit bounds, changed source/query detection, false-precondition
and out-of-range refusal, undefined arithmetic, incomplete/mistyped assignments,
altered original stdout, UNKNOWN refusal and zero-input models. The new local
source-owner RPC preparation test passes and confirms that preparation starts
no source jobs. Clippy passes for all targets of the touched API, producer and
daemon packages. These results are correctness checks, not native service,
conformance, performance, platform or release acceptance.

Raw plans, requests, solver output, assignments and local check logs are retained
outside the repository. Their artifact index SHA-256 is
`d109b00e9191d480c7da171738b6c94bd52dd1de4de009c4e7f6d1ac8c9dfb1c`.
The source-owned native acceptance runner supports an already running new
service through `SAIL_WITNESS_SOCKET`; required native gates and that service
matrix remain the next qualification step.

Failed local attempts remain retained: the initial manual runtime probe selected
the engine parent instead of its concrete build and correctly returned
Unavailable; correcting only the path produced actual SAT. An initial shadowed
result test expected a returned unsupported entry, while existing source
admission correctly rejected the entire invalid source; the test was corrected.
Intermediate lint failures were corrected before the passing local checks.

This gate establishes concrete violations of the exact unchanged scalar
contract for admitted literal bodies. Requirement IDs are attribution, not
independent semantic requirement rejection. Native Verus UNKNOWN remains
Inconclusive, no native positive kill is claimed, and no proof tier or full
realizability claim follows from this evidence.
