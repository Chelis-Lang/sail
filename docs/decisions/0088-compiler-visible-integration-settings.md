# 0088: Accept named compiler-visible integration settings

- **Status:** accepted
- **Date:** 2026-10-27

## Context

Sail's managed integration supplies operational settings to Cargo and the Verus
fork. Build scripts, procedural macros and verified source can inspect those
settings, including through `env!` and `option_env!`, and therefore compile
different inputs from plain Cargo-Verus. Issue
[#435](https://github.com/Chelis-Lang/sail/issues/435) records this difference.

The owner accepts this bounded behavioral-equivalence exception rather than
redesigning Cargo's per-unit launch and introducing a private compiler/solver
transport. The decision documents existing behavior; it does not hide these
settings or select the staged private-solver API.

## Decision

The default-mode equivalence contract permits differences caused by source,
build scripts, procedural macros or other compiler-time code observing these
existing Sail integration surfaces:

| Surface | Observable difference |
|---|---|
| Solver routing | `VERUS_Z3_PATH`, `SAIL_Z3_PATH`, `SAIL_SOLVER_MODE`, `SAIL_DAEMON_SOCKET`, and `SAIL_PROVENANCE_DIR` when provenance collection is active |
| Cargo wrapper selection | `RUSTC_WRAPPER` names Sail's managed driver alias instead of the ordinary Verus driver |
| Cargo-Verus driver options | The physical ` __VERUS_DRIVER_ARGS__` contains Sail's JSON, timing, attribution-log and retained-checksum observation options before the user's options |
| Source mapping and compiler-input collection | `SAIL_RUSTC_DEV_SOURCE_BASE`, `SAIL_VIR_SPAN_REMAP`, `SAIL_COMPILER_INPUTS`, `SAIL_COMPILER_INPUTS_BINDING`, `SAIL_NATIVE_COMPILER_INPUTS`, and `SAIL_NATIVE_COMPILER_INPUTS_BINDING`, while their owning features are active |
| Interactive-engine controls | `SAIL_ENGINE`, `SAIL_ENGINE_FUNCTION`, `SAIL_ENGINE_EVENTS`, `SAIL_ENGINE_FOCUS_REPORT`, and `SAIL_ENGINE_CONTEXT`: default managed invocation clears caller values before Cargo; explicitly selected interactive mode supplies its own compiler-child values |

Code that observes these values or their absence may choose different source, constants or
proof obligations, so its verdicts and diagnostics can differ from plain
Cargo-Verus. The exception covers that causal difference, not unrelated
verdict/diagnostic changes or arbitrary future control channels. Existing
caller-owned private-alias settings are not newly exposed by this decision.

All solver and acceptance obligations remain in force for the actual effective
inputs Sail verifies. Default solver keys bind the exact actual query bytes,
solver identity and mode. Freshness, full requested scope, query/reply
association and truthful reporting remain required. The exception never permits
reusing evidence for different effective inputs (including a changed observed
control value), accepting an answer to another
query, changing a solver answer, or reporting an unresolved obligation as proved.

The conformance harness is unchanged. A mismatch remains a failed comparison
until its cause is established and recorded against this bounded exception.
This decision adds no output filter, skipped corpus case, accepted-status
heuristic or generalized `Different`-to-`Same` conversion. Programs that do not
observe the listed surfaces retain their existing equivalence requirement.

## Consequences

Users who need plain Cargo-Verus and Sail to compile equivalent inputs must
avoid depending on the listed operational settings. Their presence is an
observable integration limit, documented in [divergences](../divergences.md)
and [known issues](../known-issues.md).

Issue #435 resolves by an explicit contract decision after these documents
merge, not by a claim that the settings became hidden or were removed.
The unselected private-solver fork component is not required for this decision;
no Cargo/backend fork, platform provider, runtime pin or release is selected.

No end-to-end false proof is demonstrated or ruled out by accepting this
exception. The broader acceptance proof and its remaining input/process
boundaries continue under [#368](https://github.com/Chelis-Lang/sail/issues/368).
