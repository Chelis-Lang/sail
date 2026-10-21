# Neutral Verus lowering qualification

The explicit source lowerer was checked against both the pinned Sail fork and
stock Verus on native Linux on 2026-10-21. Both verifiers received the exact same
generated source for each control, with the pinned Z3. No neutral contract is
established by this record alone: caller-source currentness, actual obligation
mapping and contract-to-requirement entailment remain separate checks.

The native oracle required the expected actual success or failure, complete
crate verification and matching verifier result fields. Every control reported
`encountered-vir-error = false`; compile failures cannot satisfy the negative
controls.

| Exact source control | Expected outcome | Verified / errors, on each verifier |
| --- | --- | --- |
| Add one under bounded input | Verified | 1 / 0 |
| Short-circuit arithmetic guard | Verified | 1 / 0 |
| Signed minimum | Verified | 1 / 0 |
| Mutable wallet and old scalar state | Verified | 3 / 0 |
| Negated overflowing predicate | Refuted | 0 / 1 |
| Overflowing eager old capture | Refuted | 0 / 1 |

These are whole-crate backend counts, not requirement establishment counts.
The runtime fixtures compile with the real neutral macros and check the same
bounded input and state transitions independently.

Two failed attempts remain part of the evidence. The initial attempt could not
find the CI image's engine through the ordinary mutable cache; fixture lookup
now validates the explicit `SAIL_ENGINE_BUILD` runtime, while preserving strict
missing-input failures. The next attempt exposed a real mutable receiver
postcondition error. Lowering now uses `final(self)` for current poststate and
`old(self)` for entry state, including all arithmetic definedness conditions.
Entry preconditions and eager capture assertions still refer to entry state.
Mutable by-value inputs and nonreceiver reference parameters remain unsupported.

The actual oracle log SHA-256 is
`bdafc1d26b0830f3a9eb4eb7737c9bd87178c0ba42bfdf4d8280e87159c10edc`.
Generated sources, source mappings and original verifier stdout/stderr are
retained with both failed attempts. See the
[lowering specification](../spec/neutral-verus-lowering.md) for the supported
subset, source-policy admission and the explicit native command.

The corrected lowering source passed native `ci/local.sh gate`, the complete
solver lane (including this real oracle), full E2E and `conformance fast pr`.
Conformance found no differing verdicts, unstable units or infrastructure errors;
resource-limit drift retained the existing comparison policy. This adds no
conformance corpus entries or default-route behavior changes.

| Native log | SHA-256 |
| --- | --- |
| `sail-neutral-lowering-gate.log` | `27a63ef9d0053f1993ee4700aed20708b74c94d123e76597539f1b14695ba054` |
| `sail-neutral-lowering-solver.log` | `c3a5ad58ba3e4704b20fe77366b00ac1f793d651ad91a5027034ec35589ddd27` |
| `sail-neutral-lowering-e2e.log` | `bcbe6b21ece37a17a2f3b75b2f8aa33bf0121e983552ccd0d696862e3bc61bda` |
| `sail-neutral-lowering-conformance.log` | `46695dca3369023d6435b84f2695eeba0b3802225c0a82061c0070bc3188d89c` |

The retained oracle archive, including the original failed attempts, has SHA-256
`e84e7245bb7ea743c6d265184e215f19f004b2fabeb3b57e02f38648bf2eba99`.
The explicit `neutral_contracts_acceptance` example checks the separate public
managed helper. Its library invocation is independent of the direct verifier
oracle; a matching direct-verifier result does not establish that helper path.
