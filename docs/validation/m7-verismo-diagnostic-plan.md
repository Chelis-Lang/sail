# Focused default Verismo timeout diagnostic

**Prepared, not run.** This is one supplemental diagnosis of the stock plain side
of `verismo/source/bitflags_verus/src`. The root-provided frozen executable and
runtime identity are still pending. No build or proof has been launched for this
plan. [The plan record](m7-verismo-diagnostic-plan.json) separates helper controls
from executable evidence.

The complete frozen1dc standard run failed:159 units,156 Same, one Anvil Unstable,
zero Different and two Verismo infrastructure timeouts at the original3600-second
limits. Fill/warm were not reached. Its inner timeout streams were lost by that
older harness. Preserve [the complete failure](../../bench/validation/m7-linux-default-standard-1dc-failure-2026-10-11.json)
and [the earlier nonterminal observation](m7-standard-verismo-timeout-observation.json).
Sleeping wait channels and low CPU identify neither a solver defect nor a load
cause. A later isolated success cannot repair that complete failed scope.

## Exact scope and invocation

Use a newly frozen `sail-conformance` containing the existing partial-stream
retention, the frozen companion stock `sail-z3`, and the original primary corpus,
Verus and solver pins. Record their complete hashes before execution. The working
directory must contain the frozen `corpus/toolchain.toml` and `corpus/corpus.toml`.
Keep the original cache/toolchain selection and ordinary default mode; no optional
source build, repair engine, alternate solver, daemon or new flag is introduced.
The harness continues to own its scratch preparation and per-side process groups.

The only selection is the first failed unit; one worker, one initial plain run,
and the existing two confirmation runs if a difference requires them:

```sh
SAIL_UNIT_TIMEOUT=3600 "$CONFORMANCE" run \
  --tier standard --project verismo --filter /source/bitflags_verus/src \
  --jobs 1 --shim "$SHIM" --out "$EVIDENCE/harness" \
  >"$EVIDENCE/controller.stdout" 2>"$EVIDENCE/controller.stderr" &
actor=$!
```

`CONFORMANCE`, `SHIM`, the working directory and `EVIDENCE` are reviewed frozen
inputs, not implicit current build paths. `EVIDENCE` is a new retained directory
outside Cargo scratch. There is no `--trace`, shortened timeout, query rewrite,
extra solver flag, lowered proof priority or retry of the second failing unit.
If the first plain side completes normally, the unchanged harness proceeds to
its ordinary Sail side and classifier. Record the entire focused outcome; it is
not a complete standard-tier retry.

After launch, the independent controller records the actor PID and exact Linux
start-time ticks from its stat record. Run the observer in a separate process:

```sh
python3 scripts/m7-conformance-observation.py \
  --pid "$actor" --start-time "$actor_start_ticks" \
  --output "$EVIDENCE/process-observations.jsonl"
```

The controller waits for and records the harness normally. It must not use the
observer's completion as a proof result or kill deadline. The original3600-second
per-side limit and harness kill/drain/classification are unchanged. Observation
setup, disk failure or a stuck metadata read cannot delay that independent actor
control. The observer never signals any actor. On external cancellation, cleanup
is limited to the exact owned observation process; existing harness ownership
remains with its controller.

## Observed authority and bounds

The existing source-gate observer tracks one process group. That is insufficient
here: conformance puts each timed unit in a different group. The new helper pins
the leader's Linux proc directory and expected start-time, snapshots parent
relationships, and verifies every pinned parent/start-time chain immediately
before reading descendant metadata. A missing/reparented ancestor drops the row;
leader loss/reuse ends observations. Proc directory descriptors prevent a PID
pathname from redirecting payload reads to a replacement actor.

Samples contain process/thread CPU ticks, RSS, wait channel and I/O counters,
executable and working-directory links, and descriptor numbers/targets. Descriptor
contents, process argv/environment and source/configuration files are never read.
Paths and pipe links are metadata, not proof evidence. Samples can correlate
Cargo, compiler and solver stages and identify which owned process is accumulating
CPU or waiting; they cannot establish what a blocked pipe contains.

One snapshot scans at most16384 process identities and follows at most256 owned
processes. It records at most512 thread rows and256 descriptor links in aggregate;
links are bounded8192 bytes. Per-record output is256KiB; the complete sidecar is
24MiB with a reserved terminal truncation record. The fixed5-second sampling and
7300-second observer duration concern only this independent metadata process.
Kernel I/O can delay observation, so these are not a claim of a kernel wall-time
barrier. Any missing/truncated observation is explicit and cannot authorize an
acceptance result. No actors are stopped to obtain a sample.

## Evidence and next decision

Retain controller stdout/stderr/exit, all harness output (including partial plain
streams), the focused report, observations, frozen source/corpus/toolchain/runtime
identities and this exact script/test identity. Do not delete unit output with
scratch. On timeout, record the last seen stage, cumulative/delta CPU and whether
its descriptor/thread inventory is complete. Identify an actual causal bottleneck
before proposing a production change; do not infer it from sleeping alone.

Four helper controls passed: stat parsing with nested comm delimiters; nested
parent ancestry/reuse/unrelated-group exclusion; complete single truncation with
no later row; aggregate-cap reserved terminal record. They use synthetic metadata
and files, not a verifier or service. Actual Linux proc behavior, the newly frozen
harness and the single focused diagnostic remain pending. The original full suite,
source candidate, real repair performance and live-reader gates remain separate.
