# Z3 resource counts can vary for the same successful vstd query

On an EC2 mac-m4.metal in us-east-1c, macOS 27.0 (26A428), the pinned
f8589ed4 runtime based on Verus 7d82994 verified vstd at its default resource
limit and at 100. Three fresh runs at each setting emitted identical solver
inputs apart from the resource limit, and identical answers. One solver
process's final resource count varied between 114117 and 114131. The earlier
counts were 101623 and 101675. No query reached its resource limit.

`query.smt2` preserves that single process and its one `check-sat`, including
the required prelude. It is not minimized term by term. Replay it with the
pinned runtime's Z3, starting a new process for each sample:

```sh
python3 corpus/repros/z3-resource-count-variation/repro.py /path/to/pinned/z3
```

Direct replays in fresh Z3 processes reproduce the same two counts.
Variation is intermittent; an unobserved sample is reported explicitly.
The script changes only the nonzero resource limits for its raised arm.
It does not normalize solver inputs or participate in Sail's serving path.

The release recorder correctly refuses its resource-limit subsumption witness
when counts differ. Separate proofs recorded under their exact flags need
no subsumption witness. This reproduction establishes counter variation;
it does not attribute a cause or establish an answer disagreement.

Source and licence: generated from `vstd` 0.0.0-2026-10-18-0147 through
Verus f8589ed4 and its pinned Z3. Vstd is MIT licensed; see the vstd entry
in [third-party notices](../../../THIRD-PARTY-NOTICES).
