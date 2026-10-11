# Initial HTTP hit refusal before disconnect

The frozen `2d6539f` local workspace gate passed 526 of 528 tests. Its existing
`disconnect_hides_cached_transport_without_initializing_or_waiting_for_drop`
control failed the first `Lookup::Hit` assertion, before disconnect or any
assertion about hiding an existing transport. The transport/trust setup reported
126.125333 ms; serving retained its original 250 ms budget. Classification
reported 25.692 ms and one CPU tick, with `CLK_TCK=100`. These observations do
not identify the lookup variant, the transport phases or the cause of refusal.
Classification output appeared after the failing assertion; it does not prove
that the result was published within the serving budget.

The fixture already establishes actual transport and trust without issuing an
HTTP request, then invokes the normal lookup once. The existing server delay
remains 75 ms. The added assertion message records the actual lookup variant,
existing remote metrics, existing phase timestamps and bounded request-read
metadata on failure, matching adjacent controls. It does not add a request,
readiness wait, phase, retry or change any assertion, deadline or concurrency.
No additional Cargo build or test was run for this diagnostic-only change while
the independent real HumanEval workflow was active. The original failed gate
remains a failure; the disconnect behavior was not reached in that run.

See [the shared-store guide](../http-shared-cache.md) and
[the native ownership workflow record](m7-owned-reader-d20.md).
