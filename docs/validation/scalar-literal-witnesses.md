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

Candidate `76aa6e19` subsequently failed independent review: the first-pass
observer did not bind `Check.command_index`, and the scalar model decoder
accepted malformed integer terms `(-3)` and `(-0003)` through the general
presentation decoder. These were response-integrity defects; the retained
experiment did not establish a false semantic contract violation. The corrected
candidate binds the sole final exact check command through the shared SMT
parser and uses a local structural scalar decoder. SAT and UNSAT attribution
tampering, malformed integer terms, valid unary minus and signed extrema now
have explicit regression coverage. The general presentation decoder is unchanged.

The native full gate on `76aa6e19` also failed: 1,061 tests passed and the error
registry test failed because `SAIL3145` was used without registration. This is a
completed gate failure, not an interrupted run. Original gate log SHA-256:
`d99b0d842cb8f33086655912191d7b8a5fdd168841526e17098355198478c7a9`.
The corrected candidate registers the code; renewed independent review and
the complete native qualification remain required.

The corrected local source passes its focused witness tests, preparation RPC
test, exact error-registry test, layering check and all-target clippy for the
touched packages. Its refreshed actual installed-runtime matrix again retains
sixteen SAT responses with independently evaluated assignments and eight UNSAT
controls. Corrected raw artifact index SHA-256:
`937d4e410e4989b6f9550987be4086df836757d6a38b5dcdc21f2e52d951060d`.
The original candidate, rejected review, failed native gate and earlier raw
evidence remain distinct and retained. The corrected local harness initially
used an unmatched error-registry test filter; a separately retained exact test
execution passes that existing registry test.

This gate establishes concrete violations of the exact unchanged scalar
contract for admitted literal bodies. Requirement IDs are attribution, not
independent semantic requirement rejection. Native Verus UNKNOWN remains
Inconclusive, no native positive kill is claimed, and no proof tier or full
realizability claim follows from this evidence.
