# A failed neutral body edit with an underlying unknown answer

Found on 2026-10-21 using the pinned native Linux engine `f8589ed4345f147d8fb43d8cb7992921944b91f0`, stock source `7d829946231d6413a398f6e4160cdfe951261dad`, and Z3 4.16.0.

`body_edit_verus.rs` is the exact emitted source from the actual managed neutral
helper acceptance run. The original neutral source requires `input < 100` and
ensures `next_value(input, result)`, meaning `result == input + 1`. The body edit
returns input unchanged except for its existing zero-input early return.

The managed run exits 101 with no compiler error. Its `neutral::candidate`
function has a legacy failed/confirmed/SMT-tier label. The primary nonadvisory
obligation contains an unknown answer with reason
`(:reason-unknown "(incomplete quantifiers)")`, followed by unsat. A model is
retained for the unknown check. A model after unknown is not evidence of SAT.
This record makes no claim about why the pinned solver could not decide it.

The opt-in [scalar literal witness family](../../../docs/literal-witnesses.md)
uses a separately attributed quantifier-free query and independent scalar
evaluation. It does not change this original Verus query or classify its UNKNOWN
as a SAT rejection. Predicate-based originals such as this helper fixture remain
outside the initial witness translation's admitted slice.

The exact requested-target report is retained in
`crates/sail-verus/tests/fixtures/neutral-contracts/body-edit-requested-target.json`.
Its full original run and diagnostic hashes are in the
[native qualification](../../../docs/validation/neutral-verus-lowering.md).
Reproduce through the documented `neutral_contracts_acceptance` library example,
which invokes the actual pinned managed runner and records the raw result.

The neutral projection returns `NotProved`, retaining unknown counts and reasons.
It does not change ordinary legacy ledger/report semantics. The separate compiled
runtime fixture violates its postcondition at input 1 with a satisfied precondition.
That concrete runtime sample is independent of the unknown SMT check, and neither
an SMT refutation nor a mutation-gate verdict is inferred from the legacy label.
