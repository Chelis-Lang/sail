# Incidental-edit acceptance (#118)

**Status: catalogue and harness prepared; final-runtime acceptance pending.**
The isolated closure-numbering experiment remains evidence for that candidate
only. It does not accept the combined runtime, and it does not cover all
incidental edits. See [VI-0019](../verus-issues/VI-0019-crate-wide-closure-numbering.md)
and [ADR 0049](../decisions/0049-stable-closure-names.md).

| Gate | Current scope and result |
|---|---|
| Frozen edits | `corpus/incidental-edits.json`: Vest, CapybaraKV and IronKV at their full corpus commits; five edits each: comment, blank line, helper insertion, existing helper relocation and local binder rename |
| Source controls | Full-file SHA-256 before editing; exactly one textual anchor; helper relocation preserves all other bytes; restoration checks every catalogue file before each independent edit and before removing each working copy |
| Bounded harness checks | All 15 edits apply and restore against the fetched pins; all 15 edited copies refuse a pristine assertion; changed source pins and ambiguous anchors refuse. Stub dispatch retains all 24 samples in both positive and failure controls; the three failed helper samples make the command fail. No Verus invocation or performance claim |
| Native timing | Pending: old fork, combined candidate and plain stock, three rounds, quiet host, every sample retained |
| Solver work | Pending: exact re-solved satisfiability-check observations. Existing daemon `puts` counts replies of multiple command types; it must not be reported as a count of checks |
| SMT characterization | Pending: unchanged repetitions and every edit on each runtime. The `det` table counts checks after the first byte divergence (with log-only comments omitted), not actual solver executions |
| Correctness | Pending final runtime's full fast/standard conformance, source suite and determinism gates; matching benchmark exit codes alone is insufficient |

The catalogue moves existing module-level helpers within their original
module. Local renames cover a quantified variable (Vest), a proof-local
variable (CapybaraKV) and a closure parameter (IronKV). It does not move
definitions between namespaces or claim that all refactorings preserve SMT
bytes. Changes in proof work are measurements, not an assumed pass condition.

Bounded-check raw evidence, 2026-10-16: archive SHA-256
`46c83b088ee41755447f59fc3cd481027b883e50b06eb2a3639720245390a38f`.
It includes the 15 source round trips, corruption controls, stub command
dispatch checks, scripts and exact source hashes. Stub wall times are fixture
output only and are not performance evidence.

## Exact solver-work capture design

Keep this capture separate from the default-output timing queue: requesting
JSON attribution writes additional logs and changes the measured workload.
Use independent project/cache copies with the same cold/warm/edit sequence.
Bind every capture to its successful Current project result, exact run ID and
that run's sidecars. A reused target's report preserves its establishing
run's provenance; those historical checks are not new solver work.

The existing attribution APIs provide the required join:
`Transcript::pipe_input` reconstructs the exact bytes and `LogFile::new`
records their SHA-256 plus every parsed check. Match each captured transcript
to `SessionRecord.input_sha256`, preserving multiplicity. Reject missing,
malformed, partial or unmatched records rather than treating them as zero.
For every session with `solver_started=true`, count **all** its checks. This
includes checks replayed from its earlier cached prefix: on the first miss,
the current shim feeds that whole prefix to the new solver and suppresses
its already-served replies. JSON `checks_solved` describes answer provenance
and therefore undercounts this actual replay work. Sessions that never start
a solver contribute zero executed checks. Keep prefix replay counts separate
from newly answered checks, and retain raw records for both.

This describes the intended observer; the observer build, complete capture
controls and final-runtime measurements remain pending.

## Native commands

Run on one idle native EC2 host, with no builds, conformance or other timed
work in parallel. The same Sail executable measures both forks. `PATH` must
select the pinned **stock** Cargo-Verus for the plain oracle;
`SAIL_PERF_BASELINE` names the old fork's runtime directory, and
`SAIL_PERF_FORK` names the final combined candidate's runtime directory.
Both runtime paths must already exist; this harness does not build Verus.

```sh
python3 -I scripts/perf-incidental.py check "$SAIL_PERF_CORPUS"
for project in vest capybarakv ironkv; do
  SAIL_PERF_CATALOGUE=incidental \
  SAIL_PERF_MODES='plain sail fork-sail' \
  SAIL_PERF_OUT="$SAIL_PERF_RESULTS/timing-$project" \
    scripts/perf-real.sh run "$project" 3
done
```

Use fresh output directories. The header hashes the catalogue and available
Sail, shim, stock and both fork runtime executables. `catalogue-manifest.json`
records full project pins and original/edited file hashes;
`source-edits.jsonl` records every applied edit. `selected-edits.txt` freezes
the selected cases. Each round restores a pristine source between edits;
each mode has its own fresh project and cache, and mode order rotates between
rounds. Incidental runs refuse missing explicit runtime paths, unknown edits
and zero rounds. A nonzero sample remains in the results and makes the final
command fail even if every mode failed identically.

Run SMT-log characterization separately, outside the timed queue. Set
`SAIL_PERF_FORK` to the old fork, then to the combined candidate, then leave
it empty for stock. Use a separate fresh output directory for each runtime
and project:

```sh
for project in vest capybarakv ironkv; do
  SAIL_PERF_CATALOGUE=incidental \
  SAIL_PERF_OUT="$SAIL_PERF_RESULTS/det-$SAIL_PERF_LABEL-$project" \
    scripts/perf-real.sh det "$project" 3
done
```

Raw SMT files and timing logs stay outside git. The SMT comparison's removal
of log comment lines is an analysis step only; Sail's cache continues to bind
the exact solver input. A missing SMT capture or nonzero incidental edit is
a failed capture, not an empty successful result. Archive complete logs,
manifests, runtime/source identities, refused samples and final source
restoration before host teardown. Add checksums and final results here in the
code PR once those native gates actually complete.
