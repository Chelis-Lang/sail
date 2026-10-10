# Separately prepared reader baseline

The [retained preparation record](m7-reader-prepared-baseline.json) establishes
one genuine fresh Current full proof with the exact 8e product and b346 runtime.
It records 1,839.532 seconds of unmeasured baseline preparation, including the
first Unknown repair result and the existing declarations-establishment retry.
No prior receipts or compiled targets were copied. The active workspace and
original bounded reader checks are still pending in this record; this is not
a latency or overall M7 acceptance result.

The final preparation made three actual compiler starts: HE, vstd, then HE.
It cannot satisfy the measured one-driver requirement. Its optional preparation
telemetry was absent, so this record does not infer a preparation hit or stock
fallback. All full target reports and the failed first result remain preserved.

The complete raw report has two distinct count fields. Verus's structured
crate totals are 2,060 for vstd and 441 for HE, matching the earlier a6/b346
baseline. The attributed function-record summaries are 1,871 and 331. Both
occur in the same response; the smaller attributed totals do not establish a
reduced source scope. The record labels both metrics and the exact target flags.

The frozen collector metadata retains a historical 1dc product-origin note.
It is not this run's product identity: the actual 8e source and binary hashes
are independently recorded. Neither collector was changed or opted into the
new result decoding cache. See [reader acceptance](../reader-acceptance.md) for
the separate preparation, original 600-second checks, four-reader 200-sample
200 ms p95 gates, and required current proof evidence.
