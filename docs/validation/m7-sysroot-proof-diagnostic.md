# One detailed sysroot proof diagnostic

The reviewed [wrapper and frozen inputs](m7-sysroot-proof-diagnostic-plan.md)
ran once against product `7238da8` and the exact selected b346 compiler manifest.
The [complete portable record](m7-sysroot-proof-diagnostic.json) retains the
original 600-second proof, 30-second CLI and five-second RPC bounds. No ordinary
preparation, manual `Prepare`, old receipt, additional proof or declaration retry
was used. This is a cause diagnostic, not reader or M7 acceptance.

## Actual result and cost

Job `1791701479968-650715-0` completed with exit code zero, no infrastructure
error, `accepted=true` and full `Current`. The independently complete full result
verifies all 1,871 vstd and 331 HumanEval functions, with zero failed, unknown,
provisional or unstable full obligations. The exact requested GCD function is
verified at the confirmed tier in the full result; its focus result remains
provisional. The two actual compiler invocations include dependency and project
work, with no reused targets. They are not a measured one-driver repair result.

The only full note says build-script input declarations were established. No
sysroot coverage failure or detailed `SAIL3114` reason occurred. This new proof
therefore does not reproduce or explain the original
[D20 failure](m7-owned-reader-d20-sysroot-audit.md). Its new context and input
revision are explicitly recorded; they are not forced to the historical values.
All 273 original template files remain unchanged, and the existing generated
lockfile is independently sealed. No historical lockfile hash equality is
claimed.

The outer controller inclusive duration is **478.014155386 seconds**. The wrapper
inclusive duration, including its input checks, owned cleanup, archival and final
seals, is **475.379727009 seconds**. The original submission-to-public-result
wait is **459.975781127 seconds**; the report's execution duration is
**444.132543119 seconds**. These measure different intervals and are retained
separately. Cargo reports its invocation finished in 6 minutes 26 seconds.

All 18 timer rows are retained: the original setup phases plus the Linux handoff
and natural fill-hint phases. Initial sysroot hashing takes **19.112358 seconds**;
the initial original coverage barrier succeeds in **22.522 ms**. The cold handoff
check takes **43.994 ms**, and the naturally emitted fill hint takes **9.281 ms**.
The aggregate runtime-context timer overlaps its subphases and must not be added
to them. There is no second lookup or warm digest-reuse performance claim.

## Closure and evidence

Owned daemon `650715` exits normally with code zero in **0.813971375 seconds**.
Cleanup, archival and sealing error lists are empty. All input, controller,
product, selected-runtime and original-reference seals match before and after.
The available worker stdout, stderr, proof-source events and timer sidecar are
retained through the original owned helpers; environment-bearing records are
not exported.

The raw archive contains **27 files / 42,302,687 uncompressed bytes**. Its gzip is
1,170,244 bytes with SHA-256
`d61cfe0e3480155d406feb88b683e86236e408b5c9a4074484f77fbc626faf5b`.
The 4,881-byte inventory has SHA-256
`f0f7baf15a00578e18c511d45d8ea361d76c116c9e690eab0ec9f67cffe8f7ee`.
Every archived file is independently verified against that inventory after
copying. The original D20 raw archive remains unchanged.

No Python or Rust census, retained CLI/MCP workflow, source physical-budget gate
or 2× edit benchmark was attempted. Independent concurrent source-control work
is recorded as a scheduling fact, without a causal attribution. See
[the reader guide](../reader-acceptance.md), [freshness](../freshness.md) and
[the roadmap](../ROADMAP.md) for the original outstanding acceptance requirements.
